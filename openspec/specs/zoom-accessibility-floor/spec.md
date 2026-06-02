### Requirement: Accessibility floor GSettings
The schema SHALL define `webview-zoom` (`b`, default `false`) as a master toggle and `zoom-level` (`d`, default `1.0`) as the minimum linear zoom factor. The effective floor SHALL be `zoom-level` (clamped to `[0.5, 3.0]`) when `webview-zoom` is `true`, and the hard CEF minimum (`0.5`) otherwise.

> Reconciled with shipped M22 schema: `webview-zoom` is a boolean enable, not a `d` floor; `zoom-level` carries the floor value. Per-account zoom is stored in `Account::zoom_level` (M20).

#### Scenario: Toggle off does not constrain
- **WHEN** `webview-zoom` is `false` and persisted zoom is `1.0`
- **THEN** the effective applied zoom is `1.0`

### Requirement: Effective zoom respects the floor
On every apply path (startup/first-paint, account switch, keyboard step, headerbar click), the shell SHALL compute `effective = max(persisted, floor)` before calling `set_zoom_linear`, and SHALL persist that effective value.

#### Scenario: Floor lifts a too-small persisted value
- **WHEN** persisted account zoom is `0.9`, `webview-zoom` is `true`, and `zoom-level` is `1.2`
- **AND** the shell applies zoom
- **THEN** `set_zoom_linear(1.2)` is invoked
- **AND** the persisted value is rewritten to `1.2`

### Requirement: Zoom-out cannot cross the floor
The `win.zoom-out` action SHALL refuse to reduce the linear zoom below the floor; the computed candidate `current / 1.1` is clamped up to the floor before apply.

#### Scenario: Floor at 1.2 blocks step-down
- **WHEN** current linear zoom is `1.2`, `webview-zoom` is `true`, and `zoom-level` is `1.2`
- **AND** the user activates `win.zoom-out`
- **THEN** the applied linear zoom remains `1.2`
- **AND** no spurious "decreased" log/event is emitted
