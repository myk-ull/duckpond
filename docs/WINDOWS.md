# Windows notes

## Outcome and constraint

The user confirmed that the pond and ducks work on their Windows 11 desktop. The repaired executable was installed and relaunched. Keep the pond inside Explorer's desktop: there is no standalone borderless or bottom-most window fallback.

## Two separate bugs

1. **Invisible pond:** the original code used a wgpu/DXGI swapchain in a cross-process Explorer child. The tray and sounds worked, but the rendered scene did not appear. Windows now uses the existing CPU scene framebuffer and GDI StretchDIBits in a WS_EX_LAYERED desktop child. macOS keeps its existing GPU renderer. The exact DXGI compositor failure was not independently isolated; replacing that path resolved the observed rendering failure.
2. **Background without ducks:** after hiding Explorer icons, the view returned an empty displayed-item list. Caching that result prevented later scans from recovering. The diagnostic confirmed zero scene ducks. Initial scanning now happens before hiding icons. Hidden-view scans fall back to the underlying shell folder, filter out unrelated namespace folders and virtual icons that were not visible, and retry empty responses rather than caching them. The final test retained 50 ducks through repeated hidden-view enumeration.

## Implementation map

- `src/platform/win.rs`: `parent_handle` supplies the Explorer parent before winit creates the child. `prepare_window` checks attachment, sets WS_EX_LAYERED, removes incompatible class styles, enables opaque layered composition with SetLayeredWindowAttributes, and positions the child. Attachment/composition failures stop startup instead of falling back to a top-level window.
- `src/platform/win.rs`: `set_pond_visible` uses native ShowWindow. winit's visibility setter rewrites native styles and removed the custom child/layered state during the first attempted fix; do not restore that setter on the Windows pond path.
- `src/platform/win.rs`: `paint_pond` converts the freshly rendered RGBA framebuffer to top-down BGRA and scales it with StretchDIBits. Only scaling margins are cleared, avoiding a full black clear between frames. Hover/selection labels use GDI Segoe UI. DCs and per-frame font objects are released.
- `src/platform/win.rs`: `enumerate` falls back to IShellFolder.EnumObjects when the native view is empty. Physical desktop roots and remembered visible virtual icons constrain that fallback. `scan` does not cache an empty response while physical desktop entries exist.
- `src/main.rs`: scan before hiding icons; render through GDI on Windows; keep integer scaling and mouse-coordinate mapping aligned; ignore zero-sized resize events. Recreating a destroyed desktop window refreshes scanning, visibility and occlusion state.
- `build.rs`: embed a Windows 8/10 compatibility manifest and asInvoker execution level. Layered child support requires the compatibility declaration; resource embedding errors fail the build.
- `Cargo.toml`: Windows no longer compiles pixels/wgpu or fontdue. Those dependencies remain for the existing non-Windows renderer.
- `src/items.rs`: optional DUCKPOND_CONFIG_DIR isolates test settings.

## Preferences on Windows

Preferences live in `%APPDATA%\DuckPond\preferences.json` (runtime state such
as duck homes is kept separately in `state.json`). The pond watches the
preferences file and applies edits live. The native Settings window is
macOS-only for now; on Windows, **Settings…** in the tray (or on the desktop
right-click menu) opens `preferences.json` in Notepad. To use every breed,
list them all in `"mix"` and set each entry of `"kind_breeds"` to `"mix"`.
