//! Launch at login. macOS: a per-user LaunchAgent (restarts the pond if it
//! ever crashes). Windows: the HKCU Run key.

#[cfg(target_os = "macos")]
mod imp {
    use std::path::PathBuf;
    const LABEL: &str = "com.duckpond.app";

    fn plist() -> PathBuf {
        dirs::home_dir()
            .unwrap()
            .join("Library/LaunchAgents")
            .join(format!("{LABEL}.plist"))
    }

    pub fn is_enabled() -> bool {
        plist().exists()
    }

    pub fn set(enabled: bool) {
        let p = plist();
        if !enabled {
            let _ = std::fs::remove_file(p);
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let exe = exe
            .to_string_lossy()
            .replace('&', "&amp;")
            .replace('<', "&lt;");
        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>{LABEL}</string>
    <key>ProgramArguments</key><array><string>{exe}</string></array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
    <key>ProcessType</key><string>Interactive</string>
    <key>LimitLoadToSessionType</key><string>Aqua</string>
</dict>
</plist>
"#
        );
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, body);
    }
}

#[cfg(windows)]
mod imp {
    use windows::core::{w, PCWSTR};
    use windows::Win32::System::Registry::*;

    const KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    const NAME: PCWSTR = w!("DuckPond");

    pub fn is_enabled() -> bool {
        unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                KEY,
                NAME,
                RRF_RT_REG_SZ,
                None,
                None,
                None,
            )
            .is_ok()
        }
    }

    pub fn set(enabled: bool) {
        unsafe {
            if !enabled {
                let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, KEY, NAME);
                return;
            }
            let Ok(exe) = std::env::current_exe() else {
                return;
            };
            let val: Vec<u16> = format!("\"{}\"", exe.display())
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let _ = RegSetKeyValueW(
                HKEY_CURRENT_USER,
                KEY,
                NAME,
                REG_SZ.0,
                Some(val.as_ptr() as *const _),
                (val.len() * 2) as u32,
            );
        }
    }
}

pub use imp::{is_enabled, set};
