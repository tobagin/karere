## ADDED Requirements

### Requirement: Register a StatusNotifierItem tray icon via `ksni`
The shell SHALL register a `StatusNotifierItem` D-Bus object using the `ksni = "0.3"` crate, hosted on the shared tokio runtime, with `id()` equal to the application id.

#### Scenario: Tray starts on KDE Plasma
- **WHEN** the shell starts on a session where `XDG_CURRENT_DESKTOP` contains `KDE`
- **THEN** the shell spawns `ksni::Service::run(KarereTray { ... })` on the tokio runtime
- **AND** the SNI item appears in the system tray within 2 seconds of startup

#### Scenario: Tray `id()` equals the app id
- **WHEN** the SNI item is registered
- **THEN** its `id()` returns the application id (e.g., `io.github.tobagin.karere`)

### Requirement: Tray icon reflects unread state
The shell SHALL set the SNI `icon_name` to `<app-id>-tray-symbolic` when `unread_count == 0`, and to `<app-id>-tray-unread-symbolic` when `unread_count > 0`, where `<app-id>` is `FLATPAK_ID` when set (so the installed Devel/release id matches) and `io.github.tobagin.karere` otherwise. The icons are installed app-id-prefixed because Flatpak only exports `$FLATPAK_ID*` icons to the host icon theme, where the SNI host resolves the name; a bare `karere-tray-symbolic` would be dropped on export.

#### Scenario: No unread messages
- **WHEN** `unread_count` is `0`
- **THEN** `icon_name()` returns `<app-id>-tray-symbolic`

#### Scenario: Unread messages present
- **WHEN** `unread_count` is `3`
- **THEN** `icon_name()` returns `<app-id>-tray-unread-symbolic`

### Requirement: Tray tooltip reflects unread count
The shell SHALL set the SNI `tool_tip` text to `<n> unread` (where `<n>` is the integer count) when `unread_count > 0`, and to `Karere` when `unread_count == 0`.

#### Scenario: Zero unread
- **WHEN** `unread_count` is `0`
- **THEN** `tool_tip()` produces a tooltip with title `Karere`

#### Scenario: Non-zero unread
- **WHEN** `unread_count` is `5`
- **THEN** `tool_tip()` produces a tooltip with body `5 unread`

### Requirement: Clicking the tray activates the present-window action
The shell SHALL respond to both the SNI `activate(x, y)` callback (typically a left-click) and `secondary_activate(x, y)` (typically a middle-click) by invoking the `app.present-window` GAction. (SNI has no distinct double-click event; the host maps a click gesture to `activate`.)

#### Scenario: User left-clicks the tray
- **WHEN** the user left-clicks the tray icon
- **THEN** the shell activates `app.present-window`

#### Scenario: User middle-clicks the tray
- **WHEN** the user middle-clicks the tray icon
- **THEN** the shell activates `app.present-window`

### Requirement: `app.set-unread` updates tray state
The shell SHALL provide an `app.set-unread` GAction taking a single `u32` parameter that writes the value into the shared `TrayState.unread_count` and triggers a tray refresh via `ksni::Handle::update`.

#### Scenario: Activation increments unread
- **WHEN** `app.set-unread` is activated with the parameter `3`
- **THEN** `TrayState.unread_count` becomes `3`
- **AND** `ksni::Handle::update` is called so the icon and tooltip refresh

#### Scenario: Activation clears unread
- **WHEN** `app.set-unread` is activated with the parameter `0`
- **THEN** `TrayState.unread_count` becomes `0`
- **AND** the tray icon reverts to `karere-tray-symbolic`

### Requirement: `app.present-window` toggles window visibility
The shell SHALL provide an `app.present-window` GAction that hides the primary chrome window when it is currently visible, and otherwise shows and presents it. The toggle gates on visibility alone (not `is_active`): clicking a tray menu item removes focus from the window, so an `is_active` check would never hide it.

#### Scenario: Window is visible
- **WHEN** the window's `is_visible()` is `true`
- **AND** `app.present-window` is activated
- **THEN** the window is hidden via `window.set_visible(false)`

#### Scenario: Window is hidden
- **WHEN** the window's `is_visible()` is `false`
- **AND** `app.present-window` is activated
- **THEN** the window is shown and presented via `window.present()`

### Requirement: Unread feed from notifications
The shell SHALL increment `TrayState.unread_count` by one whenever the renderer sends a `NotificationSeen` IPC event (M14), by activating `app.set-unread current+1`.

#### Scenario: Notification arrives while window unfocused
- **WHEN** the renderer sends a `NotificationSeen` IPC and `TrayState.unread_count` is `2`
- **THEN** `app.set-unread` is activated with `3`

### Requirement: Unread reset on window focus
The shell SHALL reset `TrayState.unread_count` to `0` when the primary chrome window's `is-active` property transitions to `true`.

#### Scenario: User focuses the window
- **WHEN** the window gains focus and its `is-active` property becomes `true`
- **THEN** `app.set-unread` is activated with `0`

### Requirement: Skip tray on GNOME without AppIndicator
The shell SHALL inspect `XDG_CURRENT_DESKTOP` at startup and, when it equals `GNOME` and no D-Bus owner of `org.kde.StatusNotifierWatcher` is found, log `tray skipped (GNOME w/o AppIndicator)` at INFO and SHALL NOT start `ksni::Service::run`.

#### Scenario: Stock GNOME session
- **WHEN** `XDG_CURRENT_DESKTOP` is `GNOME` and no `org.kde.StatusNotifierWatcher` owner is present on the session bus
- **THEN** the shell logs `tray skipped (GNOME w/o AppIndicator)`
- **AND** does not start the tray service
- **AND** does not emit a WARN-level D-Bus error

#### Scenario: GNOME with AppIndicator extension
- **WHEN** `XDG_CURRENT_DESKTOP` is `GNOME` and `org.kde.StatusNotifierWatcher` has a D-Bus owner
- **THEN** the shell starts the tray service normally

### Requirement: `KARERE_FORCE_TRAY` environment override
The shell SHALL start `ksni::Service::run` regardless of desktop-environment detection when the environment variable `KARERE_FORCE_TRAY` is set to `1`.

#### Scenario: Override forces tray on GNOME
- **WHEN** `XDG_CURRENT_DESKTOP` is `GNOME`, no AppIndicator watcher is detected, and `KARERE_FORCE_TRAY=1` is set in the environment
- **THEN** the shell starts the tray service and skips the auto-detect bypass

### Requirement: Cross-thread tray state via shared mutex
The shell SHALL hold tray state in an `Arc<Mutex<TrayState>>` shared between the tokio tray task and the main-thread GAction handlers, with mutations followed by `ksni::Handle::update`.

#### Scenario: GAction writes state, tray task reads it
- **WHEN** a GAction handler writes a new `unread_count` and then calls `handle.update()`
- **THEN** the next `icon_name()`, `tool_tip()`, or `menu()` invocation by `ksni` observes the new value

### Requirement: Flatpak finish-args allow StatusNotifierWatcher
The shell's Flatpak manifest SHALL include `--talk-name=org.kde.StatusNotifierWatcher` in `finish-args` so the sandboxed app can register its tray item.

#### Scenario: Built Flatpak registers SNI
- **WHEN** the Flatpak build of the shell starts on KDE Plasma
- **THEN** the tray item is registered successfully (no D-Bus permission denied error)
