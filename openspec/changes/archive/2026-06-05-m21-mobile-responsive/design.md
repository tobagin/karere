## Context

Karere v3 (the WebKit-based predecessor) ships a single ~35 KB JS file, `mobile_responsive.js`, that hooks into WhatsApp Web's React tree and rewrites layout for narrow widths (sidebar collapse, single-pane mode, font scaling). Under WebKit it was injected via `UserContentManager::add_script`; the script listens on `document` for a `karere:viewport-resize` `CustomEvent` whose `detail` carries `{ w, h }` and re-applies layout each time. The script is otherwise self-contained — it neither imports nor depends on host-bridge globals.

CEF replaces injection via the M13 render-process bundle. Concatenating `mobile_responsive.js` into the M13 bundle gives identical injection semantics at `on_context_created`. The remaining wiring is mechanical: forward GTK `size_allocate` events as `SetViewportSize` IPCs, and translate them to the CustomEvent in the bootstrap.

JS-initiated fullscreen is a separate concern. WhatsApp video-call UI calls `element.requestFullscreen()`; CEF surfaces this as `DisplayHandler::on_fullscreen_mode_change(browser, fullscreen)` in the browser process. The shell must mirror the state into GTK (`Window::fullscreen()` / `unfullscreen()`) and adjust headerbar visibility. CEF runs callbacks on the CEF UI thread, not the GTK main thread, so the change must be marshaled through the existing SharedState queue drained by the M08 polling loop.

## Goals / Non-Goals

**Goals:**
- Drop `mobile_responsive.js` in verbatim — zero edits relative to karere v3's copy.
- Feed it viewport changes on every GTK resize so layout stays in sync without polling.
- Handle JS-initiated fullscreen end-to-end: window state + headerbar visibility, both on entry and exit.
- Preserve user-initiated exit paths (Esc, F11, window manager) — exiting fullscreen by any route restores the headerbar.

**Non-Goals:**
- WhatsApp-specific responsive tweaks beyond what `mobile_responsive.js` already does. Future enhancements belong in a follow-on milestone.
- Per-account responsive overrides (zoom is M18; viewport is global per window).
- Subframe injection. Only the main frame receives the bundle (M13 invariant).
- Coalescing rapid resize events — the script is cheap; pushing every `size_allocate` is acceptable.

## Decisions

### Decision: Verbatim copy of `mobile_responsive.js`
- **Choice**: Copy `/home/tobagin/Projects/karere/src/mobile_responsive.js` into `data/js/mobile_responsive.js` with no edits. Lexical ordering under M13's bundler places it after `00-bootstrap.js`.
- **Why**: The script's contract (`karere:viewport-resize` CustomEvent) is already host-agnostic. Editing it would create a maintenance fork against karere v3.
- **Trade-off**: Any WhatsApp-driven update must be pulled from karere v3 wholesale; do not patch locally.

### Decision: Bootstrap translates `SetViewportSize` → `karere:viewport-resize` CustomEvent
- **Choice**: M13's renderer dispatcher receives `BrowserMessage::SetViewportSize { w, h }` and `document.dispatchEvent(new CustomEvent('karere:viewport-resize', { detail: { w, h } }))`. `mobile_responsive.js` is a passive listener.
- **Why**: Keeps the script identical to karere v3. The bootstrap is the only place that knows about IPC; `mobile_responsive.js` stays page-only code.
- **Alternative**: Calling a global function on `mobile_responsive.js` directly — rejected because it would require editing the script.

### Decision: Send viewport from `size_allocate`, not from a debounce/throttle layer
- **Choice**: In `src/cef_gtk_area.rs::size_allocate`, immediately after `host.was_resized()`, build and send the `SetViewportSize` IPC. No coalescing.
- **Why**: GTK already coalesces allocations during drag-resize, and the script's listener is idempotent. Adding our own debouncer is premature optimization.
- **Risk**: If profiling shows render-process saturation during resize, add a 16ms debounce in the bootstrap (single timer, latest-wins). Out of scope for this milestone.

### Decision: Fullscreen via `SharedState` queue, drained in window polling loop
- **Choice**: `DisplayHandler::on_fullscreen_mode_change` pushes `FullscreenRequest { on: bool }` into the existing SharedState queue. The M08 polling loop (`Continue` source on the GTK main thread) drains the queue and calls `window.fullscreen()` or `window.unfullscreen()`, then toggles `header_bar.set_visible(!on)`.
- **Why**: CEF callbacks run off the GTK main thread. Mutating widget state from a non-GTK thread is unsupported. The polling loop already exists for window-restore actions in M08; piggybacking is the smallest delta.
- **Alternative**: `glib::idle_add_local` from the CEF callback — works but introduces a second cross-thread mechanism alongside SharedState. Rejected for consistency.

### Decision: Headerbar visibility tied to `Window::is_fullscreen()`, not to the request
- **Choice**: After applying `fullscreen()` / `unfullscreen()`, set headerbar visibility based on the resulting window state, observed via a `notify::fullscreened` signal on the window.
- **Why**: User-initiated exits (Esc when in WM-fullscreen, F11, window-manager toggle) bypass our IPC. Hooking the GTK signal guarantees the headerbar is restored whatever exit path the user takes, including JS-initiated exits that go through `on_fullscreen_mode_change(0)`.
- **Trade-off**: Two write paths for headerbar visibility (signal + polling-loop drain). The signal is authoritative; the polling-loop call is best-effort and idempotent.

### Decision: No reverse IPC for fullscreen
- **Choice**: The browser process does not notify the renderer when the user exits fullscreen via WM (Esc / F11). CEF's `DisplayHandler::on_fullscreen_mode_change(0)` is only called when JS itself exits.
- **Why**: WhatsApp Web's video call UI tracks fullscreen via the standard `fullscreenchange` DOM event, which fires only when the document's fullscreen element actually changes. WM-driven exits leave the DOM in its "in-call" state and the user must re-click to leave the call — matching karere v3 behavior. Forcing a JS-side exit would require synthesizing `document.exitFullscreen()` from the host, which is brittle.
- **Trade-off**: Esc exits the window from fullscreen but the WhatsApp call UI may still believe it is in fullscreen. Acceptable; matches v3.

## Revision during implementation (host-side width gating)

The original design above assumed `mobile_responsive.js` is a passive listener for
a `karere:viewport-resize` CustomEvent. Implementation disproved this:

- The verbatim v3 script has **no** `karere:viewport-resize` listener and no
  width/breakpoint/matchMedia logic. It applies single-pane mobile layout
  **unconditionally** when executed (built for PinePhone/Librem5). It also reads
  page globals `window.appConfig.enableQuickCopy` and `window.__cmdParams` inside
  user-interaction handlers.
- v3 (`window.rs`) made layout responsive entirely **host-side**:
  `should_use_mobile_layout(settings, width)` (threshold 768 px, `mobile-layout`
  setting, phosh/plasma-mobile/lomiri detection), injecting the script on load via
  `evaluate_javascript` **only when mobile**, and calling `webview.reload()` on a
  width-threshold crossing so the next load re-evaluates.

Building the original always-inject + CustomEvent plan would (a) leave WhatsApp in
single-pane layout even on a wide desktop window, and (b) produce an inert
CustomEvent the script never consumes. Chosen approach (user-confirmed): **mirror
v3 host-side gating**.

- **Decision**: Place the verbatim script in `data/js-deferred/` (conditional
  injection, like `profile_dom_fallback.js`), NOT the always-run M13 bundle.
- **Decision**: Reuse the existing `on_load_end` re-apply hook (already used for
  zoom + autocorrect) to inject the script when `should_use_mobile_layout` is true,
  guarded by `window.__karereMobileApplied` for idempotency.
- **Decision**: Reload the foreground browser on a width-threshold crossing
  (tracked via `mobile_active`/`mobile_init` cells on the webview imp); the reload's
  `on_load_end` re-evaluates the gate. The verbatim script cannot un-apply its
  mutations, so reload is the only clean way back to desktop — exactly as v3 did.
- **Decision**: Drop the `SetViewportSize` → `karere:viewport-resize` CustomEvent
  path from M21 scope — the verbatim script ignores it, so it would be dead code.
  The `SetViewportSize` IPC variant + M13 dispatcher stub are left untouched.

The fullscreen half (DisplayHandler + SharedState queue + headerbar) is unaffected
and proceeds as originally designed.

## Risks / Trade-offs

- **Risk**: `mobile_responsive.js` references DOM APIs that behave differently in CEF (Chromium) vs WebKitGTK. → Mitigation: script is already vanilla DOM; smoke-test in M21 verification before declaring done.
- **Risk**: WhatsApp Web markup drifts and breaks the script. → Mitigation: pull updates from karere v3 verbatim; never patch locally.
- **Risk**: `size_allocate` fires before the renderer is ready to receive process messages. → Mitigation: bootstrap stores the last viewport and replays it inside the listener registration; the bootstrap registers the listener synchronously at top of `on_context_created` so it is live before the first main-frame load completes.
- **Risk**: `DisplayHandler::on_fullscreen_mode_change` may not be exposed by cef-rs. → Mitigation: verify at implementation; if absent, file an upstream issue and fall back to `LifeSpanHandler` callbacks where available. Bundle the JS-side responsive work even if fullscreen handling slips to a follow-on.
- **Risk**: Headerbar hide-during-fullscreen could trap the user if the WM also strips chrome. → Mitigation: standard GTK behavior unhides on `unfullscreen()`; rely on the `notify::fullscreened` signal as the single source of truth.
- **Trade-off**: Verbatim copy means we can never fix bugs in `mobile_responsive.js` locally. If a CEF-specific quirk appears, this milestone must be re-opened or the script forked explicitly with a documented divergence.
