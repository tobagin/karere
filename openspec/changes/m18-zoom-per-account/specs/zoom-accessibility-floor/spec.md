## ADDED Requirements

### Requirement: Accessibility floor GSetting
The schema SHALL define `webview-zoom` (`d`, default `1.0`) as the minimum linear zoom factor enforced across all zoom paths.

#### Scenario: Default does not constrain
- **WHEN** `webview-zoom` is at its default `1.0` and persisted zoom is `1.0`
- **THEN** the effective applied zoom is `1.0`

### Requirement: Effective zoom respects the floor
On every apply path (startup, account switch, keyboard step, headerbar click), the window SHALL compute `effective = max(persisted, webview_zoom_floor)` before calling `set_zoom_linear`, and SHALL persist that effective value.

#### Scenario: Floor lifts a too-small persisted value
- **WHEN** persisted account zoom is `0.9` and `webview-zoom` is `1.2`
- **AND** the window applies zoom
- **THEN** `set_zoom_linear(1.2)` is invoked
- **AND** the persisted value is rewritten to `1.2`

### Requirement: Zoom-out cannot cross the floor
The `win.zoom-out` action SHALL refuse to reduce the linear zoom below `webview-zoom`; the computed candidate `current / 1.1` is clamped up to the floor before apply.

#### Scenario: Floor at 1.2 blocks step-down
- **WHEN** current linear zoom is `1.2` and `webview-zoom` is `1.2`
- **AND** the user activates `win.zoom-out`
- **THEN** the applied linear zoom remains `1.2`
- **AND** no spurious "decreased" log/event is emitted
