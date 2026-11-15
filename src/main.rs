#![cfg_attr(windows, windows_subsystem = "windows")]
//! Duck Pond: your desktop icons as ducks on Monet's 1906 Water Lilies.

mod autostart;
mod catalog;
mod font;
mod items;
#[cfg(not(windows))]
mod labels;
mod platform;
mod quacks;
mod scene;
mod sprite_meta;

use items::{Item, Settings};
#[cfg(not(windows))]
use pixels::{wgpu, Pixels, PixelsBuilder, SurfaceTexture};
use platform::{Outcome, Plat};
use scene::{Img, Scene, Sound};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

static POND_PNG: &[u8] = include_bytes!("../assets/pond.png");
static LAND_PNG: &[u8] = include_bytes!("../assets/land.png");
static DUCKS_PNG: &[u8] = include_bytes!("../assets/ducks.png");
static HATS_PNG: &[u8] = include_bytes!("../assets/hats.png");

/// Dev/test mode: read real icon positions but never hide Finder/Explorer
/// icons, touch login items, or sit at desktop level.
fn demo() -> bool {
    std::env::var_os("DUCKPOND_DEMO").is_some() || snapshot_path().is_some()
}
/// Running straight out of cargo's target/ dir: don't install a login item
/// pointing at a build artefact.
fn dev_build() -> bool {
    std::env::current_exe()
        .map(|p| p.components().any(|c| c.as_os_str() == "target"))
        .unwrap_or(false)
}
/// Render the pond off-screen to this PNG after a few seconds, then exit.
fn snapshot_path() -> Option<String> {
    std::env::var("DUCKPOND_SNAPSHOT").ok()
}

static STOP: AtomicBool = AtomicBool::new(false);
const DOUBLE_CLICK: Duration = Duration::from_millis(380);
/// Target art width; the art pixel size is chosen so the pond is ~this wide.
const ART_WIDTH: f64 = 620.0;

fn set_window_visible(window: &Window, visible: bool) {
    #[cfg(windows)]
    platform::set_pond_visible(window, visible);
    #[cfg(not(windows))]
    window.set_visible(visible);
}

#[derive(Clone, Copy, PartialEq)]
enum Target {
    Duck(usize),
    Empty,
}

struct Press {
    duck: Option<usize>,
    at: (f32, f32),
    dragging: bool,
}

struct App {
    window: Option<Arc<Window>>,
    #[cfg(not(windows))]
    pixels: Option<Pixels<'static>>,
    #[cfg(windows)]
    gdi_frame: Vec<u8>,
    #[cfg(not(windows))]
    labels: Option<labels::Labels>,
    scene: Option<Scene>,
    plat: Plat,
    settings: Settings,
    pond: Img,
    land: Img,
    factor: u32,
    last: Instant,
    next_frame: Instant,
    next_scan: Instant,
    last_sound: Instant,
    cursor_art: (f32, f32),
    press: Option<Press>,
    last_click: Option<(Instant, Target, (f32, f32))>,
    pending_food: Option<(Instant, (f32, f32))>,
    mods: ModifiersState,
    occluded: bool,
    paused: bool,
    exiting: bool,
    tray: Option<TrayIcon>,
    tray_labels: Option<CheckMenuItem>,
    tray_mute: Option<CheckMenuItem>,
    tray_hats: Option<CheckMenuItem>,
    tray_pause: Option<CheckMenuItem>,
    /// last desktop listing, for re-applying the duckling share
    last_items: Vec<Item>,
    next_prefs_check: Instant,
    known: HashSet<String>,
    next_clock: Instant,
    started: Instant,
    snap_stage: u8,
}

fn save_png(path: &str, w: u32, h: u32, rgba: &[u8], k: u32) {
    let (bw, bh) = (w * k, h * k);
    let mut big = vec![0u8; (bw * bh * 4) as usize];
    for y in 0..bh {
        for x in 0..bw {
            let s = (((y / k) * w + x / k) * 4) as usize;
            let d = ((y * bw + x) * 4) as usize;
            big[d..d + 4].copy_from_slice(&rgba[s..s + 4]);
        }
    }
    let Ok(f) = std::fs::File::create(path) else {
        return;
    };
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), bw, bh);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    if let Ok(mut wr) = e.write_header() {
        let _ = wr.write_image_data(&big);
    }
}

impl App {
    fn new() -> App {
        let now = Instant::now();
        App {
            window: None,
            #[cfg(not(windows))]
            pixels: None,
            #[cfg(windows)]
            gdi_frame: Vec::new(),
            #[cfg(not(windows))]
            labels: None,
            scene: None,
            plat: Plat::new(1.0),
            settings: Settings::load(),
            pond: Img::decode(POND_PNG),
            land: Img::decode(LAND_PNG),
            factor: 1,
            last: now,
            next_frame: now,
            next_scan: now,
            last_sound: now,
            cursor_art: (0.0, 0.0),
            press: None,
            last_click: None,
            pending_food: None,
            mods: ModifiersState::empty(),
            occluded: false,
            paused: false,
            exiting: false,
            tray: None,
            tray_labels: None,
            tray_mute: None,
            tray_hats: None,
            tray_pause: None,
            last_items: vec![],
            next_prefs_check: now,
            known: HashSet::new(),
            next_clock: now,
            started: now,
            snap_stage: 0,
        }
    }

    fn art_size(&mut self, pw: u32, ph: u32) -> (u32, u32) {
        self.factor = ((pw as f64 / ART_WIDTH).round() as u32).max(1);
        ((pw / self.factor).max(1), (ph / self.factor).max(1))
    }

    fn create_window(&mut self, el: &ActiveEventLoop) {
        let Some(mon) = el
            .primary_monitor()
            .or_else(|| el.available_monitors().next())
        else {
            return;
        };
        #[allow(unused_mut)]
        let mut attrs = Window::default_attributes()
            .with_title("Duck Pond")
            .with_decorations(false)
            .with_resizable(false)
            .with_visible(false)
            .with_position(mon.position())
            .with_inner_size(mon.size());
        #[cfg(target_os = "macos")]
        {
            use winit::platform::macos::WindowAttributesExtMacOS;
            attrs = attrs.with_has_shadow(false).with_accepts_first_mouse(true);
        }
        #[cfg(windows)]
        {
            use winit::platform::windows::WindowAttributesExtWindows;
            attrs = attrs.with_skip_taskbar(true);
            if !demo() {
                attrs = unsafe { attrs.with_parent_window(Some(self.plat.parent_handle())) };
            }
        }
        let window = Arc::new(el.create_window(attrs).expect("create window"));
        self.plat.set_scale(window.scale_factor());
        if !demo() {
            self.plat.prepare_window(&window);
        }

        let size = window.inner_size();
        let (aw, ah) = self.art_size(size.width, size.height);
        #[cfg(not(windows))]
        let st = SurfaceTexture::new(size.width, size.height, window.clone());
        #[cfg(not(windows))]
        let pixels = PixelsBuilder::new(aw, ah, st)
            .request_adapter_options(wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .clear_color(wgpu::Color {
                r: 0.33,
                g: 0.47,
                b: 0.68,
                a: 1.0,
            })
            .build()
            .expect("gpu surface");

        match &mut self.scene {
            Some(s) if s.w != aw || s.h != ah => s.resize(aw, ah, &self.pond, &self.land),
            Some(_) => {}
            None => {
                self.scene = Some(Scene::new(
                    aw,
                    ah,
                    &self.pond,
                    &self.land,
                    Img::decode(DUCKS_PNG),
                    Img::decode(HATS_PNG),
                ));
            }
        }
        if let Some(s) = &mut self.scene {
            s.show_labels = self.settings.show_labels;
            s.show_hats = self.settings.show_hats || std::env::var_os("DUCKPOND_HATS").is_some();
        }
        set_window_visible(&window, snapshot_path().is_none());
        self.occluded = false;
        #[cfg(not(windows))]
        {
            self.labels = labels::Labels::new(&pixels, window.scale_factor());
            self.pixels = Some(pixels);
        }
        self.window = Some(window);
    }

    /// First launch of the pond: remember where every native icon is, then
    /// hide the real icons.
    fn take_over_desktop(&mut self) {
        if demo() {
            let snap = self.plat.snapshot_positions();
            self.settings.state.homes.extend(snap);
            return;
        }
        if self.plat.native_icons_visible() {
            let snap = self.plat.snapshot_positions();
            if !snap.is_empty() {
                self.settings.state.homes.extend(snap);
                self.settings.save();
            }
            self.plat.set_native_icons(false);
        }
    }

    fn sync(&mut self, items: Vec<Item>, entrance: bool) {
        let Some(scene) = self.scene.as_mut() else {
            return;
        };
        let f = self.factor as f64;
        let now_keys: HashSet<String> = items.iter().map(|i| i.key.clone()).collect();
        let removed: Vec<String> = self.known.difference(&now_keys).cloned().collect();
        let added: Vec<&Item> = items
            .iter()
            .filter(|i| !self.known.contains(&i.key))
            .collect();
        let had_new = !added.is_empty();

        // A rename shows up as one removal + one addition: keep the duck's spot.
        if removed.len() == 1 && added.len() == 1 && !entrance {
            if let Some(old) = self.settings.state.homes.remove(&removed[0]) {
                self.settings.state.homes.insert(added[0].key.clone(), old);
            }
        }
        for k in &removed {
            scene.remove_duck(k);
            self.settings.state.homes.remove(k);
        }
        let adults = self.settings.adult_keys(&items);
        let mut rng = scene::Rng::new(items.len() as u64 + 3);
        let mut dirty = !removed.is_empty();
        for it in added {
            let phys = it.pos.or_else(|| self.settings.state.homes.get(&it.key).copied());
            let home = match phys {
                Some((x, y)) => ((x / f) as f32, (y / f) as f32),
                None => {
                    let s = scene.free_slot();
                    self.settings
                        .state
                        .homes
                        .insert(it.key.clone(), (s.0 as f64 * f, s.1 as f64 * f));
                    dirty = true;
                    s
                }
            };
            if it.pos.is_some() && !self.settings.state.homes.contains_key(&it.key) {
                self.settings.state.homes.insert(it.key.clone(), it.pos.unwrap());
                dirty = true;
            }
            let delay = if entrance { rng.range(0.0, 1.8) } else { 0.0 };
            let h = scene::hash(&it.name);
            let breed = self.settings.breed_for(it.kind, &it.key, h);
            let hat = self.settings.hat_for(it.kind, &it.key, h);
            scene.add_duck(
                it.key.clone(),
                it.name.clone(),
                it.kind,
                !adults.contains(&it.key),
                breed,
                hat,
                home,
                delay,
            );
        }
        self.known = now_keys;
        // the ducklings rule depends on the whole flock: re-apply on any change
        if !removed.is_empty() || had_new {
            let adults = self.settings.adult_keys(&items);
            for d in scene.ducks.iter_mut() {
                d.baby = !adults.contains(&d.key);
            }
            scene.mark_families_dirty();
        }
        self.last_items = items.clone();
        if had_new {
            scene.relax_homes();
            // remember the de-overlapped spots so they survive restarts
            for d in &scene.ducks {
                self.settings
                    .state
                    .homes
                    .insert(d.key.clone(), (d.home.0 as f64 * f, d.home.1 as f64 * f));
            }
            dirty = true;
        }
        if dirty {
            self.settings.save_state();
        }
        if dirty || had_new || !removed.is_empty() {
            catalog::write_items(&scene.ducks);
        }
    }

    fn rescan(&mut self, entrance: bool) {
        if let Some(items) = self.plat.scan() {
            self.sync(items, entrance);
        }
        self.next_scan = Instant::now() + Duration::from_secs(2);
    }

    fn apply(&mut self, o: Outcome) {
        match o {
            Outcome::None => {}
            Outcome::Rescan => self.next_scan = Instant::now() + Duration::from_millis(400),
            Outcome::SendHome => {
                if let Some(s) = &mut self.scene {
                    s.send_all_home()
                }
            }
            Outcome::FeedAll => {
                if let Some(s) = &mut self.scene {
                    s.feed_everyone()
                }
            }
            Outcome::ToggleLabels => self.toggle_labels(),
            Outcome::OpenPrefs => self.open_preferences(None),
            Outcome::Customise(key) => self.open_preferences(Some(key)),
        }
    }

    /// Open the native Preferences window (a small SwiftUI app next to the
    /// pond binary on macOS). Optionally jump straight to one file.
    fn open_preferences(&mut self, file: Option<String>) {
        #[cfg(target_os = "macos")]
        {
            let helper = std::env::current_exe()
                .ok()
                .and_then(|e| e.parent().map(|d| d.join("DuckPondPreferences")))
                .filter(|h| h.exists());
            match helper {
                Some(h) => {
                    let mut c = std::process::Command::new(h);
                    c.arg("--config").arg(items::config_dir());
                    if let Some(f) = file {
                        c.arg("--file").arg(f);
                    }
                    let _ = c.spawn();
                }
                None => eprintln!("Preferences app not built: run scripts/build_prefs.sh"),
            }
        }
        #[cfg(windows)]
        {
            // TODO(windows): native settings window. Until then, the JSON.
            let _ = file;
            let _ = std::process::Command::new("notepad")
                .arg(items::Settings::prefs_path())
                .spawn();
        }
    }

    /// Re-apply preferences to the whole pond (after the Preferences window
    /// or the tray menu changed them).
    fn reapply_prefs(&mut self) {
        if let Some(scene) = &mut self.scene {
            let adults = self.settings.adult_keys(&self.last_items);
            for d in scene.ducks.iter_mut() {
                let h = scene::hash(&d.name);
                d.breed = self.settings.breed_for(d.kind, &d.key, h);
                d.hat = self.settings.hat_for(d.kind, &d.key, h);
                d.baby = !adults.contains(&d.key);
            }
            scene.mark_families_dirty();
            scene.show_labels = self.settings.show_labels || self.mods.alt_key();
            scene.show_hats = self.settings.show_hats;
            catalog::write_items(&scene.ducks);
        }
        self.next_clock = Instant::now();
        if !demo() && !dev_build() && autostart::is_enabled() != self.settings.open_at_login {
            autostart::set(self.settings.open_at_login);
        }
        if let Some(c) = &self.tray_labels {
            c.set_checked(self.settings.show_labels);
        }
        if let Some(c) = &self.tray_hats {
            c.set_checked(self.settings.show_hats);
        }
        if let Some(c) = &self.tray_mute {
            c.set_checked(self.settings.muted);
        }
    }

    fn toggle_labels(&mut self) {
        self.settings.show_labels = !self.settings.show_labels;
        self.settings.save_prefs();
        if let Some(s) = &mut self.scene {
            s.show_labels = self.settings.show_labels;
        }
        if let Some(c) = &self.tray_labels {
            c.set_checked(self.settings.show_labels);
        }
    }

    fn build_tray(&mut self) {
        let menu = Menu::new();
        let labels = CheckMenuItem::with_id(
            "tray:labels",
            "Show Name Tags",
            true,
            self.settings.show_labels,
            None,
        );
        let hats = CheckMenuItem::with_id(
            "tray:hats",
            "Show Hats",
            true,
            self.settings.show_hats,
            None,
        );
        let mute =
            CheckMenuItem::with_id("tray:mute", "Mute Quacks", true, self.settings.muted, None);
        let pause = CheckMenuItem::with_id(
            "tray:pause",
            "Pause Ducks (show normal icons)",
            true,
            false,
            None,
        );
        let _ = menu.append(&MenuItem::with_id(
            "tray:home",
            "Send Ducks Home",
            true,
            None,
        ));
        let _ = menu.append(&MenuItem::with_id(
            "tray:feed",
            "Bread for Everyone",
            true,
            None,
        ));
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&MenuItem::with_id(
            "tray:prefs",
            "Settings…",
            true,
            None,
        ));
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&labels);
        let _ = menu.append(&hats);
        let _ = menu.append(&mute);
        let _ = menu.append(&pause);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&MenuItem::with_id(
            "tray:quit",
            "Quit Duck Pond",
            true,
            None,
        ));

        // menu-bar icon: the Pekin, at 2x
        let sheet = Img::decode(DUCKS_PNG);
        let (cw, ch) = (sprite_meta::CELL_W, sprite_meta::CELL_H);
        let mut rgba = vec![0u8; (cw * 2 * ch * 2 * 4) as usize];
        for y in 0..ch * 2 {
            for x in 0..cw * 2 {
                let si = (((y / 2) * sheet.w + x / 2) * 4) as usize;
                let di = ((y * cw * 2 + x) * 4) as usize;
                rgba[di..di + 4].copy_from_slice(&sheet.px[si..si + 4]);
            }
        }
        let icon = Icon::from_rgba(rgba, cw * 2, ch * 2).ok();
        let mut b = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Duck Pond");
        if let Some(i) = icon {
            b = b.with_icon(i);
        }
        self.tray = b.build().ok();
        self.tray_labels = Some(labels);
        self.tray_mute = Some(mute);
        self.tray_hats = Some(hats);
        self.tray_pause = Some(pause);
    }

    fn handle_tray(&mut self, el: &ActiveEventLoop) {
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            match ev.id.0.as_str() {
                "tray:home" => self.apply(Outcome::SendHome),
                "tray:feed" => self.apply(Outcome::FeedAll),
                "tray:labels" => self.toggle_labels(),
                "tray:hats" => {
                    self.settings.show_hats = !self.settings.show_hats;
                    self.settings.save_prefs();
                    self.reapply_prefs();
                }
                "tray:mute" => {
                    self.settings.muted = !self.settings.muted;
                    self.settings.save_prefs();
                }
                "tray:prefs" => self.open_preferences(None),
                "tray:pause" => self.set_paused(!self.paused),
                "tray:quit" => {
                    self.exiting = true;
                    el.exit();
                }
                _ => {}
            }
        }
    }

    fn set_paused(&mut self, paused: bool) {
        if paused == self.paused {
            return;
        }
        self.paused = paused;
        if let Some(c) = &self.tray_pause {
            c.set_checked(paused);
        }
        if paused {
            if let Some(w) = &self.window {
                set_window_visible(w, false);
            }
            self.plat.set_native_icons(true);
        } else {
            // icons may have been moved while visible: re-learn homes
            let snap = self.plat.snapshot_positions();
            let f = self.factor as f64;
            if let Some(scene) = &mut self.scene {
                for (k, (x, y)) in &snap {
                    if let Some(i) = scene.find(k) {
                        scene.ducks[i].home = ((x / f) as f32, (y / f) as f32);
                    }
                }
            }
            self.settings.state.homes.extend(snap);
            self.settings.save();
            self.plat.set_native_icons(false);
            if let Some(w) = &self.window {
                set_window_visible(w, true);
            }
            self.apply(Outcome::SendHome);
        }
    }

    fn play_sounds(&mut self) {
        let Some(scene) = &mut self.scene else { return };
        let sounds: Vec<Sound> = scene.sounds.drain(..).collect();
        if self.settings.muted || self.occluded {
            return;
        }
        let now = Instant::now();
        for s in sounds.into_iter().take(1) {
            if now.duration_since(self.last_sound) < Duration::from_millis(110) {
                return;
            }
            self.last_sound = now;
            // a different take every time, so the pond never sounds looped
            let r = now.duration_since(self.started).as_nanos() as usize;
            let r = r ^ (r >> 13) ^ (r >> 29);
            match s {
                Sound::Quack(v) => {
                    let n = quacks::QUACKS.len();
                    // each duck leans towards its own voice, with some wobble
                    let i = (v as usize * 4 + r % 5) % n;
                    self.plat.play(quacks::QUACKS[i], self.settings.volume.clamp(0.0, 1.0) * 0.85);
                }
                Sound::Peep => self.plat.play(
                    quacks::PEEPS[r % quacks::PEEPS.len()],
                    self.settings.volume.clamp(0.0, 1.0) * 0.7,
                ),
            }
        }
    }

    /// Scripted visit for DUCKPOND_SNAPSHOT: let the ducks land, drop bread
    /// near a cluster, then save frames along the way.
    fn snapshot_step(&mut self, el: &ActiveEventLoop, out: &str) {
        let Some(scene) = self.scene.as_mut() else {
            return;
        };
        let t = self.started.elapsed().as_secs_f32();
        if t > 2.5 && self.snap_stage == 0 {
            self.snap_stage = 1;
            let (x, y) = scene
                .ducks
                .get(scene.ducks.len() / 3)
                .map(|d| (d.x + 14.0, d.y))
                .unwrap_or((100.0, 100.0));
            scene.drop_food(x, y);
            eprintln!("bread at {x:.0},{y:.0}");
            scene.demo_extras();
            scene.hover = Some(0);
        }
        let shots = [(2.4, "a"), (4.2, "b"), (6.0, "c")];
        for (i, (when, tag)) in shots.iter().enumerate() {
            if t > *when && self.snap_stage < 2 + i as u8 && self.snap_stage >= 1 + i as u8 {
                self.snap_stage = 2 + i as u8;
                let mut buf = vec![0u8; (scene.w * scene.h * 4) as usize];
                scene.render(&mut buf);
                let path = out.replace(".png", &format!("_{tag}.png"));
                save_png(&path, scene.w, scene.h, &buf, 3);
                eprintln!("saved {path} ({} ducks)", scene.ducks.len());
            }
        }
        if self.snap_stage >= 1 + shots.len() as u8 {
            el.exit();
        }
    }

    fn selected_key(&self) -> Option<String> {
        let s = self.scene.as_ref()?;
        s.selected
            .and_then(|i| s.ducks.get(i))
            .map(|d| d.key.clone())
    }

    fn on_mouse(&mut self, button: MouseButton, state: ElementState) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let Some(scene) = self.scene.as_mut() else {
            return;
        };
        let at = self.cursor_art;
        let now = Instant::now();
        match (button, state) {
            (MouseButton::Left, ElementState::Pressed) => {
                self.press = Some(Press {
                    duck: scene.hit(at.0, at.1),
                    at,
                    dragging: false,
                });
            }
            (MouseButton::Left, ElementState::Released) => {
                let Some(p) = self.press.take() else { return };
                if p.dragging {
                    if let Some(i) = p.duck {
                        if let Some((x, y)) = scene.release(i) {
                            let f = self.factor as f64;
                            let key = scene.ducks[i].key.clone();
                            self.settings
                                .state
                                .homes
                                .insert(key, (x as f64 * f, y as f64 * f));
                            self.settings.save_state();
                        }
                    }
                    return;
                }
                let double = |t: Target, lc: &Option<(Instant, Target, (f32, f32))>| {
                    matches!(lc, Some((when, tt, pos)) if *tt == t && now.duration_since(*when) < DOUBLE_CLICK
                        && (pos.0 - at.0).hypot(pos.1 - at.1) < 8.0)
                };
                match p.duck {
                    Some(i) => {
                        if double(Target::Duck(i), &self.last_click) {
                            let key = scene.ducks[i].key.clone();
                            self.plat.open(&key);
                            self.last_click = None;
                        } else {
                            scene.selected = Some(i);
                            scene.wake_and_quack(i);
                            self.last_click = Some((now, Target::Duck(i), at));
                        }
                    }
                    None => {
                        if double(Target::Empty, &self.last_click) {
                            self.pending_food = None;
                            scene.send_all_home();
                            self.last_click = None;
                        } else {
                            scene.selected = None;
                            self.pending_food = Some((now + DOUBLE_CLICK, at));
                            self.last_click = Some((now, Target::Empty, at));
                        }
                    }
                }
            }
            (MouseButton::Right, ElementState::Pressed) => {
                self.press = None;
                let outcome = match scene.hit(at.0, at.1) {
                    Some(i) => {
                        scene.selected = Some(i);
                        let key = scene.ducks[i].key.clone();
                        self.plat.show_item_menu(&window, &key)
                    }
                    None => {
                        let labels = scene.show_labels;
                        self.plat.show_desktop_menu(&window, labels)
                    }
                };
                self.apply(outcome);
            }
            _ => {}
        }
    }

    fn on_key(&mut self, key: &Key) {
        let cmd = if cfg!(target_os = "macos") {
            self.mods.super_key()
        } else {
            self.mods.control_key()
        };
        let Some(sel) = self.selected_key() else {
            return;
        };
        match key {
            Key::Named(NamedKey::Space) => self.plat.quick_look(&sel),
            Key::Named(NamedKey::Enter) => {
                if cfg!(target_os = "macos") {
                    self.plat.rename(&sel)
                } else {
                    self.plat.open(&sel)
                }
            }
            Key::Named(NamedKey::F2) => self.plat.rename(&sel),
            Key::Named(NamedKey::ArrowDown) if cmd => self.plat.open(&sel),
            Key::Named(NamedKey::Backspace) if cmd => self.plat.trash(&sel),
            Key::Named(NamedKey::Delete) => self.plat.trash(&sel),
            Key::Named(NamedKey::Escape) => {
                if let Some(s) = &mut self.scene {
                    s.selected = None;
                }
            }
            Key::Character(c) if cmd && c.as_str().eq_ignore_ascii_case("o") => {
                self.plat.open(&sel)
            }
            _ => {}
        }
        self.apply(Outcome::Rescan);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let first = self.scene.is_none();
        self.create_window(el);
        if self.window.is_none() {
            return;
        }
        if first {
            catalog::write_static(DUCKS_PNG, HATS_PNG);
            self.rescan(true);
            self.take_over_desktop();
            if !self.settings.state.autostart_initialised
                && !demo()
                && !dev_build()
                && !std::env::args().any(|a| a == "--desktop-test")
            {
                autostart::set(self.settings.open_at_login);
                self.settings.state.autostart_initialised = true;
                self.settings.save();
            }
            self.build_tray();
        }
        self.last = Instant::now();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.exiting = true;
                el.exit();
            }
            WindowEvent::Destroyed => {
                // Windows: Explorer restarted and took our parent with it.
                #[cfg(not(windows))]
                {
                    self.pixels = None;
                    self.labels = None;
                }
                self.window = None;
            }
            WindowEvent::RedrawRequested => {
                #[cfg(windows)]
                if let (Some(scene), Some(window)) = (self.scene.as_mut(), self.window.as_ref()) {
                    self.gdi_frame.resize((scene.w * scene.h * 4) as usize, 0);
                    scene.hires_tags = true;
                    scene.render(&mut self.gdi_frame);
                    if let Err(e) = platform::paint_pond(
                        window,
                        &mut self.gdi_frame,
                        scene.w,
                        scene.h,
                        self.factor,
                        scene.hires_tag_targets(),
                    ) {
                        // one bad frame (e.g. mid display change) shouldn't kill the pond
                        eprintln!("paint: {e}");
                    }
                    if std::env::args().any(|a| a == "--desktop-test")
                        && self.started.elapsed() > Duration::from_secs(6)
                    {
                        let report = format!(
                            "{}ducks: {}\nart: {}x{}, scale: {}\noccluded: {}\n",
                            platform::desktop_report(
                                window,
                                &self.gdi_frame,
                                scene.w,
                                scene.h,
                                self.factor
                            ),
                            scene.ducks.len(),
                            scene.w,
                            scene.h,
                            self.factor,
                            self.occluded
                        );
                        let mut rgba = self.gdi_frame.clone();
                        for p in rgba.chunks_exact_mut(4) {
                            p.swap(0, 2);
                        }
                        save_png(
                            &std::env::temp_dir()
                                .join("duckpond-desktop-test.png")
                                .to_string_lossy(),
                            scene.w,
                            scene.h,
                            &rgba,
                            2,
                        );
                        let path = std::env::temp_dir().join("duckpond-desktop-test.txt");
                        std::fs::write(&path, &report).expect("write desktop integration report");
                        eprintln!("{report}report: {}", path.display());
                        self.exiting = true;
                        el.exit();
                    }
                }
                #[cfg(not(windows))]
                {
                    let App {
                        pixels,
                        scene,
                        labels,
                        window,
                        factor,
                        ..
                    } = self;
                    if let (Some(px), Some(scene), Some(window)) =
                        (pixels.as_mut(), scene.as_mut(), window.as_ref())
                    {
                        scene.hires_tags = labels.is_some();
                        scene.render(px.frame_mut());
                        let size = window.inner_size();
                        if let Some(lb) = labels.as_mut() {
                            let f = *factor as f32;
                            let ox = (size.width as f32 - scene.w as f32 * f) / 2.0;
                            let oy = (size.height as f32 - scene.h as f32 * f) / 2.0;
                            let gap = 3.0 * window.scale_factor() as f32;
                            let tags: Vec<labels::Tag> = scene
                                .hires_tag_targets()
                                .into_iter()
                                .map(|(text, x, top)| labels::Tag {
                                    text,
                                    x: ox + x * f,
                                    bottom: oy + top * f - gap,
                                })
                                .collect();
                            lb.prepare(px, &tags, (size.width, size.height));
                        }
                        let lb = labels.as_ref();
                        let res = px.render_with(|enc, target, ctx| {
                            ctx.scaling_renderer.render(enc, target);
                            if let Some(lb) = lb {
                                lb.draw(enc, target);
                            }
                            Ok(())
                        });
                        if let Err(e) = res {
                            eprintln!("render: {e}");
                        }
                    }
                }
            }
            WindowEvent::Resized(size) => {
                if size.width == 0 || size.height == 0 {
                    return;
                }
                let (aw, ah) = self.art_size(size.width, size.height);
                #[cfg(windows)]
                if let Some(s) = &mut self.scene {
                    if s.w != aw || s.h != ah {
                        s.resize(aw, ah, &self.pond, &self.land);
                    }
                }
                #[cfg(not(windows))]
                if let Some(px) = &mut self.pixels {
                    let _ = px.resize_surface(size.width.max(1), size.height.max(1));
                    if let Some(s) = &mut self.scene {
                        if s.w != aw || s.h != ah {
                            let _ = px.resize_buffer(aw, ah);
                            s.resize(aw, ah, &self.pond, &self.land);
                        }
                    }
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.plat.set_scale(scale_factor);
                #[cfg(not(windows))]
                if let Some(l) = &mut self.labels {
                    l.set_scale(scale_factor);
                }
            }
            WindowEvent::Occluded(o) => self.occluded = o,
            WindowEvent::ModifiersChanged(m) => {
                self.mods = m.state();
                let held = self.mods.alt_key();
                if let Some(s) = &mut self.scene {
                    s.show_labels = self.settings.show_labels || held;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                #[cfg(windows)]
                {
                    let (Some(window), Some(scene)) = (&self.window, &self.scene) else {
                        return;
                    };
                    let size = window.inner_size();
                    let ox = (size.width - scene.w * self.factor) as f32 / 2.0;
                    let oy = (size.height - scene.h * self.factor) as f32 / 2.0;
                    self.cursor_art = (
                        ((position.x as f32 - ox) / self.factor as f32)
                            .floor()
                            .clamp(0.0, scene.w as f32 - 1.0)
                            + 0.5,
                        ((position.y as f32 - oy) / self.factor as f32)
                            .floor()
                            .clamp(0.0, scene.h as f32 - 1.0)
                            + 0.5,
                    );
                }
                #[cfg(not(windows))]
                {
                    let Some(px) = &self.pixels else { return };
                    let (x, y) = px
                        .window_pos_to_pixel((position.x as f32, position.y as f32))
                        .unwrap_or_else(|p| px.clamp_pixel_pos(p));
                    self.cursor_art = (x as f32 + 0.5, y as f32 + 0.5);
                }
                let Some(scene) = self.scene.as_mut() else {
                    return;
                };
                if let Some(p) = &mut self.press {
                    if let Some(i) = p.duck {
                        if !p.dragging
                            && (p.at.0 - self.cursor_art.0).hypot(p.at.1 - self.cursor_art.1) > 2.5
                        {
                            p.dragging = true;
                            scene.grab(i);
                        }
                        if p.dragging {
                            scene.drag_to(i, self.cursor_art.0, self.cursor_art.1);
                        }
                    }
                }
                scene.hover = scene.hit(self.cursor_art.0, self.cursor_art.1);
            }
            WindowEvent::CursorLeft { .. } => {
                if let Some(s) = &mut self.scene {
                    s.hover = None;
                }
            }
            WindowEvent::MouseInput { state, button, .. } => self.on_mouse(button, state),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    self.on_key(&event.logical_key);
                }
            }
            WindowEvent::DroppedFile(path) => {
                if let Some(key) = self.plat.import_drop(&path) {
                    let f = self.factor as f64;
                    let (x, y) = self.cursor_art;
                    self.settings
                        .state
                        .homes
                        .insert(key, (x as f64 * f, y as f64 * f));
                    self.settings.save();
                    self.apply(Outcome::Rescan);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if STOP.load(Ordering::Relaxed) {
            self.exiting = true;
            el.exit();
            return;
        }
        if self.window.is_none() && !self.exiting {
            self.create_window(el);
            if self.window.is_some() {
                if self.paused {
                    set_window_visible(self.window.as_ref().unwrap(), false);
                } else {
                    self.take_over_desktop();
                }
                self.rescan(false);
            }
        }
        self.handle_tray(el);
        let now = Instant::now();
        if let Some(out) = snapshot_path() {
            self.snapshot_step(el, &out);
        }

        if now >= self.next_clock {
            self.next_clock = now + Duration::from_secs(20);
            let hour = std::env::var("DUCKPOND_HOUR")
                .ok()
                .and_then(|h| h.parse::<f32>().ok())
                .unwrap_or_else(|| {
                    use chrono::Timelike;
                    let t = chrono::Local::now();
                    t.hour() as f32 + t.minute() as f32 / 60.0
                });
            // day & night off: always a soft afternoon
            let hour = if self.settings.day_night { hour } else { 14.0 };
            if let Some(s) = &mut self.scene {
                s.set_hour(hour);
            }
        }
        if now >= self.next_prefs_check {
            self.next_prefs_check = now + Duration::from_millis(700);
            if self.settings.reload_if_changed() {
                self.reapply_prefs();
            }
        }
        if now >= self.next_scan && !self.paused {
            self.rescan(false);
        }
        if let Some((t, at)) = self.pending_food {
            if now >= t {
                self.pending_food = None;
                if let Some(s) = &mut self.scene {
                    s.drop_food(at.0, at.1);
                }
            }
        }

        if now >= self.next_frame && !self.paused {
            let dt = now.duration_since(self.last).as_secs_f32().min(0.1);
            self.last = now;
            if let Some(s) = &mut self.scene {
                s.update(dt);
            }
            self.play_sounds();
            if !self.occluded {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            let busy =
                self.scene.as_ref().map(|s| s.busy()).unwrap_or(false) || self.press.is_some();
            let fps = if self.occluded {
                2.0
            } else if busy {
                30.0
            } else {
                12.0
            };
            self.next_frame = now + Duration::from_secs_f64(1.0 / fps);
        }
        let wake = if self.paused {
            now + Duration::from_millis(250)
        } else {
            self.next_frame.min(self.next_scan)
        };
        el.set_control_flow(ControlFlow::WaitUntil(wake));
    }

    fn exiting(&mut self, _el: &ActiveEventLoop) {
        // dev loop restarts (SIGTERM + DUCKPOND_KEEP_HIDDEN) skip the Finder
        // restore so Finder doesn't flicker on every rebuild
        let dev_restart =
            STOP.load(Ordering::Relaxed) && std::env::var_os("DUCKPOND_KEEP_HIDDEN").is_some();
        if !self.paused && !demo() && !dev_restart {
            self.plat.set_native_icons(true);
        }
        self.settings.save();
    }
}

const LOCK_ADDR: &str = "127.0.0.1:47817";

/// `DuckPond --quit`: ask a running pond to close (it restores the icons).
fn quit_running() -> bool {
    use std::io::Write;
    match std::net::TcpStream::connect(LOCK_ADDR) {
        Ok(mut s) => s.write_all(b"quit").is_ok(),
        Err(_) => false,
    }
}

/// `DuckPond --selftest`: exercise everything that doesn't need a screen
/// (desktop listing, icon positions, simulation, drawing) and report.
fn selftest() {
    let dir = std::env::temp_dir();
    let mut log = String::new();
    let mut say = |s: String| {
        println!("{s}");
        log.push_str(&s);
        log.push('\n');
    };
    say(format!(
        "Duck Pond self-test ({} {})",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    let mut plat = Plat::new(1.0);
    say(format!(
        "native desktop icons visible: {}",
        plat.native_icons_visible()
    ));
    let items = plat.scan().unwrap_or_default();
    say(format!("desktop items found: {}", items.len()));
    let snap = plat.snapshot_positions();
    say(format!("icon positions read: {}", snap.len()));
    for it in items.iter().take(5) {
        say(format!(
            "  - {} ({:?}) at {:?}",
            it.name,
            it.kind,
            it.pos.or_else(|| snap.get(&it.key).copied())
        ));
    }
    let settings = Settings::default();
    let (w, h) = (600, 380);
    let mut scene = Scene::new(
        w,
        h,
        &Img::decode(POND_PNG),
        &Img::decode(LAND_PNG),
        Img::decode(DUCKS_PNG),
        Img::decode(HATS_PNG),
    );
    let adults = settings.adult_keys(&items);
    let mut rng = scene::Rng::new(7);
    for it in &items {
        let home = it
            .pos
            .map(|(x, y)| ((x / 6.0) as f32, (y / 6.0) as f32))
            .unwrap_or_else(|| (rng.range(20.0, 580.0), rng.range(30.0, 370.0)));
        let h = scene::hash(&it.name);
        let breed = settings.breed_for(it.kind, &it.key, h);
        let hat = settings.hat_for(it.kind, &it.key, h);
        scene.add_duck(
            it.key.clone(),
            it.name.clone(),
            it.kind,
            !adults.contains(&it.key),
            breed,
            hat,
            home,
            0.0,
        );
    }
    if items.is_empty() {
        for k in 0..20 {
            let name = format!("test duck {k}");
            scene.add_duck(
                name.clone(),
                name,
                items::Kind::Image,
                k % 4 != 0,
                k % 2,
                None,
                (rng.range(20.0, 580.0), rng.range(30.0, 370.0)),
                0.0,
            );
        }
    }
    scene.relax_homes();
    scene.drop_food(300.0, 190.0);
    let t0 = Instant::now();
    for _ in 0..300 {
        scene.update(1.0 / 30.0);
    }
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let frames = 60;
    let r0 = Instant::now();
    for _ in 0..frames {
        scene.update(1.0 / 30.0);
        scene.render(&mut buf);
    }
    say(format!(
        "simulated 360 ticks, rendered {frames} frames: {:.2} ms/frame",
        r0.elapsed().as_secs_f64() * 1000.0 / frames as f64
    ));
    let _ = t0;
    let png = dir.join("duckpond-selftest.png");
    save_png(&png.to_string_lossy(), w, h, &buf, 2);
    say(format!("picture: {}", png.display()));
    let report = dir.join("duckpond-selftest.txt");
    let _ = std::fs::write(&report, &log);
    println!("report: {}", report.display());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    #[cfg(windows)]
    if args.iter().any(|a| a.starts_with("--")) {
        // GUI-subsystem exe: borrow the terminal we were started from, if any
        unsafe {
            let _ = windows::Win32::System::Console::AttachConsole(
                windows::Win32::System::Console::ATTACH_PARENT_PROCESS,
            );
        }
    }
    if args.iter().any(|a| a == "--quit") {
        if quit_running() {
            // give it a moment to put the icons back
            std::thread::sleep(Duration::from_millis(1500));
        }
        return;
    }
    if args.iter().any(|a| a == "--restore") {
        platform::emergency_restore();
        return;
    }
    if args.iter().any(|a| a == "--selftest") {
        selftest();
        return;
    }

    // one pond at a time; the lock socket also takes `--quit` requests
    let _lock = match std::net::TcpListener::bind(LOCK_ADDR) {
        Ok(l) => {
            if let Ok(l2) = l.try_clone() {
                std::thread::spawn(move || {
                    use std::io::Read;
                    for s in l2.incoming().flatten() {
                        let mut s = s;
                        let mut buf = [0u8; 8];
                        if let Ok(n) = s.read(&mut buf) {
                            if &buf[..n] == b"quit" {
                                STOP.store(true, Ordering::Relaxed);
                            }
                        }
                    }
                });
            }
            Some(l)
        }
        Err(_) if snapshot_path().is_some() => None,
        Err(_) => return,
    };
    let _ = ctrlc::set_handler(|| STOP.store(true, Ordering::Relaxed));
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        platform::emergency_restore();
        default_hook(info);
    }));

    #[allow(unused_mut)]
    let mut builder = EventLoop::builder();
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        builder
            .with_activation_policy(ActivationPolicy::Accessory)
            .with_default_menu(false);
    }
    let el = builder.build().expect("event loop");
    let mut app = App::new();
    let _ = el.run_app(&mut app);
}
