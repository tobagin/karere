## Why

Karere v3 already ships a self-contained `mobile_responsive.js` (~35 KB) that intercepts WhatsApp Web's layout and adapts it to narrow window widths. The script is page-agnostic in its interface (listens for a viewport-resize CustomEvent), so it can be reused verbatim under CEF by injecting it through the M13 render-process pipeline and feeding it a `SetViewportSize` IPC on every GTK size allocation. Independently, WhatsApp video calls trigger JS-initiated fullscreen requests which CEF surfaces via `DisplayHandler::on_fullscreen_mode_change`; the GTK shell currently ignores them, leaving call UI in a windowed state with the headerbar visible. Both pieces are required for visual parity with karere v3.

## What Changes

- Copy `mobile_responsive.js` from karere v3 verbatim into `data/js/` so M13's `build.rs` concatenates it into the embedded bundle.
- Extend the M13 bootstrap to translate inbound `BrowserMessage::SetViewportSize { w, h }` IPCs into a `karere:viewport-resize` `CustomEvent` on `document` that `mobile_responsive.js` already listens for.
- In `src/cef_gtk_area.rs::size_allocate`, after the existing `host.was_resized()` call, send `BrowserMessage::SetViewportSize { w, h }` over the M13 IPC channel.
- Implement `DisplayHandler::on_fullscreen_mode_change(&self, _browser, fullscreen: i32)` in `src/handlers/display.rs`: push a `FullscreenRequest { on: fullscreen != 0 }` event into `SharedState`.
- Extend the window polling loop (M08) to drain `FullscreenRequest`s and call `window.fullscreen()` / `window.unfullscreen()` accordingly.
- Hide `AdwHeaderBar` (`set_visible(false)`) on fullscreen entry; restore on exit, including user-initiated exits via Esc or F11.

## Capabilities

### New Capabilities
- `mobile-responsive-injection`: Verbatim port of karere v3's `mobile_responsive.js`, driven by host-pushed viewport size events through the M13 IPC bundle.
- `js-fullscreen-handler`: `DisplayHandler::on_fullscreen_mode_change` mapping JS-initiated fullscreen requests to GTK window fullscreen state plus headerbar visibility.

### Modified Capabilities
<!-- None: M21 only adds new capabilities. -->

## Impact

- New files: `data/js/mobile_responsive.js` (verbatim copy).
- Modified files: `data/js/00-bootstrap.js` (viewport-resize event dispatcher), `src/cef_gtk_area.rs` (size_allocate IPC), `src/handlers/display.rs` (fullscreen callback), `src/ipc.rs` (add `FullscreenRequest` if not already a `RendererMessage` peer — see design), `src/state.rs` or equivalent SharedState (fullscreen queue), and the main window module that owns the headerbar and polling loop.
- Build: `build.rs` rebuilds whenever `data/js/mobile_responsive.js` changes (covered by M13's `cargo:rerun-if-changed=data/js`).
- Runtime: every WhatsApp Web frame loaded gets mobile-responsive treatment automatically; narrow windows collapse the sidebar matching karere v3. Video calls fullscreen the window and hide chrome.
- Risk: `mobile_responsive.js` performs WhatsApp-specific DOM manipulation; if the WhatsApp Web markup drifts, the script must be refreshed from karere v3.
- Depends on: M13 (render-process IPC) and M08 (window persistence / polling loop).
