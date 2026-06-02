## ADDED Requirements

### Requirement: Enhanced focus indicators toggled by CSS class
The application SHALL expose a boolean GSettings key `focus-indicators-enhanced` (default `false`). When `true`, the application SHALL add the CSS class `enhanced-focus` to the root window; when `false`, it SHALL remove the class. A bundled stylesheet resource SHALL define focus styling scoped under `.enhanced-focus` (a 3 px accent-color outline with 2 px offset on focused descendants).

#### Scenario: Enabling enhanced focus shows accent-color rings
- **WHEN** the user sets `focus-indicators-enhanced` to `true`
- **THEN** the root window has the CSS class `enhanced-focus`
- **AND** focused buttons and entries render a 3 px accent-color outline with 2 px offset

#### Scenario: Disabling enhanced focus restores default rings
- **WHEN** `focus-indicators-enhanced` transitions from `true` to `false`
- **THEN** the `enhanced-focus` class is removed from the root window
- **AND** widgets revert to libadwaita's default focus styling

#### Scenario: Stylesheet loaded once at application startup
- **WHEN** the application starts
- **THEN** a single `gtk::CssProvider` loaded from the bundled resource is added to the default display at `STYLE_PROVIDER_PRIORITY_APPLICATION`
- **AND** no per-window stylesheet allocation occurs
