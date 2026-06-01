## Context

Karere v3 (WebKitGTK) used a linear zoom factor where `set_zoom_level(1.1)` meant "110 %". CEF instead exposes a logarithmic level on `BrowserHost`: each unit is a factor of `1.2`, and zero is 100 %. So `cef = ln(linear) / ln(1.2)` and `linear = exp(cef * ln(1.2))`. v3 stored per-account zoom as a linear `f64` in `Account` and applied it in `on_load_end`; v4 keeps that storage model but converts at the CEF boundary.

M8 already declared `win.zoom-in` / `win.zoom-out` / `win.zoom-reset` as stub actions so keybindings could be wired in the UI; this milestone supplies the handler bodies. M20 (multi-account) will add `Account::zoom_level`; we ship a single-key GSetting fallback so M18 is independently testable.

## Goals / Non-Goals

**Goals:**
- 10 % step Ctrl+plus / Ctrl+minus / Ctrl+0 keyboard zoom matching v3 feel.
- Per-account persistence with M20 `Account::zoom_level`; clean single-setting fallback when M20 not yet present.
- Accessibility floor (`webview-zoom`) that low-vision users can pin in dconf and that the UI cannot violate.
- Opt-in headerbar zoom-box (`-`, `<int>%`, `+`, reset overflow) for users who prefer mouse-only operation.
- Correct logarithmic conversion on the CEF boundary so the stored values stay linear and portable.

**Non-Goals:**
- Ctrl+wheel zoom (CEF wheel events are page-scroll; building a key+wheel intercept is deferred).
- Per-tab zoom UI (Karere has one tab per account; per-tab UI would only matter once we add real multi-tab).
- Page-level zoom memory across domains (CEF's per-origin cache is sufficient for our needs).

## Decisions

- **Storage stays linear.** Per-account `Account::zoom_level: f64` (M20) or `zoom-level` GSetting (pre-M20). All UI math operates on the linear value; conversion lives only inside `CefGtkArea::set_zoom_linear` / `get_zoom_linear`.
- **Clamp at the CEF boundary.** `set_zoom_linear` clamps input to `[0.5, 3.0]` before converting, so callers cannot accidentally push CEF into pathological levels.
- **Accessibility floor is read every apply.** On startup, account-switch, and after each step we compute `effective = max(persisted, webview_zoom_floor)` and write that back so the floor is sticky.
- **Apply after first `on_load_end`.** Setting zoom before the first paint is racy in CEF; we defer to the first load-end callback per browser, then on every subsequent step we apply immediately.
- **Headerbar zoom-box is opt-in.** Default `zoom-controls-headerbar=false` keeps the chrome clean for keyboard users; the label updates by subscribing to the same zoom-changed signal that persistence uses.
- **Step factor 1.1 (matches v3) not 1.2 (CEF unit).** Storing linear means the step is independent of CEF's logarithmic unit; conversion absorbs the mismatch.

## Risks / Trade-offs

- **M20 dependency.** If M20 slips, the pre-M20 fallback (`zoom-level` GSetting) is account-agnostic: switching accounts won't restore distinct levels. Acceptable for the short fallback window.
- **Floor surprises users who lowered zoom intentionally.** If `webview-zoom` is bumped above the persisted account zoom, the next apply silently lifts the account zoom to the floor. Document in release notes.
- **Headerbar label flicker.** Re-rendering the `<int>%` label on every step is cheap, but on rapid Ctrl+plus mashing the label could trail the CEF state by one frame. Acceptable; the keyboard accelerator is authoritative.
- **Logarithm precision.** `exp(ln(linear) / ln(1.2) * ln(1.2))` round-trips with `~1e-15` error, well below user-visible. We don't snap back to canonical 10 % steps; the stored value is whatever the user accumulated.
