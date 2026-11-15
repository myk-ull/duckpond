//! Windows: the pond window lives inside the desktop (a child of the window
//! that hosts Explorer's icon view, stacked above it), Explorer's icons are
//! hidden with the same flag as "View → Show desktop icons", and every duck
//! gets the real Explorer context menu.
#![allow(unused_unsafe)]

use super::{check_new_name, edit_part, NameCheck, Outcome};
use crate::items::{kind_for, Item};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use windows::core::{w, Interface, HSTRING, PCSTR, PCWSTR, PSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Shell::Common::{ITEMIDLIST, STRRET};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::*;
use winit::window::Window;

const OUR_FIRST: u32 = 0x8000;
// shell attribute / invoke flags (raw values from shobjidl.h)
const SFGAO_LINK: u32 = 0x0001_0000;
const SFGAO_FOLDER: u32 = 0x2000_0000;
const SFGAO_FILESYSTEM: u32 = 0x4000_0000;
const CMIC_MASK_UNICODE: u32 = 0x0000_4000;
const CMIC_MASK_PTINVOKE_RAW: u32 = 0x2000_0000;
const ID_HOME: u32 = OUR_FIRST + 1;
const ID_FEED: u32 = OUR_FIRST + 2;
const ID_LABELS: u32 = OUR_FIRST + 3;
const ID_PREFS: u32 = OUR_FIRST + 4;
const ID_CUSTOMISE: u32 = OUR_FIRST + 5;
const ID_DUP: u32 = OUR_FIRST + 6;
/// the shell's own "rename" verb, handled by us (its in-place editor lives
/// in the hidden icon view)
const ID_RENAME: u32 = OUR_FIRST + 7;

thread_local! {
    /// The shell menu currently on screen, so the helper window can forward
    /// owner-draw / submenu messages (needed for "Open with", "Send to", "New").
    static ACTIVE_MENU: RefCell<Option<IContextMenu>> = const { RefCell::new(None) };
}

struct Desk {
    view: IShellView,
    fv: IFolderView2,
    defview: HWND,
}

pub struct Plat {
    desk: Option<Desk>,
    origin: (i32, i32),
    helper: HWND,
    last_listing: Vec<String>,
    visible_virtuals: HashSet<String>,
    /// finished renames / duplicates, for the pond to act on
    done: RefCell<Vec<Outcome>>,
}

fn desktop_view() -> Option<Desk> {
    unsafe {
        let sw: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_ALL).ok()?;
        let loc = VARIANT::from(CSIDL_DESKTOP as i32);
        let root = VARIANT::default();
        let mut hwnd = 0i32;
        let disp = sw
            .FindWindowSW(&loc, &root, SWC_DESKTOP, &mut hwnd, SWFO_NEEDDISPATCH)
            .ok()?;
        let sp: IServiceProvider = disp.cast().ok()?;
        let browser: IShellBrowser = sp.QueryService(&SID_STopLevelBrowser).ok()?;
        let view = browser.QueryActiveShellView().ok()?;
        let fv: IFolderView2 = view.cast().ok()?;
        let defview = view.GetWindow().ok()?;
        Some(Desk { view, fv, defview })
    }
}

fn hwnd_of(w: &Window) -> Option<HWND> {
    match w.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)),
        _ => None,
    }
}

pub fn set_pond_visible(window: &Window, visible: bool) {
    // winit's set_visible rewrites all styles and removes WS_EX_LAYERED.
    unsafe {
        let _ = ShowWindow(
            hwnd_of(window).unwrap(),
            if visible { SW_SHOWNOACTIVATE } else { SW_HIDE },
        );
    }
}

/// GDI redirects this layered desktop child through DWM. No DXGI swapchain
/// is created for the cross-process Explorer child (invisible on Windows 11).
pub fn paint_pond(
    window: &Window,
    rgba: &mut [u8],
    w: u32,
    h: u32,
    factor: u32,
    tags: Vec<(String, f32, f32)>,
) -> windows::core::Result<()> {
    let hwnd = hwnd_of(window).expect("Windows window handle");
    let size = window.inner_size();
    let ox = (size.width - w * factor) as i32 / 2;
    let oy = (size.height - h * factor) as i32 / 2;
    // Scene redraws the entire buffer each frame; a top-down 32-bit DIB is BGRA.
    for p in rgba.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    unsafe {
        let dc = GetDC(Some(hwnd));
        if dc.is_invalid() {
            return Err(windows::core::Error::from_win32());
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        // Clear only the narrow integer-scaling margins; clearing the whole
        // surface before each blit would flash black between animation frames.
        for rect in [
            RECT {
                left: 0,
                top: 0,
                right: size.width as i32,
                bottom: oy,
            },
            RECT {
                left: 0,
                top: oy + (h * factor) as i32,
                right: size.width as i32,
                bottom: size.height as i32,
            },
            RECT {
                left: 0,
                top: 0,
                right: ox,
                bottom: size.height as i32,
            },
            RECT {
                left: ox + (w * factor) as i32,
                top: 0,
                right: size.width as i32,
                bottom: size.height as i32,
            },
        ] {
            FillRect(dc, &rect, HBRUSH(GetStockObject(BLACK_BRUSH).0));
        }
        SetStretchBltMode(dc, COLORONCOLOR);
        let lines = StretchDIBits(
            dc,
            ox,
            oy,
            (w * factor) as i32,
            (h * factor) as i32,
            0,
            0,
            w as i32,
            h as i32,
            Some(rgba.as_ptr().cast()),
            &info,
            DIB_RGB_COLORS,
            SRCCOPY,
        );
        let scale = window.scale_factor();
        let font = CreateFontW(
            -(13.0 * scale).round() as i32,
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32,
            w!("Segoe UI"),
        );
        let old = SelectObject(dc, HGDIOBJ(font.0));
        SetBkMode(dc, OPAQUE);
        SetBkColor(dc, COLORREF(0x00251e17));
        SetTextColor(dc, COLORREF(0x00eaf5fa));
        for (text, x, top) in tags {
            let mut text: Vec<u16> = text.encode_utf16().collect();
            let center = ox + (x * factor as f32) as i32;
            let bottom = oy + (top * factor as f32) as i32 - (3.0 * scale) as i32;
            let mut r = RECT {
                left: center - (160.0 * scale) as i32,
                right: center + (160.0 * scale) as i32,
                top: bottom - (20.0 * scale) as i32,
                bottom,
            };
            DrawTextW(
                dc,
                &mut text,
                &mut r,
                DT_CENTER | DT_SINGLELINE | DT_BOTTOM | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
        }
        SelectObject(dc, old);
        let _ = DeleteObject(HGDIOBJ(font.0));
        ReleaseDC(Some(hwnd), dc);

        if lines == 0 || lines == -1 {
            return Err(windows::core::Error::from_win32());
        }
    }
    Ok(())
}

/// Read back the actual GDI child surface, not the simulation's CPU output.
pub fn desktop_report(window: &Window, bgra: &[u8], w: u32, h: u32, factor: u32) -> String {
    unsafe {
        let hwnd = hwnd_of(window).unwrap();
        let parent = GetParent(hwnd).expect("desktop child parent");
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let mut class = [0u16; 128];
        let n = GetClassNameW(parent, &mut class);
        let parent_class = String::from_utf16_lossy(&class[..n as usize]);
        let size = window.inner_size();
        let ox = (size.width - w * factor) as i32 / 2;
        let oy = (size.height - h * factor) as i32 / 2;
        let dc = GetDC(Some(hwnd));
        let mut matches = 0;
        for gy in 1..=3 {
            for gx in 1..=3 {
                let x = w * gx / 4;
                let y = h * gy / 4;
                let i = ((y * w + x) * 4) as usize;
                let expected =
                    bgra[i + 2] as u32 | ((bgra[i + 1] as u32) << 8) | ((bgra[i] as u32) << 16);
                let actual = GetPixel(dc, ox + (x * factor) as i32, oy + (y * factor) as i32).0;
                if actual == expected {
                    matches += 1;
                }
            }
        }
        ReleaseDC(Some(hwnd), dc);
        let screen = GetDC(None);
        let mut screen_matches = 0;
        let mut exposed = 0;
        for gy in 1..20 {
            for gx in 1..20 {
                let x = w * gx / 20;
                let y = h * gy / 20;
                let mut pt = POINT {
                    x: ox + (x * factor) as i32,
                    y: oy + (y * factor) as i32,
                };
                let _ = ClientToScreen(hwnd, &mut pt);
                if WindowFromPoint(pt) != hwnd {
                    continue;
                }
                exposed += 1;
                let i = ((y * w + x) * 4) as usize;
                let expected =
                    bgra[i + 2] as u32 | ((bgra[i + 1] as u32) << 8) | ((bgra[i] as u32) << 16);
                if GetPixel(screen, pt.x, pt.y).0 == expected {
                    screen_matches += 1;
                }
            }
        }
        ReleaseDC(None, screen);
        let embedded =
            style & WS_CHILD.0 != 0 && (parent_class == "Progman" || parent_class == "WorkerW");
        let layered = ex & WS_EX_LAYERED.0 != 0;
        let visible = IsWindowVisible(hwnd).as_bool();
        let passed = embedded && layered && visible && matches == 9;
        format!("Desktop integration: {}\nparent: {parent_class}\nWS_CHILD: {}\nWS_EX_LAYERED: {layered}\nvisible: {visible}\nGDI readback: {matches}/9 matching samples\nScreen readback: {screen_matches}/{exposed} exposed samples match (other apps may cover the pond)\nstandalone fallback: disabled\n", if passed { "PASS" } else { "FAIL" }, style & WS_CHILD.0 != 0)
    }
}

fn strret(folder: &IShellFolder, pidl: *const ITEMIDLIST, flags: SHGDNF) -> Option<String> {
    unsafe {
        let mut sr = STRRET::default();
        folder.GetDisplayNameOf(pidl, flags, &mut sr).ok()?;
        let mut out = PWSTR::null();
        StrRetToStrW(&mut sr, Some(pidl), &mut out).ok()?;
        let s = out.to_string().ok();
        CoTaskMemFree(Some(out.0 as *const _));
        s
    }
}

fn parse_pidl(key: &str) -> Option<*mut ITEMIDLIST> {
    unsafe {
        let mut p: *mut ITEMIDLIST = std::ptr::null_mut();
        SHParseDisplayName(&HSTRING::from(key), None, &mut p, 0, None).ok()?;
        (!p.is_null()).then_some(p)
    }
}

pub fn emergency_restore() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
    if let Some(d) = desktop_view() {
        unsafe {
            let _ = d.fv.SetCurrentFolderFlags(FWF_NOICONS.0 as u32, 0);
        }
    }
}

unsafe extern "system" fn helper_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if matches!(
        msg,
        WM_INITMENUPOPUP | WM_DRAWITEM | WM_MEASUREITEM | WM_MENUCHAR
    ) {
        let r = ACTIVE_MENU.with(|c| {
            let cm = c.borrow().clone()?;
            unsafe {
                if let Ok(cm3) = cm.cast::<IContextMenu3>() {
                    let mut res = LRESULT(0);
                    if cm3.HandleMenuMsg2(msg, wp, lp, Some(&mut res)).is_ok() {
                        return Some(res);
                    }
                } else if let Ok(cm2) = cm.cast::<IContextMenu2>() {
                    if cm2.HandleMenuMsg(msg, wp, lp).is_ok() {
                        return Some(LRESULT(0));
                    }
                }
            }
            None
        });
        if let Some(r) = r {
            return r;
        }
    }
    unsafe { DefWindowProcW(h, msg, wp, lp) }
}

fn make_helper() -> HWND {
    unsafe {
        let hinst: HINSTANCE = GetModuleHandleW(None).map(|m| m.into()).unwrap_or_default();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(helper_proc),
            hInstance: hinst,
            lpszClassName: w!("DuckPondMenuHelper"),
            ..Default::default()
        };
        RegisterClassW(&wc);
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            w!("DuckPondMenuHelper"),
            w!(""),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinst),
            None,
        )
        .unwrap_or_default()
    }
}

fn desktop_dirs() -> Vec<PathBuf> {
    let mut v = vec![];
    if let Some(d) = dirs::desktop_dir() {
        v.push(d);
    }
    if let Ok(p) = std::env::var("PUBLIC") {
        v.push(PathBuf::from(p).join("Desktop"));
    }
    v
}

impl Plat {
    pub fn parent_handle(&mut self) -> RawWindowHandle {
        self.desk = desktop_view();
        let parent = self
            .desk
            .as_ref()
            .and_then(|d| unsafe { GetParent(d.defview).ok() })
            .expect("Explorer desktop unavailable; refusing a standalone pond window");
        RawWindowHandle::Win32(raw_window_handle::Win32WindowHandle::new(
            std::num::NonZeroIsize::new(parent.0 as isize).unwrap(),
        ))
    }

    pub fn new(_scale: f64) -> Plat {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        Plat {
            desk: desktop_view(),
            origin: (0, 0),
            helper: make_helper(),
            last_listing: vec![],
            visible_virtuals: HashSet::new(),
            done: RefCell::new(vec![]),
        }
    }

    pub fn set_scale(&mut self, _s: f64) {}

    fn desk(&mut self) -> Option<&Desk> {
        if self.desk.is_none() {
            self.desk = desktop_view();
        }
        self.desk.as_ref()
    }

    pub fn prepare_window(&mut self, window: &Window) {
        let h = hwnd_of(window).expect("Windows window handle");
        let pos = window
            .outer_position()
            .map(|p| (p.x, p.y))
            .unwrap_or((0, 0));
        let size = window.inner_size();
        self.origin = pos;
        // re-find the desktop view: Explorer may have restarted
        self.desk = desktop_view();
        self.last_listing.clear();
        let defview = self.desk.as_ref().map(|d| d.defview);
        unsafe {
            let parent = defview
                .and_then(|dv| GetParent(dv).ok())
                .expect("Explorer desktop unavailable; refusing a standalone pond window");
            {
                let style = GetWindowLongPtrW(h, GWL_STYLE);
                let style = (style
                    & !(WS_POPUP.0 as isize | WS_CAPTION.0 as isize | WS_THICKFRAME.0 as isize))
                    | WS_CHILD.0 as isize;
                SetWindowLongPtrW(h, GWL_STYLE, style);
                // Layered windows cannot use CS_OWNDC/CS_CLASSDC.
                let class_style = GetClassLongPtrW(h, GCL_STYLE);
                SetClassLongPtrW(
                    h,
                    GCL_STYLE,
                    (class_style & !((CS_OWNDC.0 | CS_CLASSDC.0) as usize)) as isize,
                );
                let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
                SetWindowLongPtrW(
                    h,
                    GWL_EXSTYLE,
                    (ex & !(WS_EX_APPWINDOW.0 as isize | WS_EX_NOREDIRECTIONBITMAP.0 as isize))
                        | WS_EX_LAYERED.0 as isize
                        | WS_EX_TOOLWINDOW.0 as isize,
                );
                let _ = SetParent(h, Some(parent));
                assert_eq!(
                    GetParent(h).ok(),
                    Some(parent),
                    "Could not attach pond to Explorer"
                );
                SetLayeredWindowAttributes(h, COLORREF(0), 255, LWA_ALPHA)
                    .expect("enable desktop child composition");
                let mut pt = POINT { x: pos.0, y: pos.1 };
                let _ = ScreenToClient(parent, &mut pt);
                SetWindowPos(
                    h,
                    Some(HWND_TOP),
                    pt.x,
                    pt.y,
                    size.width as i32,
                    size.height as i32,
                    SWP_FRAMECHANGED | SWP_NOACTIVATE,
                )
                .expect("position desktop pond");
            }
        }
    }

    pub fn native_icons_visible(&mut self) -> bool {
        match self.desk() {
            Some(d) => unsafe {
                d.fv.GetCurrentFolderFlags()
                    .map(|f| f & FWF_NOICONS.0 as u32 == 0)
                    .unwrap_or(true)
            },
            None => true,
        }
    }

    pub fn set_native_icons(&mut self, visible: bool) {
        if let Some(d) = self.desk() {
            unsafe {
                let _ = d.fv.SetCurrentFolderFlags(
                    FWF_NOICONS.0 as u32,
                    if visible { 0 } else { FWF_NOICONS.0 as u32 },
                );
            }
        }
    }

    fn enumerate(&mut self) -> Vec<Item> {
        let origin = self.origin;
        let visible_virtuals = self.visible_virtuals.clone();
        let roots = desktop_dirs();
        let Some(d) = self.desk() else { return vec![] };
        let mut items = vec![];
        let from_view;
        unsafe {
            let Ok(folder) =
                d.fv.GetFolder::<IShellFolder>()
                    .or_else(|_| SHGetDesktopFolder())
            else {
                return vec![];
            };
            let n = d.fv.ItemCount(SVGIO_ALLVIEW).unwrap_or(0);
            from_view = n > 0;
            let mut pidls = Vec::new();
            for i in 0..n {
                if let Ok(pidl) = d.fv.Item(i) {
                    pidls.push(pidl);
                }
            }
            // Hiding the native icon view can empty its displayed item list.
            // Enumerate the underlying shell folder as well, independently of
            // FWF_NOICONS; this also preserves virtual items such as Recycle Bin.
            if pidls.is_empty() {
                let mut enumeration = None;
                let _ = folder.EnumObjects(
                    d.defview,
                    (SHCONTF_FOLDERS.0 | SHCONTF_NONFOLDERS.0) as u32,
                    &mut enumeration,
                );
                if let Some(enumeration) = enumeration {
                    loop {
                        let mut next = [std::ptr::null_mut()];
                        let mut fetched = 0;
                        let status = enumeration.Next(&mut next, Some(&mut fetched));
                        if status.is_err() || fetched == 0 {
                            break;
                        }
                        pidls.push(next[0]);
                    }
                }
            }
            let mut spacing = POINT::default();
            let _ = d.fv.GetSpacing(&mut spacing);
            for pidl in pidls {
                let name = strret(&folder, pidl, SHGDN_NORMAL).unwrap_or_default();
                let key = strret(&folder, pidl, SHGDN_FORPARSING).unwrap_or_else(|| name.clone());
                let mut attrs = SFGAO_FOLDER | SFGAO_LINK | SFGAO_FILESYSTEM;
                let _ = folder.GetAttributesOf(&[pidl as *const _], &mut attrs);
                let pos = d.fv.GetItemPosition(pidl).ok().map(|mut pt| {
                    pt.x += spacing.x / 2;
                    pt.y += spacing.y / 3;
                    let _ = windows::Win32::Graphics::Gdi::ClientToScreen(d.defview, &mut pt);
                    ((pt.x - origin.0) as f64, (pt.y - origin.1) as f64)
                });
                CoTaskMemFree(Some(pidl as *const _));

                let path = Path::new(&key);
                if !from_view {
                    // The desktop namespace also contains hidden virtual icons
                    // and links to folders outside the physical desktop.
                    let on_desktop = path
                        .parent()
                        .map(|p| roots.iter().any(|r| p == r))
                        .unwrap_or(false);
                    if !on_desktop && !visible_virtuals.contains(&key) {
                        continue;
                    }
                }
                let meta = std::fs::metadata(path).ok();
                let is_dir = meta
                    .as_ref()
                    .map(|m| m.is_dir())
                    .unwrap_or(attrs & SFGAO_FOLDER != 0);
                let is_link = attrs & SFGAO_LINK != 0;
                let size = meta.as_ref().map(|m| m.len()).unwrap_or(u64::MAX);
                let file_name = path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or(name.clone());
                items.push(Item {
                    kind: kind_for(&file_name, is_dir, is_link),
                    size: if size == u64::MAX { 0 } else { size },
                    is_dir,
                    key,
                    name,
                    pos,
                });
            }
        }
        if from_view {
            self.visible_virtuals = items
                .iter()
                .filter(|i| i.key.starts_with("::{"))
                .map(|i| i.key.clone())
                .collect();
        }
        items
    }

    pub fn snapshot_positions(&mut self) -> HashMap<String, (f64, f64)> {
        self.enumerate()
            .into_iter()
            .filter_map(|i| i.pos.map(|p| (i.key, p)))
            .collect()
    }

    /// Cheap check of both Desktop folders; only ask Explorer (cross-process
    /// COM) when something actually changed.
    pub fn scan(&mut self) -> Option<Vec<Item>> {
        let mut listing: Vec<String> = desktop_dirs()
            .iter()
            .filter_map(|d| std::fs::read_dir(d).ok())
            .flat_map(|rd| {
                rd.flatten()
                    .map(|e| e.file_name().to_string_lossy().to_string())
            })
            .collect();
        listing.sort();
        let verify_hidden_view = std::env::args().any(|a| a == "--desktop-test");
        if listing == self.last_listing && !listing.is_empty() && !verify_hidden_view {
            return None;
        }
        let items = self.enumerate();
        // Never cache an empty/transient Explorer response while real files
        // still exist, or every subsequent scan would suppress the retry.
        if items.is_empty() && !listing.is_empty() {
            return None;
        }
        self.last_listing = listing;
        Some(items)
    }

    pub fn open(&self, key: &str) {
        let Some(pidl) = parse_pidl(key) else { return };
        unsafe {
            let mut sei = SHELLEXECUTEINFOW {
                cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_INVOKEIDLIST,
                lpIDList: pidl as *mut _,
                nShow: SW_SHOWNORMAL.0,
                ..Default::default()
            };
            let _ = ShellExecuteExW(&mut sei);
            CoTaskMemFree(Some(pidl as *const _));
        }
    }

    fn item_menu(&self, key: &str) -> Option<IContextMenu> {
        let pidl = parse_pidl(key)?;
        unsafe {
            let mut child: *mut ITEMIDLIST = std::ptr::null_mut();
            let parent: Option<IShellFolder> = SHBindToParent(pidl, Some(&mut child)).ok();
            let cm = parent.and_then(|f| {
                f.GetUIObjectOf::<IContextMenu>(self.helper, &[child as *const _], None)
                    .ok()
            });
            CoTaskMemFree(Some(pidl as *const _));
            cm
        }
    }

    fn invoke_verb(&self, key: &str, verb: &'static [u8]) {
        if let Some(cm) = self.item_menu(key) {
            unsafe {
                let info = CMINVOKECOMMANDINFO {
                    cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
                    hwnd: self.helper,
                    lpVerb: PCSTR(verb.as_ptr()),
                    nShow: SW_SHOWNORMAL.0,
                    ..Default::default()
                };
                let _ = cm.InvokeCommand(&info);
            }
        }
    }

    pub fn quick_look(&self, key: &str) {
        self.open(key)
    }
    pub fn trash(&self, key: &str) {
        self.invoke_verb(key, b"delete\0")
    }
    pub fn poll(&mut self) -> Vec<Outcome> {
        std::mem::take(&mut *self.done.borrow_mut())
    }

    /// Rename like Explorer: the whole name shows with only the part before
    /// the extension selected, so typing over it keeps ".mov".
    pub fn rename(&self, key: &str) {
        let path = PathBuf::from(key);
        if !path.exists() {
            // virtual items (This PC, Recycle Bin): the Properties sheet
            return self.invoke_verb(key, b"properties\0");
        }
        let Some(old) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
            return;
        };
        let is_dir = path.is_dir();
        let sel = edit_part(&old, is_dir).encode_utf16().count();
        let Some(typed) = rename_box(self.helper, &old, sel) else {
            return;
        };
        let new = match check_new_name(&old, &typed, is_dir) {
            NameCheck::Ok(n) => n,
            NameCheck::Bad(m) => return warn(self.helper, &m),
            NameCheck::AskExt { keep, change, .. } => {
                let r = message(
                    self.helper,
                    "If you change a file name extension, the file might become unusable.\n\nAre you sure you want to change it?",
                    MB_ICONWARNING | MB_YESNO,
                );
                if r == IDYES { change } else { keep }
            }
        };
        if new == old {
            return;
        }
        let dst = path.with_file_name(&new);
        if dst.exists() && new.to_lowercase() != old.to_lowercase() {
            return warn(
                self.helper,
                &format!("There is already a file with the name “{new}” in this location."),
            );
        }
        if let Err(e) = std::fs::rename(&path, &dst) {
            return warn(self.helper, &format!("Couldn't rename “{old}”: {e}"));
        }
        self.done.borrow_mut().push(Outcome::Renamed {
            old: key.to_string(),
            new: dst.to_string_lossy().to_string(),
        });
    }

    /// Duplicate the way Explorer names copies ("name - Copy.ext"); the
    /// copy's duck lands beside the original.
    pub fn duplicate(&self, key: &str) {
        let src = PathBuf::from(key);
        let Some(name) = src.file_name().map(|n| n.to_string_lossy().to_string()) else {
            return;
        };
        if !src.exists() {
            return;
        }
        let is_dir = src.is_dir();
        let stem = edit_part(&name, is_dir).to_string();
        let ext = name[stem.len()..].to_string();
        let mut dst = src.with_file_name(format!("{stem} - Copy{ext}"));
        let mut n = 2;
        while dst.exists() {
            dst = src.with_file_name(format!("{stem} - Copy ({n}){ext}"));
            n += 1;
        }
        let ok = if is_dir { copy_dir(&src, &dst).is_ok() } else { std::fs::copy(&src, &dst).is_ok() };
        if ok {
            self.done.borrow_mut().push(Outcome::Duplicated {
                src: key.to_string(),
                new: dst.to_string_lossy().to_string(),
            });
        } else {
            warn(self.helper, &format!("Couldn't duplicate “{name}”."));
        }
    }

    pub fn import_drop(&self, src: &Path) -> Option<String> {
        let desk = dirs::desktop_dir()?;
        if src.parent() == Some(desk.as_path()) {
            return Some(src.to_string_lossy().to_string());
        }
        let dst = desk.join(src.file_name()?);
        if dst.exists() {
            return None;
        }
        if std::fs::rename(src, &dst).is_err() {
            std::fs::copy(src, &dst).ok()?;
        }
        Some(dst.to_string_lossy().to_string())
    }

    pub fn play(&self, wav: &'static [u8], _volume: f32) {
        unsafe {
            let _ = PlaySoundW(
                PCWSTR(wav.as_ptr() as *const u16),
                None,
                SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
            );
        }
    }

    /// Show a shell menu at the cursor; returns the chosen command id.
    fn track(&self, cm: &IContextMenu, extra: &[(u32, &str)]) -> u32 {
        unsafe {
            let Ok(menu) = CreatePopupMenu() else {
                return 0;
            };
            let _ = cm.QueryContextMenu(menu, 0, 1, OUR_FIRST - 1, CMF_NORMAL | CMF_CANRENAME);
            if !extra.is_empty() {
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
                for (id, label) in extra {
                    let _ = AppendMenuW(menu, MF_STRING, *id as usize, &HSTRING::from(*label));
                }
            }
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            ACTIVE_MENU.with(|c| *c.borrow_mut() = Some(cm.clone()));
            let _ = SetForegroundWindow(self.helper);
            let cmd = TrackPopupMenuEx(
                menu,
                (TPM_RETURNCMD | TPM_RIGHTBUTTON).0,
                pt.x,
                pt.y,
                self.helper,
                None,
            )
            .0 as u32;
            ACTIVE_MENU.with(|c| *c.borrow_mut() = None);
            if cmd >= 1 && cmd < OUR_FIRST && verb_of(cm, cmd - 1).as_deref() == Some("rename") {
                let _ = DestroyMenu(menu);
                return ID_RENAME;
            }
            if cmd >= 1 && cmd < OUR_FIRST {
                let verb = (cmd - 1) as usize;
                let info = CMINVOKECOMMANDINFOEX {
                    cbSize: std::mem::size_of::<CMINVOKECOMMANDINFOEX>() as u32,
                    fMask: CMIC_MASK_UNICODE | CMIC_MASK_PTINVOKE_RAW,
                    hwnd: self.helper,
                    lpVerb: PCSTR(verb as *const u8),
                    lpVerbW: PCWSTR(verb as *const u16),
                    nShow: SW_SHOWNORMAL.0,
                    ptInvoke: pt,
                    ..Default::default()
                };
                let _ = cm.InvokeCommand(&info as *const _ as *const CMINVOKECOMMANDINFO);
            }
            let _ = DestroyMenu(menu);
            cmd
        }
    }

    pub fn show_item_menu(&mut self, _window: &Window, key: &str) -> Outcome {
        match self.item_menu(key) {
            Some(cm) => match self.track(
                &cm,
                &[(ID_DUP, "Duplicate"), (ID_CUSTOMISE, "Customise this duck…")],
            ) {
                0 => Outcome::None,
                ID_CUSTOMISE => Outcome::Customise(key.to_string()),
                ID_DUP => {
                    self.duplicate(key);
                    Outcome::None
                }
                ID_RENAME => {
                    self.rename(key);
                    Outcome::None
                }
                _ => Outcome::Rescan,
            },
            None => Outcome::None,
        }
    }

    pub fn show_desktop_menu(&mut self, _window: &Window, labels_on: bool) -> Outcome {
        let cm = self
            .desk()
            .and_then(|d| unsafe { d.view.GetItemObject::<IContextMenu>(SVGIO_BACKGROUND).ok() });
        let Some(cm) = cm else { return Outcome::None };
        let extra = [
            (ID_HOME, "Send ducks home"),
            (ID_FEED, "Bread for everyone"),
            (
                ID_LABELS,
                if labels_on {
                    "Hide name tags"
                } else {
                    "Show name tags"
                },
            ),
            (ID_PREFS, "Duck Pond settings…"),
        ];
        match self.track(&cm, &extra) {
            0 => Outcome::None,
            ID_HOME => Outcome::SendHome,
            ID_FEED => Outcome::FeedAll,
            ID_LABELS => Outcome::ToggleLabels,
            ID_PREFS => Outcome::OpenPrefs,
            _ => Outcome::Rescan,
        }
    }
}


/// The language-independent verb of a shell menu command ("rename", "copy"…).
fn verb_of(cm: &IContextMenu, id: u32) -> Option<String> {
    let mut buf = [0u16; 64];
    unsafe {
        cm.GetCommandString(
            id as usize,
            GCS_VERBW,
            None,
            PSTR(buf.as_mut_ptr() as *mut u8),
            buf.len() as u32,
        )
        .ok()?;
    }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..n]).to_ascii_lowercase())
}

fn message(owner: HWND, text: &str, style: MESSAGEBOX_STYLE) -> MESSAGEBOX_RESULT {
    unsafe {
        MessageBoxW(
            Some(owner),
            &HSTRING::from(text),
            w!("Duck Pond"),
            style | MB_SETFOREGROUND,
        )
    }
}

fn warn(owner: HWND, text: &str) {
    message(owner, text, MB_ICONWARNING | MB_OK);
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let to = dst.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &to)?;
        } else {
            std::fs::copy(e.path(), to)?;
        }
    }
    Ok(())
}

// ------------------------------------------------------------- rename box

thread_local! {
    static RENAME_DONE: RefCell<Option<Option<String>>> = const { RefCell::new(None) };
    static RENAME_EDIT: RefCell<HWND> = RefCell::new(HWND::default());
}

const RB_EDIT: i32 = 100;
/// edit-control select range (Win32_UI_Controls isn't otherwise needed)
const EM_SETSEL: u32 = 0x00B1;

unsafe extern "system" fn rename_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wp.0 & 0xffff) as i32;
            if id == IDOK.0 || id == IDCANCEL.0 {
                let text = (id == IDOK.0).then(|| {
                    let edit = RENAME_EDIT.with(|e| *e.borrow());
                    let n = GetWindowTextLengthW(edit) as usize;
                    let mut buf = vec![0u16; n + 1];
                    let got = GetWindowTextW(edit, &mut buf) as usize;
                    String::from_utf16_lossy(&buf[..got])
                });
                RENAME_DONE.with(|d| *d.borrow_mut() = Some(text));
                let _ = DestroyWindow(h);
                return LRESULT(0);
            }
            DefWindowProcW(h, msg, wp, lp)
        }
        WM_CLOSE => {
            RENAME_DONE.with(|d| *d.borrow_mut() = Some(None));
            let _ = DestroyWindow(h);
            LRESULT(0)
        }
        _ => DefWindowProcW(h, msg, wp, lp),
    }
}

/// A small modal "Rename" box: the full name, with the first `sel` UTF-16
/// units (the name part) selected. None if cancelled.
fn rename_box(owner: HWND, name: &str, sel: usize) -> Option<String> {
    unsafe {
        let inst = GetModuleHandleW(None).ok()?;
        let class = w!("DuckPondRename");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(rename_proc),
            hInstance: inst.into(),
            lpszClassName: class,
            hbrBackground: HBRUSH((COLOR_BTNFACE.0 + 1) as isize as *mut _),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc); // fails harmlessly if already registered
        let screen = GetDC(None);
        let k = GetDeviceCaps(Some(screen), LOGPIXELSX) as f32 / 96.0;
        ReleaseDC(None, screen);
        let px = |v: i32| (v as f32 * k).round() as i32;
        let (w, h) = (px(380), px(150));
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let dlg = CreateWindowExW(
            WS_EX_DLGMODALFRAME | WS_EX_TOPMOST,
            class,
            w!("Rename"),
            WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            pt.x - w / 2,
            pt.y - h / 2,
            w,
            h,
            Some(owner),
            None,
            Some(inst.into()),
            None,
        )
        .ok()?;
        let font = GetStockObject(DEFAULT_GUI_FONT);
        let child = |cls: PCWSTR, text: &str, style: WINDOW_STYLE, x, y, cw, ch, id: i32| {
            let c = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                cls,
                &HSTRING::from(text),
                WS_CHILD | WS_VISIBLE | style,
                px(x),
                px(y),
                px(cw),
                px(ch),
                Some(dlg),
                Some(HMENU(id as isize as *mut _)),
                Some(inst.into()),
                None,
            )
            .unwrap_or_default();
            SendMessageW(c, WM_SETFONT, Some(WPARAM(font.0 as usize)), Some(LPARAM(1)));
            c
        };
        child(w!("STATIC"), &format!("Rename “{name}” to:"), WINDOW_STYLE(0), 12, 10, 350, 18, -1);
        let edit = child(
            w!("EDIT"),
            name,
            WS_BORDER | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            12,
            32,
            350,
            24,
            RB_EDIT,
        );
        child(
            w!("BUTTON"),
            "Rename",
            WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
            196,
            72,
            80,
            26,
            IDOK.0,
        );
        child(w!("BUTTON"), "Cancel", WS_TABSTOP, 282, 72, 80, 26, IDCANCEL.0);
        RENAME_EDIT.with(|e| *e.borrow_mut() = edit);
        RENAME_DONE.with(|d| *d.borrow_mut() = None);
        let _ = SetForegroundWindow(dlg);
        let _ = SetFocus(Some(edit));
        SendMessageW(edit, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(sel as isize)));
        let mut msg = MSG::default();
        while RENAME_DONE.with(|d| d.borrow().is_none()) {
            if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
                break;
            }
            if !IsDialogMessageW(dlg, &msg).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        RENAME_DONE.with(|d| d.borrow_mut().take()).flatten()
    }
}
