## ADDED Requirements

### Requirement: Single-binary subprocess routing
The application SHALL re-execute itself as CEF subprocess types (renderer,
GPU, utility, zygote) using the same binary, and SHALL return immediately
from the subprocess invocation after `cef::execute_process` drives it to
completion.

#### Scenario: Renderer subprocess is detected and exits via execute_process
- **WHEN** the process is launched with a `--type=...` switch on the command line
- **THEN** `cef::execute_process` is called with the App, runs the subprocess loop, returns a non-negative exit code, and the binary exits without entering the Adw `Application`

#### Scenario: Browser process continues to Adw setup
- **WHEN** the process is launched without a `--type` switch
- **THEN** `cef::execute_process` returns `-1` and the binary proceeds to call `initialize_browser_process` and then `adw_app.run()`

### Requirement: Browser process initialization
The browser process SHALL configure CEF with windowless rendering enabled,
an external message pump, no Chromium suid sandbox, and warning-level
logging before any browser is created.

#### Scenario: CEF initialize succeeds with required settings
- **WHEN** the browser process calls `initialize_browser_process`
- **THEN** `cef::initialize` is invoked with `windowless_rendering_enabled=1`, `external_message_pump=1`, `no_sandbox=1`, `log_severity=WARNING`, and returns success
- **AND** the log emits "CEF initialized"

#### Scenario: BrowserProcessHandler reports context ready
- **WHEN** CEF finishes internal context initialization
- **THEN** the `ShellBrowserProcessHandler::on_context_initialized` callback fires and logs "CEF context initialized"

### Requirement: External message pump driven from the glib main loop
The browser process SHALL drive `cef::do_message_loop_work()` from the
glib main loop at an 8 ms cadence so CEF never stalls regardless of when
or from which thread `on_schedule_message_pump_work` is invoked.

#### Scenario: Pump tick is installed after initialize
- **WHEN** `initialize_browser_process` returns successfully
- **THEN** a `glib::timeout_add_local(8ms, ...)` is registered that calls `cef::do_message_loop_work()` and returns `ControlFlow::Continue`

#### Scenario: Pump survives schedule callbacks from non-main threads
- **WHEN** CEF would invoke `on_schedule_message_pump_work` from a non-main thread
- **THEN** the embedder does not rely on that callback to advance work; the steady glib timer keeps the pump alive

### Requirement: Wayland-first Chromium command line
The App SHALL append Wayland-friendly Chromium switches in
`on_before_command_line_processing` so subprocesses inherit them.

#### Scenario: Standard switches are appended
- **WHEN** `on_before_command_line_processing` runs with a non-null CommandLine
- **THEN** the following switches are appended: `enable-features=UseOzonePlatform`, `ozone-platform-hint=auto`, `enable-webrtc-vea-vda`, `no-startup-window`, `noerrdialogs`, `hide-crash-restore-bubble`

#### Scenario: Flatpak environment disables Chromium sandbox
- **WHEN** the environment variable `FLATPAK_ID` is set
- **THEN** `no-sandbox` is also appended to the command line
