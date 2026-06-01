## ADDED Requirements

### Requirement: `run-on-startup` toggle uses the XDG Background portal
The system SHALL expose an `app.sync-autostart` action that, when activated, calls `ashpd::desktop::background::Background::request_background()` to request (or release) autostart based on the current value of the `run-on-startup` GSetting. The portal call SHALL run on a static `tokio::runtime::Runtime` that is constructed lazily on first use and leaked via `Box::leak` so it outlives the application scope. The portal call SHALL NOT block the GTK main loop.

#### Scenario: Enabling run-on-startup invokes the portal
- **WHEN** `run-on-startup` is set to `true` and `app.sync-autostart` is activated
- **THEN** `Background::request_background()` is invoked asynchronously and the GTK main loop remains responsive while the portal dialog is shown

#### Scenario: Disabling run-on-startup invokes the portal
- **WHEN** `run-on-startup` is set to `false` and `app.sync-autostart` is activated
- **THEN** the portal is invoked to release the autostart entry

#### Scenario: Tokio runtime is created exactly once
- **WHEN** `app.sync-autostart` is activated multiple times across the lifetime of the process
- **THEN** a single `tokio::runtime::Runtime` instance services all invocations and is not re-created per call

### Requirement: Close-to-background runtime branch
The system SHALL, when `close-button-action` resolves to `"background"` at close-request time, hide the window via `set_visible(false)` and return `glib::Propagation::Stop` so the application process keeps running and can be re-presented later (e.g., via `app.present-window` from M15's tray or from `gio activate`).

#### Scenario: Hidden window re-presents via `app.present-window`
- **WHEN** the window has been hidden via the background branch and `app.present-window` is activated
- **THEN** the same window instance becomes visible again with its previous geometry intact and no CEF re-initialisation occurs

#### Scenario: Process survives close-to-background
- **WHEN** the window is hidden via the background branch
- **THEN** the application process remains alive, the CEF subprocess remains alive, and no shutdown actions are run

### Requirement: Cargo dependency surface for background mode
The system SHALL declare the following crate dependencies in `Cargo.toml` for the background-mode capability: `ashpd` with the `gtk4` and `background` features (version line at or compatible with `0.13`), and `tokio` with the `rt-multi-thread`, `macros`, and `time` features (version line at or compatible with `1`).

#### Scenario: Crates available at link time
- **WHEN** `cargo build` is run after M8 lands
- **THEN** the build resolves `ashpd` with `gtk4`+`background` features and `tokio` with `rt-multi-thread`+`macros`+`time` features without missing-feature errors
