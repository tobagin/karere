# app-bootstrap Specification

## Purpose

Defines how the gtk-cef-shell binary boots the GTK + libadwaita application
process, parses command-line input, and performs an orderly shutdown around
CEF.

## Requirements

### Requirement: GTK + libadwaita application boot
The binary SHALL initialise logging, the GL loader, CEF api hash, the
gresource bundle, and an Adw `Application` with `HANDLES_COMMAND_LINE`
before invoking `adw_app.run()`.

#### Scenario: Boot sequence succeeds on the developer host
- **WHEN** the binary starts in browser-process mode
- **THEN** env_logger is initialised, `cef::api_hash(CEF_API_VERSION_LAST, 0)` is called, libepoxy is loaded into both `epoxy` and `gl`, the compiled gresource is registered, and an Adw `Application` with the id `io.github.tobagin.GtkCefShell` and the `HANDLES_COMMAND_LINE` flag is constructed

### Requirement: URL command-line flag
The binary SHALL accept `--url=<URL>` (and `--url <URL>`) from the process
arguments and pass the value to the activation closure; if absent it SHALL
fall back to `https://example.com`.

#### Scenario: URL is captured from the command line
- **WHEN** the binary is launched with `--url=https://example.org`
- **THEN** `application::activate` is invoked with `"https://example.org"` as the URL argument

#### Scenario: Default URL is used when the flag is absent
- **WHEN** the binary is launched without a `--url` argument
- **THEN** `application::activate` is invoked with `"https://example.com"` as the URL argument

### Requirement: Clean shutdown around CEF
The binary SHALL call `cef::shutdown()` after `adw_app.run()` returns and
SHALL propagate the Adw exit code as the process exit code.

#### Scenario: Shutdown sequence on normal exit
- **WHEN** `adw_app.run()` returns
- **THEN** `cef::shutdown()` is called and the process exits with the Adw exit code converted to `i32`
