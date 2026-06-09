#!/usr/bin/env bash
# Headless boot gate for Karere.
#
# Proves the app starts past CEF initialization without crashing — the class of
# regression that has bitten this project repeatedly and is invisible to unit
# tests (UKM null-deref at PreProfileInit, metrics .pma SIGBUS, message-pump
# spin). Runs the installed Flatpak under a virtual X server with software GL,
# then asserts on the log + process state. Network is NOT required: the gate
# hinges on "CEF initialized", which is logged the moment cef::initialize()
# returns — i.e. after the startup-crash sites, before any page load.
#
# Usage: smoke-test.sh [app-id]   (default: io.github.tobagin.karere.Devel)
set -uo pipefail

APP_ID="${1:-io.github.tobagin.karere.Devel}"
LOG="$(mktemp)"
GRACE="${SMOKE_GRACE_SECS:-35}"

cleanup() {
  flatpak kill "$APP_ID" 2>/dev/null || true
  pkill -f "karere --debug" 2>/dev/null || true
}
fail() { echo "::error::$1"; echo "----- captured log -----"; cat "$LOG"; cleanup; exit 1; }

echo "Launching $APP_ID headless for ${GRACE}s…"
# fallback-x11 path under Xvfb; force software GL so the runner needs no GPU.
LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe \
  xvfb-run -a --server-args="-screen 0 1280x1024x24" \
  flatpak run "$APP_ID" --debug >"$LOG" 2>&1 &

sleep "$GRACE"

echo "===================== app log ====================="
cat "$LOG"
echo "==================================================="

# Gate 1 — cef::initialize() returned. This line is logged AFTER PreProfileInit,
# the exact point the Ukm-disable null-deref crashed (see cef_runtime.rs). Its
# presence is direct proof the startup-crash class did not regress.
grep -q "CEF initialized" "$LOG" \
  || fail "App never reached 'CEF initialized' — startup-crash regression."

# Gate 2 — no panic or fatal signal surfaced.
if grep -qiE "panicked at|thread '.*' panicked|SIGSEGV|SIGABRT|SIGBUS|cef::initialize failed" "$LOG"; then
  fail "Crash/panic detected in startup log."
fi

# Gate 3 — a Karere process is still alive after the grace window (it didn't
# init then immediately die). Matches the real app process, not the launcher
# (whose cmdline carries the dotted app-id, e.g. '…karere.Devel --debug').
pgrep -f "karere --debug" >/dev/null \
  || fail "No live Karere process after ${GRACE}s — exited/crashed during startup."

# Informational: did the offscreen renderer paint a frame? (network-dependent,
# so not a hard gate — a value > 0 means the full render path is healthy.)
echo "on_paint frames observed: $(grep -c 'on_paint' "$LOG" || true)"

echo "✅ Smoke test passed: CEF initialized, process alive, no crash/panic."
cleanup
exit 0
