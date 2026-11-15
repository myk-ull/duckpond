# Duck Pond 🦆

Your desktop icons, as little pixel-art ducks swimming on Monet's
*Water Lilies* (1906).

Every file and folder on your desktop becomes a duck. They paddle, waddle onto
lily pads, nap, daydream in thought bubbles, and trail their ducklings around
the pond. Your files still work exactly as before:

| Do this | And |
|---|---|
| Double-click a duck | opens its file |
| Right-click a duck | the usual Finder / Explorer menu, plus **Customise This Duck…** |
| Drag a duck | moves it (its new spot is remembered) |
| Click the water | drops bread; nearby ducks queue up politely and quack |
| Double-click the water | everyone scurries back to their spot |
| Hold ⌥ / Alt | shows every name tag |
| Drop a file on the pond | moves it onto the Desktop and a duck hatches |

The pond is a small native Rust app: one tiny pixel framebuffer, scaled up on
the GPU (macOS) or with GDI (Windows). It idles at a low frame rate and drops
to almost nothing when a full-screen app covers the desktop.

## Settings

Open **Settings…** from the duck in the menu bar (or right-click the water).
On macOS it's a native window in the style of System Settings:

- **General**: open at login, quack volume, name tags, day & night (golden
  hour in the evening, fireflies after dark).
- **Ducks & Ducklings**: which breeds are in the mix, and how much of the
  flock are ducklings (slider from none to almost all).
- **File Types**: which breed each kind of file hatches as (folders, images,
  PDFs, code…), or “Mix”.
- **Hats**: turn hats on and pick one per kind of file.
- **Your Files**: search any file on your desktop and set its exact breed,
  age (duckling / grown-up) and hat.

With several displays, every display gets the pond. Your file ducks stay on
the main display, and each other display gets its own visiting family that
stays on that screen. Double-click a visitor and it tells you it's just
visiting. Double-clicking the water sends everyone home, visitors included.

Grown-ups are drawn at 30×27 pixels and ducklings at 20×18, so parents stand
half again as tall as their babies.

Every change applies to the pond immediately. Nine breeds are included. The
default is white Pekins with their bright yellow ducklings, as in real life.

## Install / build

### macOS 13+

Needs [Rust](https://rustup.rs) and the Xcode command-line tools.

```bash
./scripts/bundle_mac.sh        # builds dist/Duck Pond.app
open "dist/Duck Pond.app"
```

The first launch asks Finder where your icons are (macOS will ask you to allow
that), then hides Finder's own desktop icons. **Quit Duck Pond** from the menu
bar duck brings them back.

For development, `./scripts/dev.sh` runs the pond and rebuilds on every save;
`./scripts/build_prefs.sh` builds the Settings window next to it.

### Windows 10 / 11

```bash
cargo build --release --target x86_64-pc-windows-gnu    # from macOS/Linux (needs mingw-w64)
makensis installer/windows/duckpond.nsi                 # → dist/DuckPondSetup.exe
```

The installer needs no admin rights. The app is unsigned, so SmartScreen may
warn: **More info → Run anyway**. `DuckPond.exe --selftest` writes a diagnostic
report; `--quit` and `--restore` bring the desktop icons back. See
[docs/WINDOWS.md](docs/WINDOWS.md) for how the desktop integration works.
On Windows, Settings currently opens `preferences.json`; a native window is
still to come.

## How it's made

- `src/scene.rs`: the pond simulation and software renderer (ducks, bread,
  ripples, particles, day & night).
- `src/platform/`: desktop integration for macOS (`mac.rs`) and Windows
  (`win.rs`): hiding the real icons, menus, opening files.
- `src/items.rs`: preferences and the rules for which file becomes which duck.
- `prefs-mac/`: the SwiftUI Settings window.
- `tools/`: the art pipeline. `duck_art.py` holds the hand-drawn sprite
  templates, `make_ducks.py` animates and colours them into `assets/ducks.png`,
  `make_mask.py` finds the lily pads ducks can walk on.

Preferences live in `preferences.json` in the app's config folder
(`~/Library/Application Support/DuckPond` or `%APPDATA%\DuckPond`); duck
positions live separately in `state.json`.

## Credits

- Painting: Claude Monet, *Water Lilies* (1906), Art Institute of Chicago,
  public domain (CC0). Pixelated by `tools/`.
- Quacks: excerpts from Wikimedia Commons recordings, CC BY-SA. See
  [assets/quacks/CREDITS.md](assets/quacks/CREDITS.md). Duckling peeps are
  synthesised.
- Duck sprites: drawn for this project.

## Licence

Code and duck art: [MIT](LICENSE). The quack recordings in `assets/quacks/`
remain under their CC BY-SA licences.
