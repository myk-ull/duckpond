//! macOS: a borderless window at the desktop-icon level, Finder's own desktop
//! icons switched off, and Finder-equivalent actions on each duck.
#![allow(unused_unsafe)]

use super::{check_new_name, edit_part, NameCheck, Outcome};
use crate::items::{kind_for, Item};
use objc2::AnyThread;
use objc2_app_kit::{NSSound, NSView, NSWindowCollectionBehavior, NSWorkspace};
use objc2_foundation::{NSData, NSString, NSURL};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tray_icon::menu::{ContextMenu, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use winit::window::Window;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGWindowLevelForKey(key: i32) -> i32;
}
const K_CG_DESKTOP_ICON_WINDOW_LEVEL_KEY: i32 = 18;

pub struct Plat {
    desktop: PathBuf,
    scale: f64,
    open_with: Vec<String>,
    /// results of work finished off the main thread (renames, duplicates)
    tx: std::sync::mpsc::Sender<Outcome>,
    rx: std::sync::mpsc::Receiver<Outcome>,
}

fn osa(script: &str) -> Option<String> {
    let out = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// A Finder-style warning (blocks: call from a worker thread).
fn alert(msg: &str) {
    let _ = osa(&format!(
        "tell application \"Finder\"\nactivate\ndisplay alert \"{}\" as warning\nend tell",
        q(msg)
    ));
}

/// Run AppleScript off the main thread so dialogs never freeze the pond.
fn osa_bg(script: String) {
    std::thread::spawn(move || {
        let _ = osa(&script);
    });
}

fn q(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn file_ref(path: &str) -> String {
    format!("(POSIX file \"{}\" as alias)", q(path))
}

fn ns_view(window: &Window) -> Option<*mut std::ffi::c_void> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::AppKit(h) => Some(h.ns_view.as_ptr()),
        _ => None,
    }
}

pub fn emergency_restore() {
    let _ = Command::new("defaults")
        .args(["delete", "com.apple.finder", "CreateDesktop"])
        .status();
    let _ = Command::new("killall").arg("Finder").status();
}

impl Plat {
    pub fn new(scale: f64) -> Plat {
        let (tx, rx) = std::sync::mpsc::channel();
        Plat {
            desktop: dirs::desktop_dir()
                .unwrap_or_else(|| dirs::home_dir().unwrap().join("Desktop")),
            scale,
            open_with: vec![],
            tx,
            rx,
        }
    }

    pub fn set_scale(&mut self, s: f64) {
        self.scale = s;
    }

    pub fn prepare_window(&self, window: &Window) {
        let Some(view) = ns_view(window) else { return };
        unsafe {
            let view: &NSView = &*(view as *const NSView);
            if let Some(win) = view.window() {
                win.setLevel(CGWindowLevelForKey(K_CG_DESKTOP_ICON_WINDOW_LEVEL_KEY) as isize);
                win.setCollectionBehavior(
                    NSWindowCollectionBehavior::CanJoinAllSpaces
                        | NSWindowCollectionBehavior::Stationary
                        | NSWindowCollectionBehavior::IgnoresCycle,
                );
                win.setHasShadow(false);
                if let Some(screen) = win.screen() {
                    win.setFrame_display(screen.frame(), true);
                }
            }
        }
    }

    pub fn native_icons_visible(&self) -> bool {
        let out = Command::new("defaults")
            .args(["read", "com.apple.finder", "CreateDesktop"])
            .output();
        match out {
            Ok(o) if o.status.success() => {
                let v = String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .to_ascii_lowercase();
                !(v == "0" || v == "false" || v == "no")
            }
            _ => true,
        }
    }

    pub fn set_native_icons(&mut self, visible: bool) {
        if visible {
            let _ = Command::new("defaults")
                .args(["delete", "com.apple.finder", "CreateDesktop"])
                .status();
        } else {
            let _ = Command::new("defaults")
                .args([
                    "write",
                    "com.apple.finder",
                    "CreateDesktop",
                    "-bool",
                    "false",
                ])
                .status();
        }
        let _ = Command::new("killall").arg("Finder").status();
    }

    /// Ask Finder where every icon sits (only works while its desktop is on).
    pub fn snapshot_positions(&mut self) -> HashMap<String, (f64, f64)> {
        let js = r#"const f=Application("Finder");const it=f.desktop.items;const n=it.name();const p=it.desktopPosition();JSON.stringify(n.map((x,i)=>[x,p[i].x,p[i].y]))"#;
        let out = Command::new("osascript")
            .args(["-l", "JavaScript", "-e", js])
            .output();
        let mut map = HashMap::new();
        if let Ok(o) = out {
            if let Ok(rows) = serde_json::from_slice::<Vec<(String, f64, f64)>>(&o.stdout) {
                for (name, x, y) in rows {
                    let key = self.desktop.join(&name).to_string_lossy().to_string();
                    map.insert(key, (x * self.scale, y * self.scale));
                }
            }
        }
        map
    }

    pub fn scan(&mut self) -> Option<Vec<Item>> {
        let rd = std::fs::read_dir(&self.desktop).ok()?;
        let mut items = vec![];
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "Icon\r" {
                continue;
            }
            let path = e.path();
            let lmeta = std::fs::symlink_metadata(&path).ok();
            let is_link = lmeta
                .as_ref()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false);
            let meta = std::fs::metadata(&path).ok();
            let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let kind = kind_for(&name, is_dir, is_link);
            items.push(Item {
                key: path.to_string_lossy().to_string(),
                name,
                kind,
                size,
                is_dir,
                pos: None,
            });
        }
        Some(items)
    }

    pub fn open(&self, key: &str) {
        let _ = Command::new("open").arg(key).spawn();
    }

    pub fn quick_look(&self, key: &str) {
        let _ = Command::new("qlmanage")
            .args(["-p", key])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }

    pub fn trash(&self, key: &str) {
        osa_bg(format!(
            "tell application \"Finder\" to delete {}",
            file_ref(key)
        ));
    }

    /// Finished renames / duplicates, for the pond to act on.
    pub fn poll(&mut self) -> Vec<Outcome> {
        self.rx.try_iter().collect()
    }

    /// Rename like Finder: the box starts with just the name part, so typing
    /// over it keeps ".mov"; typing a different extension asks first.
    pub fn rename(&self, key: &str) {
        let (key, tx) = (key.to_string(), self.tx.clone());
        std::thread::spawn(move || {
            let path = PathBuf::from(&key);
            let Some(old) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
                return;
            };
            let is_dir = path.is_dir();
            let start = edit_part(&old, is_dir);
            let kept = &old[start.len()..];
            let hint = if kept.is_empty() {
                String::new()
            } else {
                format!("\n\n“{kept}” is kept unless you type a different one.")
            };
            let Some(typed) = osa(&format!(
                "tell application \"Finder\"\nactivate\nset r to display dialog \"Rename “{o}”:{h}\" default answer \"{d}\" buttons {{\"Cancel\", \"Rename\"}} default button \"Rename\" cancel button \"Cancel\" with title \"Duck Pond\"\nreturn text returned of r\nend tell",
                o = q(&old),
                h = q(&hint),
                d = q(start)
            )) else {
                return;
            };
            let new = match check_new_name(&old, &typed, is_dir) {
                NameCheck::Ok(n) => n,
                NameCheck::Bad(m) => return alert(&m),
                NameCheck::AskExt { keep, change, old_ext, new_ext } => {
                    let pick = osa(&format!(
                        "tell application \"Finder\"\nactivate\nset r to display dialog \"Are you sure you want to change the extension from “.{o}” to “.{n}”?\n\nIf you make this change, your file may open in a different app.\" buttons {{\"Keep .{o}\", \"Use .{n}\"}} default button \"Keep .{o}\" with title \"Duck Pond\" with icon caution\nreturn button returned of r\nend tell",
                        o = q(&old_ext),
                        n = q(&new_ext)
                    ));
                    match pick {
                        Some(b) if b.starts_with("Use") => change,
                        Some(_) => keep,
                        None => return,
                    }
                }
            };
            if new == old {
                return;
            }
            let dst = path.with_file_name(&new);
            // a case-only change is the same file on a case-insensitive disk
            if dst.exists() && new.to_lowercase() != old.to_lowercase() {
                return alert(&format!(
                    "The name “{new}” is already taken. Please choose a different name."
                ));
            }
            if let Err(e) = std::fs::rename(&path, &dst) {
                return alert(&format!("“{old}” couldn't be renamed: {e}"));
            }
            let _ = tx.send(Outcome::Renamed {
                old: key,
                new: dst.to_string_lossy().to_string(),
            });
        });
    }

    /// Duplicate like Finder ("name copy.ext"), then tell the pond where the
    /// copy is so its duck can land beside the original.
    pub fn duplicate(&self, key: &str) {
        let (key, tx) = (key.to_string(), self.tx.clone());
        std::thread::spawn(move || {
            let script = format!(
                "tell application \"Finder\"\nset d to duplicate {}\nreturn POSIX path of (d as alias)\nend tell",
                file_ref(&key)
            );
            if let Some(p) = osa(&script) {
                let new = p.trim_end_matches('/').to_string();
                if !new.is_empty() {
                    let _ = tx.send(Outcome::Duplicated { src: key, new });
                }
            }
        });
    }

    /// Something was dragged onto the pond: move it onto the Desktop.
    pub fn import_drop(&self, src: &Path) -> Option<String> {
        if src.parent() == Some(self.desktop.as_path()) {
            return Some(src.to_string_lossy().to_string());
        }
        let name = src.file_name()?;
        let mut dst = self.desktop.join(name);
        let mut n = 2;
        while dst.exists() {
            let stem = src.file_stem()?.to_string_lossy();
            let ext = src
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            dst = self.desktop.join(format!("{stem} {n}{ext}"));
            n += 1;
        }
        if std::fs::rename(src, &dst).is_err() {
            // different volume: copy like Finder does
            Command::new("ditto").arg(src).arg(&dst).status().ok()?;
        }
        Some(dst.to_string_lossy().to_string())
    }

    pub fn play(&self, wav: &'static [u8], volume: f32) {
        unsafe {
            let data = NSData::with_bytes(wav);
            if let Some(s) = NSSound::initWithData(NSSound::alloc(), &data) {
                s.setVolume(volume);
                s.play();
                // NSSound keeps itself alive while playing.
                std::mem::forget(s);
            }
        }
    }

    // ---------------------------------------------------------------- menus

    fn apps_for(&self, key: &str) -> (Option<String>, Vec<String>) {
        unsafe {
            let ws = NSWorkspace::sharedWorkspace();
            let url = NSURL::fileURLWithPath(&NSString::from_str(key));
            let default = ws
                .URLForApplicationToOpenURL(&url)
                .and_then(|u| u.path())
                .map(|p| p.to_string());
            let mut apps: Vec<String> = ws
                .URLsForApplicationsToOpenURL(&url)
                .iter()
                .filter_map(|u| u.path())
                .map(|p| p.to_string())
                .collect();
            apps.retain(|a| Some(a) != default.as_ref());
            apps.sort_by_key(|a| app_name(a).to_lowercase());
            apps.dedup();
            apps.truncate(14);
            (default, apps)
        }
    }

    pub fn show_item_menu(&mut self, window: &Window, key: &str) -> Outcome {
        let Some(view) = ns_view(window) else {
            return Outcome::None;
        };
        let name = Path::new(key)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let (default, apps) = self.apps_for(key);
        self.open_with.clear();

        let m = Menu::new();
        let _ = m.append(&MenuItem::with_id("duck:open", "Open", true, None));
        let ow = Submenu::new("Open With", true);
        if let Some(d) = &default {
            self.open_with.push(d.clone());
            let _ = ow.append(&MenuItem::with_id(
                "duck:ow:0",
                format!("{} (default)", app_name(d)),
                true,
                None,
            ));
            let _ = ow.append(&PredefinedMenuItem::separator());
        }
        for a in apps {
            let i = self.open_with.len();
            self.open_with.push(a.clone());
            let _ = ow.append(&MenuItem::with_id(
                format!("duck:ow:{i}"),
                app_name(&a),
                true,
                None,
            ));
        }
        let _ = ow.append(&PredefinedMenuItem::separator());
        let _ = ow.append(&MenuItem::with_id("duck:owother", "Other…", true, None));
        let _ = m.append(&ow);
        let _ = m.append(&PredefinedMenuItem::separator());
        let _ = m.append(&MenuItem::with_id(
            "duck:trash",
            "Move to Trash",
            true,
            None,
        ));
        let _ = m.append(&PredefinedMenuItem::separator());
        let _ = m.append(&MenuItem::with_id("duck:info", "Get Info", true, None));
        let _ = m.append(&MenuItem::with_id("duck:rename", "Rename…", true, None));
        let _ = m.append(&MenuItem::with_id(
            "duck:zip",
            format!("Compress “{}”", short(&name)),
            true,
            None,
        ));
        let _ = m.append(&MenuItem::with_id("duck:dup", "Duplicate", true, None));
        let _ = m.append(&MenuItem::with_id("duck:alias", "Make Alias", true, None));
        let _ = m.append(&MenuItem::with_id(
            "duck:ql",
            format!("Quick Look “{}”", short(&name)),
            true,
            None,
        ));
        let _ = m.append(&PredefinedMenuItem::separator());
        let _ = m.append(&MenuItem::with_id(
            "duck:copy",
            format!("Copy “{}”", short(&name)),
            true,
            None,
        ));
        let _ = m.append(&PredefinedMenuItem::separator());
        let _ = m.append(&MenuItem::with_id(
            "duck:reveal",
            "Show in Finder",
            true,
            None,
        ));
        let _ = m.append(&PredefinedMenuItem::separator());
        let _ = m.append(&MenuItem::with_id(
            "duck:customise",
            "Customise This Duck…",
            true,
            None,
        ));

        unsafe {
            m.show_context_menu_for_nsview(view, None);
        }
        let mut out = Outcome::None;
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            out = self.handle_item(&ev.id.0, key);
        }
        out
    }

    fn handle_item(&self, id: &str, key: &str) -> Outcome {
        let f = file_ref(key);
        match id {
            "duck:open" => self.open(key),
            "duck:owother" => osa_bg(format!(
                "tell application \"Finder\"\nactivate\nset a to choose application with prompt \"Open with:\" as alias\nopen {f} using a\nend tell"
            )),
            "duck:trash" => {
                self.trash(key);
                return Outcome::Rescan;
            }
            "duck:info" => osa_bg(format!("tell application \"Finder\"\nactivate\nopen information window of {f}\nend tell")),
            "duck:rename" => self.rename(key),
            "duck:zip" => {
                let p = PathBuf::from(key);
                let dst = p.with_file_name(format!("{}.zip", p.file_name().unwrap_or_default().to_string_lossy()));
                let _ = Command::new("ditto").args(["-c", "-k", "--sequesterRsrc", "--keepParent"]).arg(&p).arg(dst).spawn();
                return Outcome::Rescan;
            }
            "duck:dup" => self.duplicate(key),
            "duck:alias" => osa_bg(format!("tell application \"Finder\" to make new alias file at desktop to {f}")),
            "duck:ql" => self.quick_look(key),
            "duck:copy" => osa_bg(format!("set the clipboard to (POSIX file \"{}\")", q(key))),
            "duck:reveal" => {
                let _ = Command::new("open").arg("-R").arg(key).spawn();
            }
            "duck:customise" => return Outcome::Customise(key.to_string()),
            other => {
                if let Some(i) = other.strip_prefix("duck:ow:").and_then(|s| s.parse::<usize>().ok()) {
                    if let Some(app) = self.open_with.get(i) {
                        let _ = Command::new("open").arg("-a").arg(app).arg(key).spawn();
                    }
                }
            }
        }
        Outcome::None
    }

    pub fn show_desktop_menu(&mut self, window: &Window, labels_on: bool) -> Outcome {
        let Some(view) = ns_view(window) else {
            return Outcome::None;
        };
        let m = Menu::new();
        let _ = m.append(&MenuItem::with_id(
            "desk:newfolder",
            "New Folder",
            true,
            None,
        ));
        let _ = m.append(&PredefinedMenuItem::separator());
        let _ = m.append(&MenuItem::with_id(
            "desk:home",
            "Send Ducks Home",
            true,
            None,
        ));
        let _ = m.append(&MenuItem::with_id(
            "desk:feed",
            "Bread for Everyone",
            true,
            None,
        ));
        let _ = m.append(&MenuItem::with_id(
            "desk:labels",
            if labels_on {
                "Hide Name Tags"
            } else {
                "Show Name Tags"
            },
            true,
            None,
        ));
        let _ = m.append(&PredefinedMenuItem::separator());
        let _ = m.append(&MenuItem::with_id(
            "desk:finder",
            "Open Desktop in Finder",
            true,
            None,
        ));
        let _ = m.append(&MenuItem::with_id(
            "desk:prefs",
            "Duck Pond Settings…",
            true,
            None,
        ));
        unsafe {
            m.show_context_menu_for_nsview(view, None);
        }
        let mut out = Outcome::None;
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            out = match ev.id.0.as_str() {
                "desk:newfolder" => {
                    osa_bg("tell application \"Finder\" to make new folder at desktop".into());
                    Outcome::Rescan
                }
                "desk:home" => Outcome::SendHome,
                "desk:feed" => Outcome::FeedAll,
                "desk:labels" => Outcome::ToggleLabels,
                "desk:prefs" => Outcome::OpenPrefs,
                "desk:finder" => {
                    let _ = Command::new("open").arg(&self.desktop).spawn();
                    Outcome::None
                }
                _ => Outcome::None,
            };
        }
        out
    }
}

fn app_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn short(name: &str) -> String {
    if name.chars().count() > 28 {
        format!("{}…", name.chars().take(27).collect::<String>())
    } else {
        name.to_string()
    }
}
