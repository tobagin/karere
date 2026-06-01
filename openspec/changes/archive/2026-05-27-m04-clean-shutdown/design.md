## Context

The M1 shell wired a minimal `connect_close_request` that returned `Propagation::Proceed` immediately, letting GTK destroy the window before CEF had a chance to shut down its renderer subprocess. CEF's contract requires the embedder to call `BrowserHost::close_browser` and wait for the asynchronous `OnBeforeClose` callback before destroying the host widget. Violating this leaves zombie zygote subprocesses and surfaces `Couldn't release X11 display` warnings on shutdown.

`ShellLifeSpanHandler` already held a `state.browser` handle (set in `on_after_created`) but did not record a closed flag. The window and the CEF area communicate through the `CefGtkArea` widget, whose `life_span` field carries the `LifeSpanState`.

## Goals / Non-Goals

**Goals:**
- Deterministic teardown: `close_browser` → CEF runs async shutdown → `OnBeforeClose` → window destroyed.
- Clean exit code 0 on both manual close and `app.quit`.
- Cover the case where the widget is removed from its parent before the window closes.
- Reuse the existing `LifeSpanState` plumbing — no new channels or signals.

**Non-Goals:**
- Surfacing JavaScript `beforeunload` prompts to the user (no modal confirmation UX).
- Timeout/escape hatch if `OnBeforeClose` never fires (deferred to a later milestone).
- Coordinated multi-window shutdown ordering — each window handles its own browser.

## Decisions

- **Two-phase close gated by a `Cell<bool>` (`closing`) plus `is_browser_closed()`** instead of a channel/future. CEF callbacks do not drive the GLib mainloop directly, so a small `glib::timeout_add_local` poll (50 ms) is the simplest way to re-enter the close path once the browser confirms. Alternative considered: posting a custom signal from `on_before_close` into the main thread — rejected as more wiring for the same observable behavior.
- **`is_browser_closed()` defaults to `true` when no `LifeSpanState` exists.** This lets windows that never created a browser (e.g., closed during startup) proceed without hanging. Alternative: panic — rejected as needlessly fragile.
- **`do_close` returns 0 (allow close).** We do not implement the CEF pattern of returning 1 to defer; the M4 scope explicitly omits before-unload UX.
- **`force_close = 0` on `host.close_browser`.** Lets page-side `beforeunload` handlers run server-side even though we do not surface their result. Switching to `force_close = 1` would skip that and is reserved for forced shutdown paths.
- **`unrealize` also calls `close_browser()`** so the handshake runs even if the widget is detached before its containing window emits `close_request`.

## Risks / Trade-offs

- **Risk:** 50 ms poll adds shutdown latency. **Mitigation:** Imperceptible to users; can be tightened later if profiling demands it.
- **Risk:** If `OnBeforeClose` never fires (CEF bug or stuck renderer), the window hangs visible forever. **Mitigation:** Out of scope for M4; later milestone can add a hard timeout that flips `force_close = 1` and forces destroy.
- **Trade-off:** The `Cell<bool>` `closing` flag is per-window and is not reset — once a window enters closing it cannot be cancelled. Acceptable because there is no cancel UX yet.

## Migration Plan

- Replace the M1 `connect_close_request` returning `Propagation::Proceed` with the two-phase handler.
- Extend `LifeSpanState` with `closed: bool` (default `false`).
- No data migration, no config changes. Rollback: revert the three touched files.
