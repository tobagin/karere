# karere-actions Specification

## Purpose

Defines the Karere action surface: the application-level and window-level
`GAction` groups, the accelerator table wired during startup, and the
`AdwToastOverlay` placement in the window tree that downstream milestones use to
emit toasts.

## Requirements

### Requirement: Application-level action group is registered
The system SHALL register the following actions on `KarereApplication` during `startup`: `app.quit`, `app.about`, `app.preferences`, `app.show-help-overlay`, `app.sync-autostart`, `app.present-window`, `app.notification-clicked`, `app.switch-account`, `app.set-unread`, `app.refresh-tray-accounts`, `app.open-download`. Action implementations that depend on a feature landing in a later milestone SHALL be wired now as no-op stubs that emit a `g_warning` identifying the owning milestone and return without crashing.

#### Scenario: `app.quit` exits cleanly
- **WHEN** `gio activate io.github.tobagin.karere quit` is invoked or `Ctrl+Q` is pressed
- **THEN** the application initiates the M4 clean-shutdown flow and exits with status 0

#### Scenario: `app.about` opens AdwAboutDialog populated from metainfo
- **WHEN** the user triggers `app.about` from the application menu
- **THEN** an `adw::AboutDialog` is presented and its release-notes section is populated from `/app/share/metainfo/io.github.tobagin.karere.metainfo.xml`

#### Scenario: Stub actions log a warning and return
- **WHEN** the user triggers a stub action such as `app.notification-clicked`, `app.switch-account`, `app.set-unread`, `app.refresh-tray-accounts`, or `app.open-download`
- **THEN** a `g_warning` is logged naming the action and the milestone that will land its body, and no crash or panic occurs

### Requirement: Window-level action group is registered
The system SHALL register the following actions on `KarereWindow`: `win.toggle-fullscreen`, `win.minimize`, `win.refresh`, `win.refresh-hard`, `win.zoom-in`, `win.zoom-out`, `win.zoom-reset`, `win.close`. `win.toggle-fullscreen` SHALL flip the result of `window.is_fullscreen()`. `win.minimize` SHALL call `window.minimize()`. `win.close` SHALL trigger the window's `close_request` so it respects the `close-button-action` GSetting. `win.refresh`, `win.refresh-hard`, `win.zoom-in`, `win.zoom-out`, and `win.zoom-reset` SHALL be guarded stubs that no-op when the M9 RequestHandler or M18 zoom interfaces are not yet available.

#### Scenario: `win.toggle-fullscreen` toggles fullscreen state
- **WHEN** the window is windowed and the user presses F11
- **THEN** the window enters fullscreen
- **WHEN** the window is fullscreen and the user presses F11 again
- **THEN** the window returns to windowed state

#### Scenario: `win.minimize` minimizes the window
- **WHEN** the user presses `Ctrl+M`
- **THEN** the window is minimized

#### Scenario: `win.close` respects `close-button-action`
- **WHEN** `close-button-action` is `"background"` and the user presses `Ctrl+W`
- **THEN** the window hides via `set_visible(false)` and the process continues running

### Requirement: Accelerator table is registered on startup
The system SHALL call `gtk::Application::set_accels_for_action` during `KarereApplication::startup` for every binding in the M8 accel table: `app.quit`→`<Primary>q`; `app.preferences`→`<Primary>comma`; `app.show-help-overlay`→`<Primary>question`; `win.toggle-fullscreen`→`F11`,`<Alt>Return`; `win.minimize`→`<Primary>m`; `win.refresh`→`<Primary>r`,`F5`; `win.refresh-hard`→`<Primary><Shift>r`; `win.zoom-in`→`<Primary>plus`,`<Primary>equal`,`<Primary>KP_Add`; `win.zoom-out`→`<Primary>minus`,`<Primary>KP_Subtract`; `win.zoom-reset`→`<Primary>0`,`<Primary>KP_0`; `win.close`→`<Primary>w`.

#### Scenario: Accels are queryable post-startup
- **WHEN** `gtk::Application::accels_for_action("app.quit")` is queried after startup completes
- **THEN** the returned slice contains `"<Primary>q"`

#### Scenario: `app.about` has no accel
- **WHEN** `gtk::Application::accels_for_action("app.about")` is queried
- **THEN** the returned slice is empty

### Requirement: `AdwToastOverlay` is present in the window tree
The system SHALL wrap the existing `AdwToolbarView` inside `data/ui/window.blp` with an `AdwToastOverlay` and expose it on `KarereWindow` as a `TemplateChild<AdwToastOverlay>` so downstream milestones (M9 toasts, M12 download notifications) can call `add_toast` directly.

#### Scenario: Toast overlay is reachable from window code
- **WHEN** Rust code holds a reference to a `KarereWindow` instance
- **THEN** it can access the `toast_overlay` template child and call `add_toast` without further plumbing
