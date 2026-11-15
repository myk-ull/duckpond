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
}

#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "macos")]
pub use mac::*;

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::*;
