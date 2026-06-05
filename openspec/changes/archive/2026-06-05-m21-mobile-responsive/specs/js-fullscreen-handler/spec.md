## ADDED Requirements

### Requirement: DisplayHandler observes JS-initiated fullscreen
The application SHALL implement `DisplayHandler::on_fullscreen_mode_change` so that JS-initiated fullscreen requests are observable by the browser process.

#### Scenario: Fullscreen entry observed
- **WHEN** a page calls `element.requestFullscreen()` and CEF invokes `on_fullscreen_mode_change(browser, fullscreen)` with `fullscreen != 0`
- **THEN** the handler pushes `FullscreenRequest { on: true }` into SharedState

#### Scenario: Fullscreen exit observed
- **WHEN** CEF invokes `on_fullscreen_mode_change(browser, fullscreen)` with `fullscreen == 0`
- **THEN** the handler pushes `FullscreenRequest { on: false }` into SharedState

#### Scenario: Off-main-thread invocation
- **WHEN** `on_fullscreen_mode_change` is invoked from the CEF UI thread (not the GTK main thread)
- **THEN** the handler MUST NOT touch GTK widgets directly
- **AND** all GTK state changes are deferred to the SharedState-draining polling loop

### Requirement: Window fullscreen state mirrors requests
The window polling loop SHALL drain `FullscreenRequest` events and apply them to the GTK window.

#### Scenario: Enter fullscreen
- **WHEN** the polling loop drains `FullscreenRequest { on: true }`
- **THEN** the loop calls `window.fullscreen()` on the GTK `ApplicationWindow`

#### Scenario: Exit fullscreen
- **WHEN** the polling loop drains `FullscreenRequest { on: false }`
- **THEN** the loop calls `window.unfullscreen()` on the GTK `ApplicationWindow`

### Requirement: Headerbar visibility tracks fullscreen state
The application SHALL hide the Adwaita headerbar while the window is fullscreen and restore it when the window leaves fullscreen, regardless of how fullscreen was entered or exited.

#### Scenario: Headerbar hidden on entry
- **WHEN** the GTK `ApplicationWindow` enters the fullscreen state (`window.is_fullscreen()` becomes true)
- **THEN** the `AdwHeaderBar` widget receives `set_visible(false)`

#### Scenario: Headerbar restored on JS-initiated exit
- **WHEN** the GTK `ApplicationWindow` leaves the fullscreen state due to `on_fullscreen_mode_change(0)` having been processed
- **THEN** the `AdwHeaderBar` widget receives `set_visible(true)`

#### Scenario: Headerbar restored on user-initiated exit
- **WHEN** the user exits fullscreen via Esc, F11, or the window manager
- **THEN** the `notify::fullscreened` signal fires on the window
- **AND** the signal handler restores `AdwHeaderBar::set_visible(true)` even though no IPC was received

#### Scenario: Single source of truth
- **WHEN** both the polling-loop drain and the `notify::fullscreened` signal converge on the same boolean state
- **THEN** repeated calls to `set_visible(true)` or `set_visible(false)` MUST be idempotent and MUST NOT flicker the headerbar
