#!/bin/bash
# Live-ish reload: rebuild on every save and swap the running pond.
# The old pond keeps swimming until the new build succeeds.
# Ctrl-C stops the pond and brings Finder's desktop icons back.
cd "$(dirname "$0")/.."
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
export DUCKPOND_KEEP_HIDDEN=1
stamp=$(mktemp)
pid=""
start() { ./target/debug/duckpond & pid=$!; echo "🦆 pond running (pid $pid)"; }
stop() { if [ -n "$pid" ]; then kill -TERM "$pid" 2>/dev/null; wait "$pid" 2>/dev/null; fi; pid=""; }
finish() {
  stop
  echo "restoring Finder desktop icons…"
  defaults delete com.apple.finder CreateDesktop 2>/dev/null
  killall Finder
  exit 0
}
trap finish INT TERM
cargo build && start
touch "$stamp"
while true; do
  sleep 1
  if [ -n "$(find src assets Cargo.toml -newer "$stamp" -type f 2>/dev/null | head -1)" ]; then
    touch "$stamp"
    echo "🔨 change detected, rebuilding…"
    if cargo build 2>&1 | grep -E "^(error|warning: unused)" -A5; [ "${PIPESTATUS[0]}" -eq 0 ]; then
      stop; start
    else
      echo "❌ build failed: the old pond keeps swimming"
    fi
  fi
done
