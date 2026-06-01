## 1. CEF zoom boundary

- [ ] 1.1 Add `CefGtkArea::set_zoom_linear(&self, linear: f64)` that clamps to `[0.5, 3.0]` and calls `host.set_zoom_level(linear.ln() / 1.2_f64.ln())`.
- [ ] 1.2 Add `CefGtkArea::get_zoom_linear(&self) -> f64` returning `(host.get_zoom_level() * 1.2_f64.ln()).exp()`.
- [ ] 1.3 Unit-test round-trip: `set_zoom_linear(x); assert!((get_zoom_linear() - x).abs() < 1e-9)` for `x in {0.5, 1.0, 1.1, 1.331, 3.0}`.
- [ ] 1.4 Unit-test clamp: `set_zoom_linear(5.0)` results in `get_zoom_linear() ≈ 3.0`.

## 2. Persistence

- [ ] 2.1 Add gschema key `zoom-level` (`d`, default `1.0`) for the pre-M20 fallback.
- [ ] 2.2 Add helper `load_zoom_for_active_account()` that reads `Account::zoom_level` when M20 is present, else `gsettings.get_double("zoom-level")`.
- [ ] 2.3 Add helper `persist_zoom(linear: f64)` mirror to 2.2.
- [ ] 2.4 Wire `persist_zoom` into every change site (keyboard handler, headerbar handler, floor lift-up).

## 3. Keyboard actions

- [ ] 3.1 Replace M8 `win.zoom-in` stub with `let cur = get_zoom_linear(); set_zoom_linear(cur * 1.1); persist_zoom(get_zoom_linear());`.
- [ ] 3.2 Replace M8 `win.zoom-out` stub with the same pattern using `/ 1.1`, clamped up to `webview-zoom` floor.
- [ ] 3.3 Replace M8 `win.zoom-reset` stub with `set_zoom_linear(max(1.0, floor)); persist_zoom(...)`.
- [ ] 3.4 Manual: Ctrl+plus enlarges 10 %, Ctrl+minus shrinks 10 %, Ctrl+0 resets; close + reopen window preserves the value.

## 4. First-paint application

- [ ] 4.1 In the `on_load_end` handler, on first call per browser, read persisted linear zoom, apply floor, call `set_zoom_linear`.
- [ ] 4.2 On active-account switch (M20), re-apply zoom from the new account's stored value.
- [ ] 4.3 Manual (M20): two accounts holding `1.0` and `1.5` retain independent levels across switches.

## 5. Accessibility floor

- [ ] 5.1 Add gschema key `webview-zoom` (`d`, default `1.0`).
- [ ] 5.2 In every apply path, compute `effective = max(persisted, floor)` and persist the effective value back.
- [ ] 5.3 Make `win.zoom-out` short-circuit when `current / 1.1 < floor`, leaving the zoom at the floor.
- [ ] 5.4 Manual: set `webview-zoom=1.2` in dconf, restart, confirm Ctrl+minus cannot reduce below 1.2.

## 6. Headerbar zoom-box

- [ ] 6.1 Add gschema key `zoom-controls-headerbar` (`b`, default `false`).
- [ ] 6.2 In `data/ui/window.blp`, add a `gtk::Box` to the headerbar with three `gtk::Button`s (`-`, label, `+`) and an overflow "reset" item.
- [ ] 6.3 Bind the box visibility to `zoom-controls-headerbar` via `gtk::Settings` or `bind_property` on the GSetting wrapper.
- [ ] 6.4 Update the `<int>%` label on every zoom change (subscribe to the same notify path that persistence uses).
- [ ] 6.5 Wire the buttons to the same `win.zoom-*` actions used by the keyboard.
- [ ] 6.6 Manual: with `zoom-controls-headerbar=true`, buttons appear, label tracks state, clicks behave identically to keyboard.

## 7. Documentation

- [ ] 7.1 Note in release notes that bumping `webview-zoom` above an account's persisted zoom will silently lift the account zoom to the floor.
- [ ] 7.2 Document the M20 dependency and the pre-M20 single-key fallback behavior.
