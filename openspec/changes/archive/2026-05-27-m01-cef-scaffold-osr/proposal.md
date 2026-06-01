## Why

gtk-cef-shell needs a working CEF process model wired into a GTK4 + libadwaita
application before any rendering, input, or UI work can begin. Without the
subprocess re-exec, App + BrowserProcessHandler, and an external message pump
driven from the glib main loop, no later milestone (M2 render path, M3 input,
M4 lifecycle, M5 permissions, M6 Flatpak) has anywhere to attach. This change
records the M1 scaffolding that has already landed so the rest of the roadmap
has a documented baseline.

## What Changes

- Add Cargo dependencies for gtk4 0.11, libadwaita 0.9, cef 148, epoxy, gl,
  libloading, anyhow, log, env_logger, parking_lot, once_cell, and glib /
  gio 0.22.
- Introduce a meson skeleton (`meson.build`, `meson_options.txt`, `build.rs`)
  that delegates the binary build to cargo via a custom target, plus a `data/`
  layout for the desktop file, AppStream metainfo, gschema, gresource, and the
  composite UI template (initial empty stubs).
- Implement `src/main.rs` boot sequence: env_logger init, `cef::api_hash`,
  libepoxy GL loader, `Args::new`, `--type` switch detection, single
  `cef::execute_process` that returns immediately for subprocesses and -1 for
  the browser process, `cef_runtime::initialize_browser_process`, Adw
  `Application` with `HANDLES_COMMAND_LINE`, command-line activation that
  spawns the window, and `cef::shutdown` after `adw_app.run()`.
- Implement `src/cef_runtime.rs`: `ShellApp` (`wrap_app!`) that appends
  `enable-features=UseOzonePlatform`, `ozone-platform-hint=auto`,
  `enable-webrtc-vea-vda`, `no-startup-window`, `noerrdialogs`,
  `hide-crash-restore-bubble`, plus `no-sandbox` under `FLATPAK_ID`;
  `ShellBrowserProcessHandler` (`wrap_browser_process_handler!`) whose
  `on_context_initialized` only flips a ready flag; and
  `initialize_browser_process` that sets `windowless_rendering_enabled=1`,
  `external_message_pump=1`, `no_sandbox=1`, `log_severity=WARNING`, then
  installs an 8 ms `glib::timeout_add_local` driving
  `cef::do_message_loop_work()` unconditionally (CEF schedules from non-main
  threads, so we cannot rely on `on_schedule_message_pump_work`).
- Add `src/application.rs` and `src/window.rs` skeletons so the Adw activation
  has somewhere to land (no render path yet).
- Add `download-cef.sh` that fetches the CEF 148.0.8+g18e00ea+chromium-148
  minimal tarball into `cef-binaries/` and symlinks `cef-binaries/current`.

## Capabilities

### New Capabilities
- `cef-process-model`: subprocess re-exec, browser-process init, and the
  external glib message pump that keeps CEF alive on the GTK main loop.
- `app-bootstrap`: GTK4 + libadwaita application entry point, command-line
  parsing (`--url`), gresource registration, and clean shutdown sequencing
  around CEF init/shutdown.
- `cef-binary-provisioning`: developer script and directory layout for
  fetching the upstream CEF binary distribution used at build and run time.

### Modified Capabilities
<!-- none; this is the first milestone -->

## Impact

- New files: `Cargo.toml`, `Cargo.lock`, `build.rs`, `meson.build`,
  `meson_options.txt`, `download-cef.sh`, `src/main.rs`, `src/cef_runtime.rs`,
  `src/application.rs`, `src/window.rs`, `data/` stub layout.
- New external runtime dependency: CEF 148 binary distribution under
  `cef-binaries/current/Release` (consumed via `CEF_PATH`).
- Out of scope for this change: input forwarding (M3), clean shutdown beyond
  `cef::shutdown` (M4), permission dialog (M5), Flatpak manifest (M6), and the
  render path itself (M2).
