## ADDED Requirements

### Requirement: Window geometry persists across launches
The system SHALL bind the GSettings keys `window-width`, `window-height`, and `window-maximized` from the karere schema to the `KarereWindow`'s `default-width`, `default-height`, and `maximized` properties using bidirectional `Settings::bind` so geometry survives normal close and restart.

#### Scenario: Resize then relaunch restores size
- **WHEN** the user resizes the window to 1100x800 and then closes and re-launches the application
- **THEN** the newly constructed window opens at 1100x800

#### Scenario: Maximize then relaunch restores maximized state
- **WHEN** the user maximizes the window and then closes and re-launches the application
- **THEN** the newly constructed window opens maximized

#### Scenario: First-launch defaults
- **WHEN** no `window-width` or `window-height` value has ever been written for the current user
- **THEN** the window opens at the default size declared in the karere gschema

### Requirement: Close-button behaviour is driven by `close-button-action` GSetting
The system SHALL read the `close-button-action` GSetting on every `close_request` invocation. When the value is `"background"` the system SHALL call `set_visible(false)` on the window and return `glib::Propagation::Stop`. When the value is `"quit"` the system SHALL fall through to the existing M4 CEF-close gate. The system SHALL also install a `connect_changed("close-button-action", ...)` handler so runtime toggles take effect on the next close request without restart.

#### Scenario: Background mode hides the window
- **WHEN** `close-button-action` is `"background"` and the user clicks the close button
- **THEN** the window is hidden via `set_visible(false)` and the application process continues running

#### Scenario: Quit mode tears down via M4 path
- **WHEN** `close-button-action` is `"quit"` and the user clicks the close button
- **THEN** the existing M4 CEF clean-shutdown flow runs and the application exits

#### Scenario: Runtime toggle takes effect immediately
- **WHEN** `close-button-action` is changed from `"quit"` to `"background"` while the window is open and the user then clicks close
- **THEN** the window hides instead of exiting, with no application restart required

### Requirement: `start-in-background` GSetting gates initial `present()`
The system SHALL read the `start-in-background` GSetting in `connect_command_line` after window construction. When the value is `true` AND a tray is configured, the system SHALL skip the `present()` call on the constructed window. In all other cases the window SHALL be presented normally.

#### Scenario: Start in background with tray configured
- **WHEN** `start-in-background` is `true` and a tray is configured and the application launches
- **THEN** the window is constructed but `present()` is not called and no window appears on screen

#### Scenario: Start in background without tray falls back to presenting
- **WHEN** `start-in-background` is `true` and no tray is configured and the application launches
- **THEN** the window is presented normally so it is not unreachable

#### Scenario: Default behaviour
- **WHEN** `start-in-background` is `false` and the application launches
- **THEN** the window is presented normally regardless of tray state
