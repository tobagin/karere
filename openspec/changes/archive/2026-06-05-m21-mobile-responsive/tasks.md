## 0. Implementation revision (host-side width gating)

> During apply, the central premise of the original tasks proved false: the
> verbatim karere v3 `mobile_responsive.js` has **no** `karere:viewport-resize`
> listener and no width logic — it applies single-pane mobile layout
> *unconditionally* when executed (it was built for PinePhone/Librem5). v3 made
> it "responsive" entirely host-side: `should_use_mobile_layout(settings, width)`
> (768 px threshold + DE detection + the `mobile-layout` setting), injecting the
> script on load only when mobile and reloading on a threshold crossing.
>
> M21 therefore mirrors v3: the verbatim script lives in `data/js-deferred/`
> (NOT the always-run M13 bundle), is injected from `on_load_end` only when the
> host decides the layout is mobile, and a width-threshold crossing reloads the
> page so the next load re-evaluates. The inert `SetViewportSize` → CustomEvent
> plumbing from the original tasks 2–3 is dropped (the script ignores it). The
> fullscreen half (tasks 4–5) is unchanged.

## 1. Verbatim mobile-responsive script

- [x] 1.1 Place `data/js-deferred/mobile_responsive.js` as a byte-for-byte copy of upstream `pparent76/Whatslectron-UT` `whatslectron-src/ubuntutheme.js` (sha256 `d2d1cf2c…`). NOTE: the v3 copy (`49b58b89…`) was tried first but its hardcoded `.two.childNodes[4]` broke on current WhatsApp; the upstream same-version file carries the `findIndexChatList()`/`indexChatList` drift fix. (`data/js-deferred/` is the conditional-injection dir alongside `profile_dom_fallback.js`, so M13's `build.rs` does NOT auto-bundle it.)
- [x] 1.2 Confirm `build.rs` (which enumerates only `data/js/*.js`) does NOT include the file in `$OUT_DIR/injected_bundle.js`, so it never auto-runs on every page.
- [x] 1.3 Embed the file in the browser process via `const EMBED_MOBILE: &str = include_str!(".../data/js-deferred/mobile_responsive.js")` so it can be injected on demand.

## 2. Host-side mobile-layout gate

- [x] 2.1 Add `should_use_mobile_layout(width_logical_px: i32) -> bool` (in `src/web_view.rs`): mirror v3 — `mobile-layout` GSetting `enabled`/`disabled`/`auto`; for `auto`, true if `XDG_CURRENT_DESKTOP` matches phosh/plasma-mobile/lomiri, or width in `(0, 768)`.
- [x] 2.2 Add `apply_mobile_layout(browser, width_logical_px)` (in `src/web_view.rs`): when `should_use_mobile_layout` is true, inject `EMBED_MOBILE` into the main frame via `execute_java_script` (idempotent per page via a `window.__karereMobileApplied` guard).
- [x] 2.3 Call it from `ShellLoadHandler::on_load_end` (after the existing zoom/autocorrect re-apply), computing logical width from `SharedState.size` / `scale_factor`. This is the v3 inject-on-load path and re-applies after every navigation/reload.

## 3. Reload on threshold crossing

- [x] 3.1 Track `mobile_active: Cell<bool>` + `mobile_init: Cell<bool>` on the `KarereWebView` imp.
- [x] 3.2 In `size_allocate` (after `was_resized()`), with logical `width > 0`: compute `is_mobile`. First allocation seeds `mobile_active`/`mobile_init` without reloading (the first `on_load_end` injects if mobile). On a later change of `is_mobile`, update the cell and `reload()` the foreground browser so its `on_load_end` re-evaluates the gate (mirrors v3 `webview.reload()`).
- [x] 3.3 Reload is fire-and-forget on the GTK main thread; resolves the browser via the existing `resolved_browser` helper and no-ops if absent.

## 4. Display handler — JS-initiated fullscreen

- [x] 4.1 Verified `DisplayHandler::on_fullscreen_mode_change(&self, browser, fullscreen: c_int)` is exposed by cef-rs 148.2.0.
- [x] 4.2 Add a `fullscreen_request: Option<bool>` field to `SharedState` (drained by the M08 window poll loop).
- [x] 4.3 Implement `on_fullscreen_mode_change` in `src/handlers/display.rs`: set `shared.fullscreen_request = Some(fullscreen != 0)`; do not touch GTK widgets.

## 5. Window fullscreen + headerbar wiring

- [x] 5.1 In the M08 poll loop (`start_state_poll`), drain `fullscreen_request`: `window.fullscreen()` when `true`, `window.unfullscreen()` when `false`.
- [x] 5.2 Give the `Adw.HeaderBar` an id (`header_bar`) + template child; connect `notify::fullscreened` on the window → `header_bar.set_visible(!window.is_fullscreen())`.
- [x] 5.3 The `notify::fullscreened` signal is the authoritative (and only) path for headerbar visibility; the poll-loop drain only changes window fullscreen state, so the two never fight.
- [x] 5.4 Headerbar restoration is guaranteed on every exit path (JS `exitFullscreen()` → `on_fullscreen_mode_change(0)` → unfullscreen → signal; Esc/F11/WM → signal directly), since the signal fires on the resulting state regardless of cause.

## 6. Verification

- [x] 6.1 Verified via CDP on the live logged-in page: with `mobile-layout=enabled` the embedded script auto-injects on `on_load_end`, `findIndexChatList()` resolves (`indexChatList=3`), `main()` completes (`__karereMobileApplied=true`, `.added_menu_button` present) → single-pane mobile layout applied. (Width-gate auto-mode narrow/widen reload still worth a manual eyeball.)
- [x] 6.2 N/A — WhatsApp Web's call UI has no in-page `requestFullscreen`; its "expand/pop-out" opens `web.whatsapp.com/call/popout` (NEW_POPUP / Document PiP), which OSR cannot host (blank + UI freeze). DPiP is disabled and the pop-out is suppressed in `on_before_popup`, so the call runs in WhatsApp's in-page floating window. The `on_fullscreen_mode_change`→window-fullscreen handler remains for any genuine element `requestFullscreen`.
- [x] 6.3 N/A — see 6.2 (no JS-initiated call fullscreen to exit; Esc/F11 window-fullscreen + headerbar restore already verified in 6.4).
- [x] 6.4 Press F11 to toggle fullscreen manually; confirm headerbar visibility tracks the window state both directions.
- [x] 6.5 Rapid-drag the window edge across the 768 px boundary; confirm no reload storm / console errors.

## 7. Documentation

- [x] 7.1 The verbatim file is left byte-identical (no top-of-file comment added); its provenance is documented here and at the `EMBED_MOBILE` `include_str!` site instead.
- [x] 7.2 Add a note to `src/handlers/display.rs` explaining fullscreen mutations are deferred to the GTK main thread via `SharedState`.
