## ADDED Requirements

### Requirement: `theme` GSetting drives `adw::StyleManager` color-scheme
The system SHALL read the `theme` GSetting during `KarereApplication::startup` and map its value to an `adw::ColorScheme` as follows: `"system"`→`ColorScheme::Default`, `"light"`→`ColorScheme::ForceLight`, `"dark"`→`ColorScheme::ForceDark`. The mapping result SHALL be applied via `adw::StyleManager::default().set_color_scheme(...)`. The system SHALL also install a `connect_changed("theme", ...)` handler that re-runs the mapping so theme changes take effect immediately without restarting the application.

#### Scenario: Initial dark theme honoured at startup
- **WHEN** the `theme` GSetting is `"dark"` before launch and the application starts
- **THEN** `adw::StyleManager::default().color_scheme()` returns `ColorScheme::ForceDark`

#### Scenario: Live switch to light theme
- **WHEN** the application is running with `theme=dark` and the user (or a CLI write) changes `theme` to `"light"`
- **THEN** `adw::StyleManager::default().color_scheme()` becomes `ColorScheme::ForceLight` and the UI re-renders in light mode without restarting

#### Scenario: `system` value follows desktop preference
- **WHEN** the `theme` GSetting is `"system"`
- **THEN** the color scheme is `ColorScheme::Default` and libadwaita honours the desktop's `prefer-color-scheme` portal value

#### Scenario: Unknown value falls back to system
- **WHEN** the `theme` GSetting holds a value not in `{"system","light","dark"}`
- **THEN** the system applies `ColorScheme::Default` and logs a `g_warning` naming the offending value
