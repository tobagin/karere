## 1. Verbatim mobile-responsive script

- [ ] 1.1 Copy `/home/tobagin/Projects/karere/src/mobile_responsive.js` to `data/js/mobile_responsive.js` byte-for-byte; do not edit, reformat, or annotate.
- [ ] 1.2 Confirm the file is picked up by M13's `build.rs` enumeration (lexical order places it after `00-bootstrap.js`).
- [ ] 1.3 Verify with `cargo build` that `$OUT_DIR/injected_bundle.js` now contains the script contents.

## 2. Viewport IPC plumbing

- [ ] 2.1 Confirm `BrowserMessage::SetViewportSize { w: i32, h: i32 }` already exists in `src/ipc.rs` from M13; add it if missing.
- [ ] 2.2 In `src/cef_gtk_area.rs::size_allocate`, after the existing `host.was_resized()` call, send `BrowserMessage::SetViewportSize { w, h }` via the M13 process-message helper.
- [ ] 2.3 Ensure the IPC send is fire-and-forget and does not block the GTK main thread on a failed send (log and drop).

## 3. Bootstrap event bridge

- [ ] 3.1 Extend `data/js/00-bootstrap.js` so the inbound `SetViewportSize` dispatcher calls `document.dispatchEvent(new CustomEvent('karere:viewport-resize', { detail: { w, h } }))`.
- [ ] 3.2 Register the dispatcher synchronously at the top of `00-bootstrap.js` so it is live before `mobile_responsive.js` runs later in the bundle.
- [ ] 3.3 Cache the most recent viewport `{ w, h }` on a renderer-side global so a late-binding listener can replay the last value on demand (defensive against any race during context creation).

## 4. Display handler — JS-initiated fullscreen

- [ ] 4.1 Verify `DisplayHandler::on_fullscreen_mode_change` is exposed by cef-rs; if absent, file an upstream issue and stop at the responsive half of this milestone.
- [ ] 4.2 Add a `FullscreenRequest { on: bool }` variant to the SharedState event queue used by the M08 window polling loop.
- [ ] 4.3 Implement `on_fullscreen_mode_change(&self, _browser, fullscreen: i32)` in `src/handlers/display.rs`: push `FullscreenRequest { on: fullscreen != 0 }` into SharedState; do not touch GTK widgets.

## 5. Window fullscreen + headerbar wiring

- [ ] 5.1 In the M08 polling loop, drain `FullscreenRequest`s: call `window.fullscreen()` when `on == true`, `window.unfullscreen()` when `on == false`.
- [ ] 5.2 Connect a `notify::fullscreened` signal on the `ApplicationWindow`; in the handler, call `header_bar.set_visible(!window.is_fullscreen())`.
- [ ] 5.3 Ensure the signal handler is the authoritative path for headerbar visibility; remove any duplicate `set_visible` calls from the polling-loop drain (or make them strictly idempotent).
- [ ] 5.4 Confirm the headerbar restoration works on all exit paths: JS `document.exitFullscreen()`, Esc, F11, and WM-driven exit.

## 6. Verification

- [ ] 6.1 Launch the app at karere's WhatsApp Web target; resize the window to a narrow width and confirm the WhatsApp sidebar collapses to the single-pane mobile layout, matching karere v3 at the same dimensions.
- [ ] 6.2 Start a WhatsApp video call; trigger fullscreen on the call surface; confirm the GTK window enters fullscreen and the Adwaita headerbar disappears.
- [ ] 6.3 Press Esc while fullscreen; confirm the window leaves fullscreen and the headerbar reappears.
- [ ] 6.4 Press F11 to toggle fullscreen manually; confirm headerbar visibility tracks the resulting window state in both directions.
- [ ] 6.5 Rapid-drag the window edge; confirm the responsive layout updates smoothly with no console errors from `mobile_responsive.js`.

## 7. Documentation

- [ ] 7.1 Add a top-of-file comment to `data/js/mobile_responsive.js` declaring it a verbatim copy and naming the karere v3 source path; this is the ONE permitted edit and only if a comment can be added without altering script behavior — otherwise leave the file fully verbatim and document upstream in the milestone notes only.
- [ ] 7.2 Add a short note to `src/handlers/display.rs` explaining that fullscreen mutations are deferred to the GTK main thread via SharedState.
