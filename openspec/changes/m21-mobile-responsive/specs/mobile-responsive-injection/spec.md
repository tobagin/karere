## ADDED Requirements

### Requirement: Verbatim karere v3 mobile-responsive script
The application SHALL ship `data/js/mobile_responsive.js` as a byte-for-byte copy of karere v3's `src/mobile_responsive.js` so that WhatsApp Web layout adapts identically under CEF.

#### Scenario: Source parity
- **WHEN** the file `data/js/mobile_responsive.js` is compared against `karere/src/mobile_responsive.js`
- **THEN** the contents are identical
- **AND** no local edits, formatting changes, or comment additions have been applied

#### Scenario: Bundle inclusion
- **WHEN** `build.rs` runs and concatenates `data/js/*.js`
- **THEN** `mobile_responsive.js` is included in `$OUT_DIR/injected_bundle.js`
- **AND** it appears after `00-bootstrap.js` per lexical ordering

### Requirement: Viewport size delivered on every GTK size allocation
The browser process SHALL send a `BrowserMessage::SetViewportSize { w, h }` IPC to the renderer for every `size_allocate` event on the CEF GTK area.

#### Scenario: Allocation triggers IPC
- **WHEN** `src/cef_gtk_area.rs::size_allocate` fires with width `w` and height `h`
- **THEN** the host calls `host.was_resized()` first (existing behavior)
- **AND** then sends `BrowserMessage::SetViewportSize { w, h }` over the M13 process-message channel

#### Scenario: Resize during drag
- **WHEN** the user drags the window edge across multiple pixel deltas
- **THEN** each allocation produces a fresh IPC
- **AND** no debouncing is performed in the browser process

### Requirement: Bootstrap dispatches viewport CustomEvent
The M13 bootstrap script SHALL translate inbound `SetViewportSize` IPCs into a `karere:viewport-resize` `CustomEvent` on `document`.

#### Scenario: IPC received
- **WHEN** the renderer-side dispatcher receives a `BrowserMessage::SetViewportSize { w, h }`
- **THEN** the bootstrap calls `document.dispatchEvent(new CustomEvent('karere:viewport-resize', { detail: { w, h } }))`

#### Scenario: Listener pre-registered
- **WHEN** `on_context_created` fires for the main frame
- **THEN** the bootstrap's IPC dispatcher is registered before `mobile_responsive.js` runs
- **AND** the first `SetViewportSize` IPC after page load reaches `mobile_responsive.js`'s listener

#### Scenario: Mobile-responsive script reacts
- **WHEN** `mobile_responsive.js` receives a `karere:viewport-resize` event with narrow `w` (below its sidebar-collapse breakpoint)
- **THEN** the WhatsApp Web sidebar collapses to the single-pane layout
- **AND** the visual result matches karere v3 on an equivalently-sized window
