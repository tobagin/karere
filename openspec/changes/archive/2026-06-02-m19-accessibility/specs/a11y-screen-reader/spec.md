## ADDED Requirements

### Requirement: Screen-reader option appends Chromium caret-browsing flag
The application SHALL expose a boolean GSettings key `screen-reader-opts` (default `false`). When `true`, the CEF runtime SHALL append `--enable-caret-browsing` to the Chromium command line inside `on_before_command_line_processing`. The setting SHALL be restart-required and the preferences UI SHALL communicate that.

#### Scenario: Enabling screen-reader-opts adds caret-browsing flag on next launch
- **WHEN** the user sets `screen-reader-opts` to `true` and restarts the application
- **THEN** `on_before_command_line_processing` appends `--enable-caret-browsing` to the command line
- **AND** Chromium DevTools "Document settings" shows caret browsing active

#### Scenario: Disabling screen-reader-opts removes the flag on next launch
- **WHEN** `screen-reader-opts` is `false` and the application starts
- **THEN** `--enable-caret-browsing` is not appended to the Chromium command line

#### Scenario: Restart requirement is surfaced to users
- **WHEN** the user toggles `screen-reader-opts` at runtime
- **THEN** the preferences page indicates that a restart is required for the change to take effect
- **AND** no attempt is made to live-reconfigure the running Chromium subprocesses
