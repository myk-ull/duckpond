//! Desktop items, their kinds, and the remembered "home" positions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Folder,
    Image,
    Video,
    Audio,
    Pdf,
    Doc,
    Code,
    Archive,
    App,
    Alias,
    Other,
}

/// One icon on the desktop.
#[derive(Clone, Debug)]
pub struct Item {
    /// Stable identity: a filesystem path, or a shell parsing name on Windows.
    pub key: String,
    pub name: String,
    pub kind: Kind,
    /// File size in bytes (0 for folders / virtual items).
    pub size: u64,
    pub is_dir: bool,
    /// Where the native icon sits, in physical pixels relative to the pond
    /// window, when the platform can tell us.
    pub pos: Option<(f64, f64)>,
}

impl Kind {
    pub const ALL: [Kind; 11] = [
        Kind::Folder,
        Kind::Image,
        Kind::Video,
        Kind::Audio,
        Kind::Pdf,
        Kind::Doc,
        Kind::Code,
        Kind::Archive,
        Kind::App,
        Kind::Alias,
        Kind::Other,
    ];
    pub fn key(self) -> &'static str {
        match self {
            Kind::Folder => "folder",
            Kind::Image => "image",
            Kind::Video => "video",
            Kind::Audio => "audio",
            Kind::Pdf => "pdf",
            Kind::Doc => "doc",
            Kind::Code => "code",
            Kind::Archive => "archive",
            Kind::App => "app",
            Kind::Alias => "alias",
            Kind::Other => "other",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Kind::Folder => "Folders",
            Kind::Image => "Images & screenshots",
            Kind::Video => "Videos & recordings",
            Kind::Audio => "Audio",
            Kind::Pdf => "PDFs",
            Kind::Doc => "Documents & notes",
            Kind::Code => "Code",
            Kind::Archive => "Archives",
            Kind::App => "Apps",
            Kind::Alias => "Aliases & shortcuts",
            Kind::Other => "Everything else",
        }
    }
}

/// "mix" means: pick from the ticked breeds, by filename.
pub const MIX: &str = "mix";

fn default_mix() -> Vec<String> {
    vec!["pekin".into(), "buttercup".into()]
}

fn default_kind_breeds() -> HashMap<String, String> {
    [
        ("folder", "pekin"),
        ("app", "pekin"),
        ("pdf", "buttercup"),
        ("doc", "buttercup"),
        ("code", "buttercup"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

pub fn kind_for(name: &str, is_dir: bool, is_link: bool) -> Kind {
    let lower = name.to_ascii_lowercase();
    let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    if ext == "app" || ext == "exe" || ext == "msi" {
        return Kind::App;
    }
    if is_link || ext == "lnk" || ext == "url" || ext == "webloc" || ext == "alias" {
        return Kind::Alias;
    }
    if is_dir {
        return Kind::Folder;
    }
    match ext {
        "png" | "jpg" | "jpeg" | "gif" | "heic" | "webp" | "bmp" | "tiff" | "tif" | "svg"
        | "psd" | "raw" => Kind::Image,
        "mov" | "mp4" | "m4v" | "avi" | "mkv" | "webm" | "mts" => Kind::Video,
        "mp3" | "wav" | "aiff" | "aif" | "m4a" | "flac" | "ogg" | "aac" => Kind::Audio,
        "pdf" => Kind::Pdf,
        "md" | "txt" | "doc" | "docx" | "rtf" | "pages" | "odt" | "tex" | "csv" | "xlsx"
        | "xls" | "key" | "ppt" | "pptx" | "numbers" => Kind::Doc,
        "py" | "js" | "ts" | "tsx" | "jsx" | "rs" | "go" | "c" | "cpp" | "h" | "java" | "rb"
        | "sh" | "json" | "html" | "css" | "swift" | "kt" | "ipynb" | "toml" | "yaml" | "yml"
        | "sql" => Kind::Code,
        "zip" | "tar" | "gz" | "tgz" | "rar" | "7z" | "dmg" | "pkg" | "iso" | "xz" | "bz2" => {
            Kind::Archive
        }
        _ => Kind::Other,
    }
}

pub fn config_dir() -> PathBuf {
    let d = std::env::var_os("DUCKPOND_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::config_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("DuckPond")
        });
    let _ = std::fs::create_dir_all(&d);
    d
}

/// "automatic" in a per-file override means: use the file-type rule.
pub const AUTO: &str = "auto";
/// A hat choice meaning: bare-headed.
pub const NO_HAT: &str = "none";

/// The hat each file type wears by default (when hats are switched on).
pub fn default_kind_hats() -> HashMap<String, String> {
    [
        ("folder", "straw"),
        ("image", "beret"),
        ("video", "cap"),
        ("audio", "headphones"),
        ("pdf", "mortar"),
        ("doc", "glasses"),
        ("code", "propeller"),
        ("archive", "party"),
        ("app", "crown"),
        ("alias", "bow"),
        ("other", "flower"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

/// Per-file choices from the Preferences window. Missing / "auto" fields
/// fall back to the file-type rules.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FilePref {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breed: Option<String>,
    /// "duckling" | "adult"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age: Option<String>,
    /// a hat id, or "none"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hat: Option<String>,
}

fn yes() -> bool {
    true
}
fn default_volume() -> f32 {
    0.6
}
fn default_share() -> f32 {
    0.8
}

/// Everything the user chooses. Lives in preferences.json, which the
/// Preferences window edits; the pond notices changes and applies them live.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prefs {
    #[serde(default)]
    pub version: u32,
    #[serde(default = "yes")]
    pub open_at_login: bool,
    #[serde(default)]
    pub muted: bool,
    #[serde(default = "default_volume")]
    pub volume: f32,
    #[serde(default)]
    pub show_labels: bool,
    #[serde(default)]
    pub show_hats: bool,
    /// tint the pond with the time of day (golden hour, night + fireflies)
    #[serde(default = "yes")]
    pub day_night: bool,
    /// breeds in the mix
    #[serde(default = "default_mix")]
    pub mix: Vec<String>,
    /// file kind -> breed id, or "mix"
    #[serde(default = "default_kind_breeds")]
    pub kind_breeds: HashMap<String, String>,
    /// file kind -> hat id, or "none"
    #[serde(default = "default_kind_hats")]
    pub kind_hats: HashMap<String, String>,
    /// share of the flock that are ducklings (0.0 - 0.95)
    #[serde(default = "default_share")]
    pub duckling_share: f32,
    /// per-file overrides, keyed by item key (path)
    #[serde(default)]
    pub files: HashMap<String, FilePref>,
}

const PREFS_VERSION: u32 = 3;

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            version: PREFS_VERSION,
            open_at_login: true,
            muted: false,
            volume: default_volume(),
            show_labels: false,
            show_hats: false,
            day_night: true,
            mix: default_mix(),
            kind_breeds: default_kind_breeds(),
            kind_hats: default_kind_hats(),
            duckling_share: default_share(),
            files: HashMap::new(),
        }
    }
}

fn mix_hash(hash: u64) -> u64 {
    // splitmix finaliser: similar names ("Screenshot 2026-…") still spread evenly
    let mut z = hash.wrapping_add(0x9e3779b97f4a7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

impl Prefs {
    fn file(&self, key: &str) -> Option<&FilePref> {
        self.files.get(key)
    }

    /// Which breed (index into sprite_meta::BREEDS) this file hatches as.
    pub fn breed_for(&self, kind: Kind, key: &str, hash: u64) -> usize {
        let names = crate::sprite_meta::BREEDS;
        let idx = |b: &str| names.iter().position(|n| *n == b);
        if let Some(b) = self.file(key).and_then(|f| f.breed.as_deref()) {
            if let Some(i) = idx(b) {
                return i;
            }
        }
        let choice = self.kind_breeds.get(kind.key()).map(|s| s.as_str()).unwrap_or(MIX);
        if choice != MIX {
            if let Some(i) = idx(choice) {
                return i;
            }
        }
        let pool: Vec<usize> = self.mix.iter().filter_map(|b| idx(b)).collect();
        if pool.is_empty() {
            return 0;
        }
        pool[(mix_hash(hash) % pool.len() as u64) as usize]
    }

    /// Which hat (index into sprite_meta::HATS) this file wears, if any.
    pub fn hat_for(&self, kind: Kind, key: &str, hash: u64) -> Option<usize> {
        let hats = crate::sprite_meta::HATS;
        let pick = match self.file(key).and_then(|f| f.hat.as_deref()) {
            Some(h) if h != AUTO => h.to_string(),
            _ => {
                let h = self.kind_hats.get(kind.key()).cloned().unwrap_or_else(|| NO_HAT.into());
                // screenshots: every third beret is navy, for variety
                if h == "beret" && hash % 3 == 0 { "beret_navy".into() } else { h }
            }
        };
        hats.iter().position(|h| h.name == pick)
    }

    /// Which items hatch as grown-ups. Per-file choices win; then folders and
    /// apps (the parents), then the biggest files, until the flock has the
    /// chosen share of ducklings.
    pub fn adult_keys(&self, items: &[Item]) -> std::collections::HashSet<String> {
        use std::collections::HashSet;
        let share = self.duckling_share.clamp(0.0, 0.95) as f64;
        let target = ((items.len() as f64) * (1.0 - share)).ceil() as usize;
        let age = |i: &Item| self.file(&i.key).and_then(|f| f.age.as_deref()).filter(|a| *a != AUTO);
        let mut adults: HashSet<String> =
            items.iter().filter(|i| age(i) == Some("adult")).map(|i| i.key.clone()).collect();
        let free: Vec<&Item> = items.iter().filter(|i| age(i).is_none()).collect();
        for i in free.iter().filter(|i| i.is_dir || i.kind == Kind::App) {
            if adults.len() >= target.max(1) {
                break;
            }
            adults.insert(i.key.clone());
        }
        let mut files: Vec<&&Item> = free.iter().filter(|i| !adults.contains(&i.key)).collect();
        files.sort_by(|a, b| b.size.cmp(&a.size));
        for f in files {
            if adults.len() >= target {
                break;
            }
            adults.insert(f.key.clone());
        }
        adults
    }
}

/// Runtime state the pond keeps for itself (state.json): never touched by
/// the Preferences window, so it can't clobber where ducks live.
#[derive(Default, Serialize, Deserialize)]
pub struct State {
    /// Home positions in physical pixels, keyed by item key.
    #[serde(default)]
    pub homes: HashMap<String, (f64, f64)>,
    #[serde(default)]
    pub autostart_initialised: bool,
}

#[derive(Default)]
pub struct Settings {
    pub prefs: Prefs,
    pub state: State,
    /// mtime of preferences.json as we last wrote / read it
    prefs_stamp: Option<std::time::SystemTime>,
}

impl std::ops::Deref for Settings {
    type Target = Prefs;
    fn deref(&self) -> &Prefs {
        &self.prefs
    }
}
impl std::ops::DerefMut for Settings {
    fn deref_mut(&mut self) -> &mut Prefs {
        &mut self.prefs
    }
}

fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> bool {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).is_ok() && std::fs::rename(tmp, path).is_ok()
}

fn mtime(p: &std::path::Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

impl Settings {
    pub fn prefs_path() -> PathBuf {
        config_dir().join("preferences.json")
    }
    fn state_path() -> PathBuf {
        config_dir().join("state.json")
    }

    pub fn load() -> Settings {
        let pp = Self::prefs_path();
        let read = |p: &std::path::Path| std::fs::read(p).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
        let mut prefs: Prefs = read(&pp).and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        let mut state: State = read(&Self::state_path()).and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        let mut migrated = false;
        // older builds kept everything in settings.json
        if !pp.exists() {
            if let Some(old) = read(&config_dir().join("settings.json")) {
                if let Ok(s) = serde_json::from_value::<State>(old.clone()) {
                    if state.homes.is_empty() {
                        state = s;
                    }
                }
                for k in ["muted", "show_labels", "show_hats"] {
                    if let Some(b) = old.get(k).and_then(|v| v.as_bool()) {
                        match k {
                            "muted" => prefs.muted = b,
                            "show_labels" => prefs.show_labels = b,
                            _ => prefs.show_hats = b,
                        }
                    }
                }
                if old.get("version").and_then(|v| v.as_u64()).unwrap_or(0) >= 2 {
                    if let Some(m) = old.get("mix").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                        prefs.mix = m;
                    }
                    if let Some(m) = old.get("kind_breeds").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                        prefs.kind_breeds = m;
                    }
                }
            }
            migrated = true;
        }
        if prefs.version < PREFS_VERSION {
            prefs.version = PREFS_VERSION;
            migrated = true;
        }
        let mut s = Settings { prefs, state, prefs_stamp: None };
        if migrated {
            s.save();
        }
        s.prefs_stamp = mtime(&pp);
        s
    }

    pub fn save(&mut self) {
        self.save_state();
        self.save_prefs();
    }

    pub fn save_state(&self) {
        if let Ok(b) = serde_json::to_vec_pretty(&self.state) {
            write_atomic(&Self::state_path(), &b);
        }
    }

    pub fn save_prefs(&mut self) {
        if let Ok(b) = serde_json::to_vec_pretty(&self.prefs) {
            if write_atomic(&Self::prefs_path(), &b) {
                self.prefs_stamp = mtime(&Self::prefs_path());
            }
        }
    }

    /// Pick up edits made by the Preferences window. True if anything changed.
    pub fn reload_if_changed(&mut self) -> bool {
        let p = Self::prefs_path();
        let now = mtime(&p);
        if now.is_none() || now == self.prefs_stamp {
            return false;
        }
        self.prefs_stamp = now;
        let Some(new) = std::fs::read(&p).ok().and_then(|b| serde_json::from_slice::<Prefs>(&b).ok()) else {
            return false;
        };
        if new == self.prefs {
            return false;
        }
        self.prefs = new;
        true
    }
}
