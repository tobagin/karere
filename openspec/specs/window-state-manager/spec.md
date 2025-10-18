# window-state-manager Specification

## Purpose
TBD - created by archiving change refactor-window-components. Update Purpose after archive.
## Requirements
### Requirement: Window State Manager Class Creation
The application SHALL provide a `WindowStateManager` class in `src/managers/WindowStateManager.vala` that persists and restores window geometry and state.

#### Scenario: Window state manager instantiation
**GIVEN** the application creates a new Window
**WHEN** WindowStateManager is instantiated
**THEN** the manager receives Settings and Adw.ApplicationWindow references
**AND** the manager is ready to restore and track state

### Requirement: Window State Restoration
The window state manager SHALL restore window geometry and state from GSettings on startup.

#### Scenario: Restore window size from settings
**GIVEN** WindowStateManager is instantiated
**AND** GSettings contains window-width=1200 and window-height=800
**WHEN** restore_state() is called
**THEN** the window's default size is set to 1200x800

#### Scenario: Restore maximized state
**GIVEN** WindowStateManager is instantiated
**AND** GSettings contains window-maximized=true
**WHEN** restore_state() is called
**THEN** the window is maximized

#### Scenario: Restore normal state
**GIVEN** WindowStateManager is instantiated
**AND** GSettings contains window-maximized=false
**WHEN** restore_state() is called
**THEN** the window is not maximized

#### Scenario: Settings unavailable fallback
**GIVEN** WindowStateManager is instantiated with null Settings
**WHEN** restore_state() is called
**THEN** a warning is logged
**AND** the window default size is set to 1200x800 (fallback)
**AND** the window is not maximized

### Requirement: Window State Tracking
The window state manager SHALL monitor window property changes and persist them to GSettings.

#### Scenario: Start tracking window changes
**GIVEN** WindowStateManager is instantiated
**WHEN** start_tracking() is called
**THEN** the manager connects to window notify["maximized"] signal
**AND** the manager connects to window notify["default-width"] signal
**AND** the manager connects to window notify["default-height"] signal

#### Scenario: Window resize tracking
**GIVEN** tracking is started
**WHEN** the window is resized to 1400x900
**THEN** the notify["default-width"] and notify["default-height"] signals fire
**AND** save_state() is called automatically
**AND** GSettings is updated with window-width=1400 and window-height=900

#### Scenario: Window maximize tracking
**GIVEN** tracking is started
**WHEN** the window is maximized
**THEN** the notify["maximized"] signal fires
**AND** save_state() is called automatically
**AND** GSettings is updated with window-maximized=true

#### Scenario: Window unmaximize tracking
**GIVEN** tracking is started and window is maximized
**WHEN** the window is unmaximized
**THEN** the notify["maximized"] signal fires
**AND** save_state() is called automatically
**AND** GSettings is updated with window-maximized=false

### Requirement: Window State Persistence
The window state manager SHALL save window geometry and state to GSettings.

#### Scenario: Save window size
**GIVEN** the window has default size 1024x768
**WHEN** save_state() is called
**THEN** GSettings window-width is set to 1024
**AND** GSettings window-height is set to 768

#### Scenario: Save maximized state
**GIVEN** the window is maximized
**WHEN** save_state() is called
**THEN** GSettings window-maximized is set to true

#### Scenario: Save on window close
**GIVEN** tracking is started
**WHEN** Window.close_request() is called
**THEN** save_state() is explicitly called
**AND** current window state is persisted before closing

#### Scenario: Settings unavailable during save
**GIVEN** Settings reference is null
**WHEN** save_state() is called
**THEN** the method returns early without error
**AND** no GSettings operations are attempted

### Requirement: Error Handling and Logging
The window state manager SHALL handle errors gracefully and provide debug logging.

#### Scenario: Null settings handling
**GIVEN** Settings is null
**WHEN** any method is called
**THEN** warnings are logged for operations that need settings
**AND** fallback behavior is used (defaults for restore, no-op for save)

#### Scenario: State restoration logging
**GIVEN** state is being restored
**WHEN** restore_state() completes
**THEN** a debug log message is recorded with "Window state restored: {width}x{height}, maximized: {state}"

#### Scenario: State save logging
**GIVEN** state is being saved
**WHEN** save_state() completes
**THEN** a debug log message is recorded with "Window state saved: {width}x{height}, maximized: {state}"

#### Scenario: Invalid size values
**GIVEN** GSettings contains invalid window dimensions (e.g., 0x0)
**WHEN** restore_state() is called
**THEN** the window falls back to default size 1200x800
**AND** a warning is logged

