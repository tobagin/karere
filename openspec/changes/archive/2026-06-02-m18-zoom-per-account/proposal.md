## Why

Karere v3 (`window.rs:2292-2337`) wires Ctrl+plus/minus/0 to step zoom in 10 % increments and persists per-account. v4 must preserve that UX while replacing WebKit's linear `set_zoom_level` with CEF's logarithmic `BrowserHost::set_zoom_level(f64)` (where `cef = log(linear) / log(1.2)`). We also want an accessibility floor (minimum zoom) and an optional headerbar zoom-box for users who don't use keyboard shortcuts.

## What Changes

- Add logarithmic ↔ linear conversion helpers on `CefGtkArea` (`set_zoom_linear`, `get_zoom_linear`) with `[0.5, 3.0]` clamp.
- Wire `win.zoom-in` / `win.zoom-out` / `win.zoom-reset` (stubbed in M8) to step the active account's zoom by ±10 % or reset to 1.0; apply after first `on_load_end`.
- Persist per-account via M20's `Account::zoom_level`; before M20 lands, fall back to a single `zoom-level` GSetting.
- Add an accessibility floor (`webview-zoom` GSetting, linear `f64`, default 1.0): `zoom-out` cannot drop below it; on apply, effective zoom is `max(persisted, floor)`.
- Add an opt-in headerbar zoom-box (`-`, `<int>%`, `+`, reset overflow) bound to `zoom-controls-headerbar` GSetting (bool, default false), label re-renders on every zoom change.
- gschema additions: `zoom-level` (d, 1.0), `webview-zoom` (d, 1.0), `zoom-controls-headerbar` (b, false).

## Capabilities

### New Capabilities
- `zoom-control`: Per-account web zoom with logarithmic CEF translation, keyboard + headerbar UI, and persistence.
- `zoom-accessibility-floor`: Minimum zoom level enforced across all UI paths to support low-vision users.

### Modified Capabilities
<!-- none -->

## Impact

- `src/cef_gtk_area.rs`: add `set_zoom_linear` / `get_zoom_linear` (log/linear conversion, clamp).
- `src/window.rs`: replace M8 zoom action stubs with real handlers; load + apply zoom on first paint; persist on every change.
- `data/ui/window.blp`: add headerbar zoom-box (`gtk::Box` with three `gtk::Button`s + overflow), visibility bound to `zoom-controls-headerbar`.
- gschema XML: three new keys (`zoom-level`, `webview-zoom`, `zoom-controls-headerbar`).
- Depends on M20 `Account::zoom_level` for multi-account isolation; pre-M20 fallback documented.
- Non-goals: Ctrl+wheel zoom (CEF wheel = page-scroll; would need a key+wheel intercept later) and per-tab zoom UI.
