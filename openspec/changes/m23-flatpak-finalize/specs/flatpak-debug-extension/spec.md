## ADDED Requirements

### Requirement: Manifest declares io.github.tobagin.karere.Debug extension
The Flatpak manifest SHALL declare an `add-extensions` block for `io.github.tobagin.karere.Debug` with `directory: lib/debug`, `autodelete: 'true'`, and `no-autodownload: 'true'`. The Debug extension SHALL carry detached debug symbols for the karere binary and any first-party shared libraries.

#### Scenario: Manifest contains the Debug extension declaration
- **WHEN** the manifest is parsed
- **THEN** `add-extensions` contains an entry keyed `io.github.tobagin.karere.Debug`
- **AND** that entry sets `directory: lib/debug`, `autodelete: 'true'`, and `no-autodownload: 'true'`

#### Scenario: Debug extension installs separately from the base app
- **WHEN** the user runs `flatpak install --user --no-deps repo io.github.tobagin.karere.Debug`
- **THEN** the installation succeeds
- **AND** debug symbols are mounted under `/app/lib/debug` of the running karere sandbox

### Requirement: Symbolicated stack traces via coredumpctl debug karere
With the Debug extension installed, `coredumpctl debug karere` SHALL resolve symbols for crash backtraces without requiring user-side `debuginfod` configuration.

#### Scenario: Crash backtrace resolves function names
- **WHEN** karere crashes and the user runs `coredumpctl debug karere`
- **THEN** the resulting backtrace shows resolved function names from the karere binary
- **AND** no "??" placeholders appear in the karere frames

#### Scenario: Base app runs normally without Debug extension installed
- **WHEN** the base app is installed but the Debug extension is not
- **THEN** karere launches and operates normally
- **AND** the absence of `/app/lib/debug` content does not impair runtime behavior
