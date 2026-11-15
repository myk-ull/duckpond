#!/bin/bash
# Build the native macOS Settings window next to the pond binary.
#   scripts/build_prefs.sh [debug|release]   (default: debug)
set -euo pipefail
cd "$(dirname "$0")/.."
profile="${1:-debug}"
out="target/$profile"
mkdir -p "$out"
arch="$(uname -m)"
swiftc -O -parse-as-library \
  -target "${arch}-apple-macos13.0" \
  -o "$out/DuckPondPreferences" \
  prefs-mac/DuckPondPreferences.swift
echo "built $out/DuckPondPreferences"
