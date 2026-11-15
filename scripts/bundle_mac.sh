#!/bin/bash
# Build "Duck Pond.app" (the pond + its Settings window) into dist/.
#   ./scripts/bundle_mac.sh
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"

cargo build --release
./scripts/build_prefs.sh release

app="dist/Duck Pond.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/duckpond "$app/Contents/MacOS/duckpond"
cp target/release/DuckPondPreferences "$app/Contents/MacOS/DuckPondPreferences"
cp assets/AppIcon.icns "$app/Contents/Resources/AppIcon.icns"
version="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Duck Pond</string>
    <key>CFBundleDisplayName</key><string>Duck Pond</string>
    <key>CFBundleIdentifier</key><string>app.duckpond.pond</string>
    <key>CFBundleExecutable</key><string>duckpond</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>${version}</string>
    <key>CFBundleVersion</key><string>${version}</string>
    <key>LSMinimumSystemVersion</key><string>13.0</string>
    <key>LSUIElement</key><true/>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSAppleEventsUsageDescription</key>
    <string>Duck Pond asks Finder where your desktop icons sit, and uses Finder for actions like Move to Trash, Get Info and Rename.</string>
</dict>
</plist>
PLIST

# ad-hoc signature so macOS will launch a locally built app
codesign --force --deep --sign - "$app" >/dev/null
echo "built $app"
