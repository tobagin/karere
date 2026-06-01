## Why

CEF requires a deterministic close sequence — the embedder must call `BrowserHost::close_browser`, wait for the renderer subprocess to confirm via `OnBeforeClose`, and only then dismiss the GTK window. The M1 implementation returned `Propagation::Proceed` immediately on `close_request`, tearing down the widget while the renderer subprocess was still alive. This leaked subprocesses, produced `Couldn't release X11 display` warnings, and could crash the next launch.

## What Changes

- `KarereWindow` (named `ShellWindow` in M1–M6, renamed in M7) gets a two-phase `connect_close_request` handler gated on a `closing` cell and `CefGtkArea::is_browser_closed()`.
- `CefGtkArea` gains `close_browser()` (calls `host.close_browser(0)` with `force_close = 0`) and `is_browser_closed() -> bool` (reads `life_span.lock().as_ref().map(|life| life.state.lock().closed).unwrap_or(true)`).
- `CefGtkArea::unrealize` also calls `close_browser()` to cover widget-removed-before-window-closed.
- `ShellLifeSpanHandler` adds `state.closed: bool`, toggled to `true` inside `on_before_close`. `do_close` returns 0 (allow).
- First close invocation spawns a 50 ms `glib::timeout_add_local` poll that re-fires `win.close()` once `is_browser_closed` is true; the source is dropped when the second close pass proceeds.

## Capabilities

### New Capabilities
- `cef-clean-shutdown`: Two-phase, gated window-close handshake coordinated with the CEF browser lifecycle so the renderer subprocess exits before GTK tears down.

### Modified Capabilities
<!-- none -->

## Impact

- Code: `src/window.rs` (`connect_close_request`, `closing` cell), `src/cef_gtk_area.rs` (`close_browser`, `is_browser_closed`, `unrealize`), `src/handlers/life_span.rs` (`state.closed`, `on_before_close`, `do_close`).
- Runtime: process exits with code 0 on manual close and `app.quit`; no `Couldn't release X11 display` warnings; no orphan zygote subprocesses under `strace -f`.
- No external APIs, no new dependencies.
- Non-goal: surfacing page-side `beforeunload` returns to the user — we close anyway.
