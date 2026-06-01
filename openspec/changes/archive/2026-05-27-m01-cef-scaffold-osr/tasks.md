## 1. Cargo and meson skeleton

- [x] 1.1 Add Cargo dependencies (`gtk4` 0.11 v4_12, `libadwaita` 0.9 v1_6, `glib`/`gio` 0.22, `cef` 148 with `build-util`, `epoxy`, `gl`, `libloading`, `anyhow`, `log`, `env_logger`, `parking_lot`, `once_cell`) and `glib-build-tools` as build-dependency in `Cargo.toml`
- [x] 1.2 Configure release profile (`lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`)
- [x] 1.3 Add `build.rs`, `meson.build`, and `meson_options.txt` with a custom target that delegates the binary build to cargo
- [x] 1.4 Add `data/` directory with placeholder desktop entry, AppStream metainfo, gschema, gresource manifest, and composite UI template stubs

## 2. CEF runtime module (`src/cef_runtime.rs`)

- [x] 2.1 Define `ShellApp` and the `wrap_app!` `ShellAppBuilder`
- [x] 2.2 Implement `on_before_command_line_processing` to append `enable-features=UseOzonePlatform`, `ozone-platform-hint=auto`, `enable-webrtc-vea-vda`, `no-startup-window`, `noerrdialogs`, `hide-crash-restore-bubble`
- [x] 2.3 Append `--no-sandbox` when `FLATPAK_ID` is set
- [x] 2.4 Implement `browser_process_handler()` returning a `ShellBrowserProcessHandlerBuilder`
- [x] 2.5 Define `ShellBrowserProcessHandler` with an `Arc<Mutex<PumpState>>` (scheduled + ready flags) and a `wrap_browser_process_handler!` builder
- [x] 2.6 Implement `on_context_initialized` to set the ready flag and log "CEF context initialized"
- [x] 2.7 Leave `on_schedule_message_pump_work` as a no-op (documented in source)
- [x] 2.8 Implement `initialize_browser_process(args, app)` setting `windowless_rendering_enabled=1`, `external_message_pump=1`, `no_sandbox=1`, `log_severity=WARNING`
- [x] 2.9 Install an 8 ms `glib::timeout_add_local` that calls `cef::do_message_loop_work()` and returns `ControlFlow::Continue`

## 3. Application entry point (`src/main.rs`)

- [x] 3.1 Initialise `env_logger` with `info,gtk_cef_shell=debug` default filter
- [x] 3.2 Call `cef::api_hash(CEF_API_VERSION_LAST, 0)` and load libepoxy into `epoxy` and `gl`
- [x] 3.3 Build `Args::new()`, detect the `--type` switch via `as_cmd_line().has_switch("type")`
- [x] 3.4 Call `cef::execute_process` once with the App; if subprocess, assert non-negative exit code and return; if browser, assert it returned `-1`
- [x] 3.5 Call `cef_runtime::initialize_browser_process`
- [x] 3.6 Parse `--url=` / `--url` from process arguments, falling back to `https://example.com`
- [x] 3.7 Register the compiled gresource and construct an Adw `Application` with id `io.github.tobagin.GtkCefShell` and `HANDLES_COMMAND_LINE`
- [x] 3.8 Wire `connect_command_line` to call `application::activate(app, &url)` and return `ExitCode::SUCCESS`
- [x] 3.9 After `adw_app.run()`, call `cef::shutdown()` and propagate the exit code

## 4. UI skeleton

- [x] 4.1 Add `src/application.rs` exposing `activate(app, url)` that constructs and presents `ShellWindow`
- [x] 4.2 Add `src/window.rs` with a `ShellWindow` placeholder (no render path yet)

## 5. CEF binary provisioning

- [x] 5.1 Add `download-cef.sh` that downloads `cef_binary_148.0.8+g18e00ea+chromium-148.0.7778.96_linux64_minimal` into `cef-binaries/`
- [x] 5.2 Maintain a `cef-binaries/current` symlink pointing at the latest extracted directory

## 6. Verification

- [x] 6.1 `CEF_PATH=$(pwd)/cef-binaries/current/Release cargo build` completes without errors
- [x] 6.2 `CEF_PATH=...Release ./target/debug/gtk-cef-shell --url=https://example.com` logs "CEF context initialized" and "CEF initialized"
- [x] 6.3 Process exits cleanly on Ctrl-C after `cef::shutdown`
