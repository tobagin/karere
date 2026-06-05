# mobile-responsive-injection Specification

## Purpose

Adapt WhatsApp Web's layout for mobile/narrow form factors under CEF by injecting a verbatim upstream responsive script on-demand. The host process decides when mobile layout applies (from window width and settings) and injects the deferred script on load, reloading the page when the width crosses the mobile/desktop threshold.

## Requirements

### Requirement: Verbatim upstream mobile-responsive script

The application SHALL ship `data/js-deferred/mobile_responsive.js` as a
byte-for-byte copy of the upstream Whatslectron-UT responsive script so that
WhatsApp Web layout adapts under CEF.

> Source revision: M21 initially copied karere v3's `src/mobile_responsive.js`
> (`@version 20251009`), but that copy hardcodes `.two.childNodes[4]` as the chat
> list and broke against current WhatsApp Web (the chat-list pane moved to a
> different index, so `main()` threw at `My.chatListHeader`). The upstream source
> — `pparent76/Whatslectron-UT` `whatslectron-src/ubuntutheme.js`, same
> `@version 20251009`, sha256 `d2d1cf2c…` — carries the drift fix: a
> `findIndexChatList()` that locates the `[role="grid"]`'s top-level `.two` child
> and drives all selectors off a dynamic `indexChatList`. Per user direction, the
> file is the verbatim upstream `ubuntutheme.js`.
>
> Behavioural note: the script applies single-pane mobile layout *unconditionally*
> when executed and has no viewport/breakpoint logic of its own. Layout is made
> responsive entirely host-side (inject-when-mobile + reload on threshold
> crossing), so the script is a **deferred, conditionally-injected** asset (like
> `profile_dom_fallback.js`) rather than part of the always-run M13 bundle.

#### Scenario: Source parity
- **WHEN** the file `data/js-deferred/mobile_responsive.js` is compared against upstream `pparent76/Whatslectron-UT` `whatslectron-src/ubuntutheme.js`
- **THEN** the contents are identical (sha256 `d2d1cf2cfbf14de06da76da0332340426f484353aeea7bc28a108229f445e156`)
- **AND** no local edits, formatting changes, or comment additions have been applied

#### Scenario: Not auto-bundled
- **WHEN** `build.rs` enumerates `data/js/*.js` for `$OUT_DIR/injected_bundle.js`
- **THEN** `mobile_responsive.js` is NOT included (it lives in `data/js-deferred/`)
- **AND** it is embedded separately via `include_str!` for on-demand injection

### Requirement: Host decides mobile layout from window width and settings

The browser process SHALL decide whether mobile layout applies using
`should_use_mobile_layout`, mirroring karere v3: the `mobile-layout` GSetting
(`enabled` / `disabled` / `auto`), and for `auto`, a mobile desktop environment
(`XDG_CURRENT_DESKTOP` containing phosh / plasma-mobile / lomiri) or a logical
window width in the open interval `(0, 768)` px.

#### Scenario: Narrow window is mobile
- **WHEN** the setting is `auto` and the logical window width is below 768 px (and > 0)
- **THEN** `should_use_mobile_layout` returns true

#### Scenario: Wide window is desktop
- **WHEN** the setting is `auto`, the desktop environment is not a mobile one, and the logical width is ≥ 768 px
- **THEN** `should_use_mobile_layout` returns false

#### Scenario: Explicit override
- **WHEN** the setting is `enabled` (or `disabled`)
- **THEN** `should_use_mobile_layout` returns true (or false) regardless of width

### Requirement: Mobile script injected on load when mobile

The application SHALL inject the verbatim script into the main frame from
`on_load_end` whenever the host decides the layout is mobile, so layout is applied
on first paint and re-applied after every navigation/reload.

#### Scenario: Inject on mobile load
- **WHEN** a main-frame load finishes successfully and `should_use_mobile_layout` is true for the current width
- **THEN** the host executes the embedded `mobile_responsive.js` in that frame
- **AND** the WhatsApp Web sidebar collapses to the single-pane layout, matching karere v3

#### Scenario: No injection on desktop load
- **WHEN** a main-frame load finishes and `should_use_mobile_layout` is false
- **THEN** the host does not inject the script and the desktop multi-pane layout is preserved

#### Scenario: Idempotent re-apply
- **WHEN** injection is attempted more than once within the same page lifetime
- **THEN** a `window.__karereMobileApplied` guard prevents running the script twice

### Requirement: Width-threshold crossing reloads to re-evaluate the gate

The embedding widget SHALL reload the page when the logical width crosses the
mobile/desktop threshold, so the next `on_load_end` re-evaluates the gate (the
verbatim script cannot un-apply its DOM/CSS mutations without a reload — matching
karere v3).

#### Scenario: First allocation seeds state without reload
- **WHEN** `size_allocate` fires for the first time with a non-zero logical width
- **THEN** the widget records the current mobile/desktop state without reloading (the first `on_load_end` handles initial injection)

#### Scenario: Crossing into mobile
- **WHEN** the logical width shrinks across 768 px so `should_use_mobile_layout` flips to true
- **THEN** the widget reloads the foreground browser, whose `on_load_end` then injects the script

#### Scenario: Crossing into desktop
- **WHEN** the logical width grows across 768 px so `should_use_mobile_layout` flips to false
- **THEN** the widget reloads the foreground browser, whose `on_load_end` skips injection, restoring desktop layout
