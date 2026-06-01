## ADDED Requirements

### Requirement: Flatpak manifest builds and installs the app end-to-end

The project SHALL provide a flatpak-builder manifest at `packaging/io.github.tobagin.GtkCefShell.yml` that, when invoked with `flatpak-builder --user --install --force-clean build-dir packaging/io.github.tobagin.GtkCefShell.yml`, produces an installed flatpak named `io.github.tobagin.GtkCefShell` capable of rendering a web page via `flatpak run io.github.tobagin.GtkCefShell --url=<url>`.

#### Scenario: Single-command build and install succeeds
- **WHEN** a contributor runs `flatpak-builder --user --install --force-clean build-dir packaging/io.github.tobagin.GtkCefShell.yml` from the repository root
- **THEN** the build completes without errors and `io.github.tobagin.GtkCefShell` is installed in the user's flatpak repository

#### Scenario: Installed flatpak renders a page
- **WHEN** the user runs `flatpak run io.github.tobagin.GtkCefShell --url=https://example.com`
- **THEN** the application window opens and renders the example.com page via the sandboxed CEF runtime

#### Scenario: Cached rebuild stays under five minutes
- **WHEN** the CEF tarball is already in the flatpak-builder source cache and the contributor reruns the build command
- **THEN** the full build completes in under five minutes on a typical developer workstation

### Requirement: CEF runtime is assembled as a self-contained `/app/lib/cef/` tree

The manifest SHALL include a `cef-binaries` module that extracts the upstream CEF tarball and merges its `Release/*` and `Resources/*` subdirectories together with `include/`, `libcef_dll/`, `cmake/`, and `CMakeLists.txt` into a single flat `/app/lib/cef/` directory so that both `cef-dll-sys`'s build script and the CEF runtime loader find their expected files at that root.

#### Scenario: `locales/` lives at the cef_dir root
- **WHEN** the `cef-binaries` module finishes installing
- **THEN** `/app/lib/cef/locales/` exists directly under the cef_dir root, and `cef-dll-sys`'s `copy_cef_runtime_files(cef_dir, target_dir)` completes without error during the subsequent `gtk-cef-shell` module build

#### Scenario: `*.pak` files sit next to `libcef.so`
- **WHEN** the installed flatpak starts CEF
- **THEN** `libcef.so`, the `*.pak` resources, and the `locales/` directory are all present in the directory pointed to by `LD_LIBRARY_PATH=/app/lib/cef`, and CEF initializes without missing-resource errors

#### Scenario: `archive.json` marker is written
- **WHEN** the `cef-binaries` module finishes installing
- **THEN** `/app/lib/cef/archive.json` exists with the form `{"type":"minimal","name":"<tarball>","sha1":"<sha1>"}` so that `download_cef::check_archive_json` accepts the directory

#### Scenario: Per-architecture tarball is selected
- **WHEN** the manifest is built on x86_64
- **THEN** the `cef_binary_…_linux64_minimal.tar.bz2` source is used with its sha256
- **WHEN** the manifest is built on aarch64
- **THEN** the `cef_binary_…_linuxarm64_minimal.tar.bz2` source is used with its sha1 (since the upstream index only publishes sha1 for arm64)

### Requirement: Cargo build runs fully offline against vendored crates

The manifest SHALL build `gtk-cef-shell` without any network access by sourcing every crate from `packaging/cargo-sources.json` and exporting `CARGO_HOME=/run/build/gtk-cef-shell/cargo` and `CARGO_NET_OFFLINE=true` globally.

#### Scenario: Build module has no `--share=network`
- **WHEN** an auditor inspects the `gtk-cef-shell` module's `build-options` in the manifest
- **THEN** no `--share=network` build-arg is present

#### Scenario: `cargo-sources.json` is exhaustive
- **WHEN** flatpak-builder applies `packaging/cargo-sources.json` to the build directory
- **THEN** `cargo/vendor/<crate>` directories with matching `.cargo-checksum.json` files exist for every dependency in `Cargo.lock`, and `cargo/config` contains `[source.crates-io] replace-with = vendored-sources`

#### Scenario: Meson auto-selects the vendored CARGO_HOME
- **WHEN** the meson build runs inside the flatpak sandbox and `project_source_root()/cargo` exists
- **THEN** the cargo custom_target uses that directory as `CARGO_HOME` so the offline configuration is picked up

### Requirement: Cargo custom_target uses a shell wrapper

The `meson.build` cargo `custom_target` SHALL invoke `sh -c 'env <env vars> cargo build <opts> && cp <out> @OUTPUT@'` rather than a flat argv, because the `&&` between `cargo build` and `cp` requires a shell.

#### Scenario: Custom target argv starts with `sh -c`
- **WHEN** the project is configured with meson
- **THEN** the `cargo-build` custom_target's command begins with `sh`, `-c`, and a single string argument that contains both the `cargo build` invocation and the trailing `cp` to `@OUTPUT@`

### Requirement: Sandbox permissions are the minimum needed by the shell

The manifest's `finish-args` SHALL grant exactly the permissions required for the GTK + CEF shell to run: `--share=ipc`, `--share=network`, `--socket=wayland`, `--socket=fallback-x11`, `--socket=pulseaudio`, `--device=all`, `--filesystem=xdg-download`, `--talk-name=org.freedesktop.Notifications`, `--talk-name=org.freedesktop.portal.Desktop`, and `--env=LD_LIBRARY_PATH=/app/lib/cef`.

#### Scenario: Finish-args match the documented set
- **WHEN** an auditor reads `packaging/io.github.tobagin.GtkCefShell.yml`'s `finish-args`
- **THEN** the list contains exactly the permissions enumerated above and no others

### Requirement: AppStream compose and icon cache are disabled until M23

The manifest SHALL set `appstream-compose: false` and the meson `gnome.post_install` call SHALL omit `gtk_update_icon_cache: true` until the project ships valid icons and metainfo (tracked in M23).

#### Scenario: appstream-compose disabled
- **WHEN** an auditor reads the manifest
- **THEN** `appstream-compose: false` is set at the top level

#### Scenario: No icon cache regeneration
- **WHEN** an auditor reads `meson.build`
- **THEN** the `gnome.post_install` call does not pass `gtk_update_icon_cache: true`

### Requirement: CEF message pump runs on the main thread

The Rust code SHALL drive `cef::do_message_loop_work()` from a steady 8 ms `glib::timeout_add_local` installed once on the main thread, and SHALL NOT schedule pump work from `on_schedule_message_pump_work` (which CEF can call from a non-main thread, causing `glib::timeout_add_local_once` to panic).

#### Scenario: No glib scheduling from CEF callback thread
- **WHEN** CEF invokes `on_schedule_message_pump_work` from a non-main thread
- **THEN** the callback returns without calling any `glib::timeout_add_*` function, and no panic occurs

#### Scenario: Steady pump processes CEF work
- **WHEN** the application is running
- **THEN** a single main-thread timeout fires every 8 ms and calls `cef::do_message_loop_work()`, keeping CEF responsive without orphaning zygote processes on shutdown (M04 behavior preserved inside the flatpak)
