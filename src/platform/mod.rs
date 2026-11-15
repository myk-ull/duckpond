//! Per-OS desktop integration. Each backend exposes the same `Plat` surface:
//! prepare_window, native_icons_visible, set_native_icons, snapshot_positions,
//! scan, open, quick_look, trash, rename, import_drop, play, show_item_menu,
//! show_desktop_menu, plus a free `emergency_restore()`.

/// What the pond should do after a menu closes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    None,
    Rescan,
    SendHome,
    FeedAll,
    ToggleLabels,
    /// open the Preferences window
    OpenPrefs,
    /// open Preferences on this file's row
    Customise(String),
    /// a file was renamed from the pond: the duck keeps its spot and looks
    Renamed { old: String, new: String },
    /// a file was duplicated from the pond: the copy lands beside it
    Duplicated { src: String, new: String },
}

/// What to do with a name typed into a rename box.
#[derive(Debug, PartialEq, Eq)]
pub enum NameCheck {
    /// rename to this
    Ok(String),
    /// the extension changed: ask "Keep .old / Use .new"; (keep, use)
    AskExt { keep: String, change: String, old_ext: String, new_ext: String },
    /// can't use it; the message says why
    Bad(String),
}

/// A real extension: short, no spaces ("v1.2 draft" has none).
pub fn ext_of(name: &str) -> Option<&str> {
    let (stem, ext) = name.rsplit_once('.')?;
    let ok = !stem.is_empty()
        && !ext.is_empty()
        && ext.len() <= 12
        && ext.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    ok.then_some(ext)
}

pub fn stem_of(name: &str) -> &str {
    match ext_of(name) {
        Some(e) => &name[..name.len() - e.len() - 1],
        None => name,
    }
}

/// Folders only carry an extension when they're bundles (.app, .pages…).
fn keeps_ext(name: &str, is_dir: bool) -> Option<&str> {
    let e = ext_of(name)?;
    if !is_dir {
        return Some(e);
    }
    const BUNDLES: [&str; 12] = [
        "app", "pages", "numbers", "key", "bundle", "rtfd", "photoslibrary", "framework",
        "xcodeproj", "xcworkspace", "playground", "band",
    ];
    BUNDLES.contains(&e.to_ascii_lowercase().as_str()).then_some(e)
}

/// The part of a name a rename box starts with selected (or pre-filled):
/// everything but a kept extension.
pub fn edit_part(name: &str, is_dir: bool) -> &str {
    match keeps_ext(name, is_dir) {
        Some(e) => &name[..name.len() - e.len() - 1],
        None => name,
    }
}

/// Finder / Explorer rules for a typed name: typing over the name part
/// keeps the extension; typing a different one asks first.
pub fn check_new_name(old: &str, typed: &str, is_dir: bool) -> NameCheck {
    let typed = typed.trim();
    if typed.is_empty() || typed == "." || typed == ".." {
        return NameCheck::Bad("A name can't be empty.".into());
    }
    let bad: &[char] = if cfg!(windows) {
        &['<', '>', ':', '"', '/', '\\', '|', '?', '*']
    } else {
        &['/', ':']
    };
    if let Some(c) = typed.chars().find(|c| bad.contains(c) || c.is_control()) {
        return NameCheck::Bad(format!("A name can't contain “{c}”."));
    }
    let Some(old_ext) = keeps_ext(old, is_dir) else {
        return NameCheck::Ok(typed.to_string());
    };
    match ext_of(typed) {
        None => NameCheck::Ok(format!("{typed}.{old_ext}")),
        Some(e) if e.eq_ignore_ascii_case(old_ext) => NameCheck::Ok(typed.to_string()),
        Some(e) => NameCheck::AskExt {
            keep: format!("{}.{old_ext}", stem_of(typed)),
            change: typed.to_string(),
            old_ext: old_ext.to_string(),
            new_ext: e.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rename_rules() {
        assert_eq!(check_new_name("clip.mov", "holiday", false), NameCheck::Ok("holiday.mov".into()));
        assert_eq!(check_new_name("clip.mov", "holiday.MOV", false), NameCheck::Ok("holiday.MOV".into()));
        assert!(matches!(check_new_name("clip.mov", "holiday.mp4", false), NameCheck::AskExt { .. }));
        assert_eq!(check_new_name("Photos", "Pics", true), NameCheck::Ok("Pics".into()));
        assert_eq!(check_new_name("my.folder", "x", true), NameCheck::Ok("x".into()));
        assert_eq!(check_new_name("Notes.app", "Jot", true), NameCheck::Ok("Jot.app".into()));
        assert_eq!(check_new_name("v1.2 draft", "v1.3 draft", false), NameCheck::Ok("v1.3 draft".into()));
        assert_eq!(check_new_name("README", "readme", false), NameCheck::Ok("readme".into()));
        assert!(matches!(check_new_name("a.txt", "  ", false), NameCheck::Bad(_)));
        assert_eq!(stem_of("clip.mov"), "clip");
    }
}

#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "macos")]
pub use mac::*;

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::*;
