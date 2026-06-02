## 1. CEF zoom boundary

- [x] 1.1 Add `CefGtkArea::set_zoom_linear(&self, linear: f64)` that clamps to `[0.5, 3.0]` and calls `host.set_zoom_level(linear.ln() / 1.2_f64.ln())`. (Implemented on `KarereWebView` in `src/web_view.rs`; the actual widget — there is no `CefGtkArea`.)
- [x] 1.2 Add `CefGtkArea::get_zoom_linear(&self) -> f64` returning `(host.get_zoom_level() * 1.2_f64.ln()).exp()`.
- [x] 1.3 Unit-test round-trip: `set_zoom_linear(x); assert!((get_zoom_linear() - x).abs() < 1e-9)` for `x in {0.5, 1.0, 1.1, 1.331, 3.0}`. (Tests the pure `linear_to_cef`/`cef_to_linear` helpers, which carry the same math + clamp; the host-backed methods need a live browser.)
- [x] 1.4 Unit-test clamp: `set_zoom_linear(5.0)` results in `get_zoom_linear() ≈ 3.0`.

## 2. Persistence

- [x] 2.1 Add gschema key `zoom-level` (`d`, default `1.0`) for the pre-M20 fallback. (Already present from M22; reconciled — M20 landed, so it now serves as the accessibility-floor value, and per-account persistence uses `Account::zoom_level` directly.)
- [x] 2.2 Add helper `load_zoom_for_active_account()` that reads `Account::zoom_level` when M20 is present, else `gsettings.get_double("zoom-level")`. (M20 present → reads `manager().active().zoom_level`; `src/window.rs`.)
- [x] 2.3 Add helper `persist_zoom(linear: f64)` mirror to 2.2. (Writes via `AccountManager::set_zoom`, which saves without emitting `accounts-changed` to avoid a switcher rebuild-storm.)
- [x] 2.4 Wire `persist_zoom` into every change site (keyboard handler, headerbar handler, floor lift-up). (Keyboard + headerbar share `apply_and_persist_zoom`; floor lift-up persists in `apply_zoom_from_account`.)

## 3. Keyboard actions

- [x] 3.1 Replace M8 `win.zoom-in` stub with `let cur = get_zoom_linear(); set_zoom_linear(cur * 1.1); persist_zoom(get_zoom_linear());`. (`KarereWindow::zoom_step` — steps from the persisted value rather than `get_zoom_linear` so the floor is honoured even with no live browser.)
- [x] 3.2 Replace M8 `win.zoom-out` stub with the same pattern using `/ 1.1`, clamped up to `webview-zoom` floor.
- [x] 3.3 Replace M8 `win.zoom-reset` stub with `set_zoom_linear(max(1.0, floor)); persist_zoom(...)`.
- [x] 3.4 Manual: Ctrl+plus enlarges 10 %, Ctrl+minus shrinks 10 %, Ctrl+0 resets; close + reopen window preserves the value. (Verified live.)

## 4. First-paint application

- [x] 4.1 In the `on_load_end` handler, on first call per browser, read persisted linear zoom, apply floor, call `set_zoom_linear`. (`web_view::apply_zoom_from_account`, called from `ShellLoadHandler::on_load_end`; applied on every successful main-frame load — idempotent and naturally per-account via `account_for_browser`.)
- [x] 4.2 On active-account switch (M20), re-apply zoom from the new account's stored value. (`KarereWindow::switch_account` re-applies + updates the headerbar label.)
- [x] 4.3 Manual (M20): two accounts holding `1.0` and `1.5` retain independent levels across switches. (Verified live: active account's `zoom_level` climbed 1.0→1.1→1.21 while the background account stayed 1.0 in `accounts.json`.)

## 5. Accessibility floor

- [x] 5.1 Add gschema key `webview-zoom` (`d`, default `1.0`). (Present from M22 as a `b` master-toggle gating the `zoom-level` floor value, not a `d` floor — reconciled to the shipped schema. `web_view::zoom_floor` reads `webview-zoom ? zoom-level : ZOOM_MIN`.)
- [x] 5.2 In every apply path, compute `effective = max(persisted, floor)` and persist the effective value back. (`apply_and_persist_zoom` + `apply_zoom_from_account`.)
- [x] 5.3 Make `win.zoom-out` short-circuit when `current / 1.1 < floor`, leaving the zoom at the floor. (`zoom_step`: `(cur / 1.1).max(floor)`.)
- [x] 5.4 Manual: set `webview-zoom=1.2` in dconf, restart, confirm Ctrl+minus cannot reduce below 1.2. (Verified live with `webview-zoom=true` + `zoom-level=1.2`.)

## 6. Headerbar zoom-box

- [x] 6.1 Add gschema key `zoom-controls-headerbar` (`b`, default `false`). (Present from M22.)
- [x] 6.2 In `data/ui/window.blp`, add a `gtk::Box` to the headerbar with three `gtk::Button`s (`-`, label, `+`) and an overflow "reset" item. (`-` / `+` buttons flank a `zoom_label`; the reset action is bound to the percentage label button itself — click the `<int>%` to reset.)
- [x] 6.3 Bind the box visibility to `zoom-controls-headerbar` via `gtk::Settings` or `bind_property` on the GSetting wrapper. (`settings.bind(... "visible")`, GET flag, in `constructed`.)
- [x] 6.4 Update the `<int>%` label on every zoom change (subscribe to the same notify path that persistence uses). (`update_zoom_label` called from `apply_and_persist_zoom` and `switch_account`; seeded in `constructed`.)
- [x] 6.5 Wire the buttons to the same `win.zoom-*` actions used by the keyboard. (`action-name` in the blp.)
- [x] 6.6 Manual: with `zoom-controls-headerbar=true`, buttons appear, label tracks state, clicks behave identically to keyboard. (Verified live.)

## 7. Documentation

- [x] 7.1 Note in release notes that bumping `webview-zoom` above an account's persisted zoom will silently lift the account zoom to the floor. (CHANGELOG `[Unreleased]` → Added.)
- [x] 7.2 Document the M20 dependency and the pre-M20 single-key fallback behavior. (CHANGELOG note: M20 present → per-account; pre-M20 → shared `zoom-level`.)
