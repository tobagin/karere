## ADDED Requirements

### Requirement: Logarithmic CEF zoom conversion
The shell SHALL convert between the linear zoom factor used by Karere UI/storage and CEF's logarithmic `BrowserHost::set_zoom_level` API via `cef = ln(linear) / ln(1.2)` and `linear = exp(cef * ln(1.2))`, and SHALL clamp linear input to `[0.5, 3.0]` before applying.

#### Scenario: Set zoom to 1.1 linear
- **WHEN** `CefGtkArea::set_zoom_linear(1.1)` is called
- **THEN** `BrowserHost::set_zoom_level(ln(1.1)/ln(1.2))` is invoked on the active browser

#### Scenario: Get zoom returns linear
- **WHEN** CEF reports `get_zoom_level() == ln(1.21)/ln(1.2)`
- **AND** `CefGtkArea::get_zoom_linear()` is called
- **THEN** the returned value is within `1e-9` of `1.21`

#### Scenario: Clamp out-of-range input
- **WHEN** `CefGtkArea::set_zoom_linear(5.0)` is called
- **THEN** the value passed to CEF corresponds to linear `3.0`, not `5.0`

### Requirement: Keyboard zoom actions step ±10 % or reset
The window SHALL handle `win.zoom-in`, `win.zoom-out`, and `win.zoom-reset` actions by multiplying the current linear zoom by `1.1`, dividing by `1.1`, or setting it to `1.0` respectively, then applying via `set_zoom_linear` and persisting.

#### Scenario: Ctrl+plus from 1.0
- **WHEN** the user activates `win.zoom-in` while current linear zoom is `1.0`
- **THEN** the new linear zoom is `1.1`
- **AND** the value is persisted (per-account in M20, GSetting `zoom-level` pre-M20)

#### Scenario: Ctrl+0 resets
- **WHEN** the user activates `win.zoom-reset` while current linear zoom is `1.331`
- **THEN** the new linear zoom is `1.0`

### Requirement: Apply persisted zoom on first paint
On window startup or active-account change, the window SHALL read the persisted linear zoom, apply the accessibility floor, and call `set_zoom_linear` after the first `on_load_end` callback for that browser.

#### Scenario: Startup with persisted 1.2
- **WHEN** the window opens an account whose stored linear zoom is `1.2`
- **AND** the first `on_load_end` fires
- **THEN** `set_zoom_linear(1.2)` is invoked once

#### Scenario: Per-account isolation (M20)
- **WHEN** two accounts hold linear zooms `1.0` and `1.5`
- **AND** the user switches between them
- **THEN** the applied zoom matches the active account's stored value

### Requirement: Headerbar zoom-box (opt-in)
When GSetting `zoom-controls-headerbar` is `true`, the headerbar SHALL display a `gtk::Box` containing `-`, `<int>%` label, `+` buttons and a reset overflow; the label SHALL update on every zoom change.

#### Scenario: Box hidden by default
- **WHEN** `zoom-controls-headerbar` is `false`
- **THEN** the headerbar zoom-box is not visible

#### Scenario: Buttons trigger same actions
- **WHEN** `zoom-controls-headerbar` is `true`
- **AND** the user clicks `+`
- **THEN** the same effect as `win.zoom-in` occurs (zoom × 1.1, persisted, label updates)

### Requirement: Persisted GSettings keys
The schema SHALL define `zoom-level` (`d`, default `1.0`) — the accessibility-floor value, also the pre-M20 single-key zoom fallback — and `zoom-controls-headerbar` (`b`, default `false`) for the headerbar opt-in. With M20 present, per-account zoom is stored in `Account::zoom_level`, not `zoom-level`.

#### Scenario: Defaults
- **WHEN** the schema is freshly installed
- **THEN** `zoom-level` reads as `1.0` and `zoom-controls-headerbar` reads as `false`
