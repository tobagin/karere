## Why

Contributors and downstream users need a reproducible single-shot build of GTK CEF Shell so everyone runs the same binary against the same CEF runtime. The CEF tarball is 294 MB and not hosted on Flathub, and Cargo cannot reach crates.io from inside the flatpak-builder sandbox. We therefore must (a) ship CEF as our own module source and (b) vendor every crate so the build is fully offline.

## What Changes

- Add `packaging/io.github.tobagin.GtkCefShell.yml` — a flatpak-builder manifest targeting `org.gnome.Platform//50` with the `rust-stable` SDK extension and a self-contained `/app/lib/cef/` runtime.
- Add a `cef-binaries` module that downloads the CEF tarball per-arch (x86_64 + aarch64), merges `Release/*` and `Resources/*` into a flat `/app/lib/cef/` root, and writes the `archive.json` marker so `download_cef::check_archive_json` is satisfied.
- Add `packaging/cargo-sources.json` (vendored crates, 463 entries) generated from `Cargo.lock` so the meson build can run with `CARGO_NET_OFFLINE=true`.
- Update `meson.build`: drive the cargo step through `sh -c 'env … cargo build … && cp …'`, auto-select `project_source_root()/cargo` as `CARGO_HOME` when present, and drop `gtk_update_icon_cache: true` from `gnome.post_install` (no icons ship yet).
- Set `appstream-compose: false` on the manifest until icons + metainfo are valid (deferred to M23).
- Update `download-cef.sh` to pull `148.0.8+g18e00ea+chromium-148.0.7778.96` and fix the symlink target so URL-encoded vs decoded directory names line up.
- Replace the `on_schedule_message_pump_work` callback (which panicked when CEF called it off the main thread via `glib::timeout_add_local_once`) with a steady 8 ms main-thread `timeout_add_local` pump driving `cef::do_message_loop_work()`.

## Capabilities

### New Capabilities
- `flatpak-packaging`: reproducible flatpak-builder pipeline that vendors crates, manages the CEF runtime as a local module, and produces an installable `io.github.tobagin.GtkCefShell` flatpak with the sandbox permissions the shell needs.

### Modified Capabilities
<!-- None — earlier milestones (M01–M04) have not yet been archived into openspec/specs/, so there is no existing spec to delta. The message-pump fix is captured under this milestone because it surfaced during flatpak QA. -->

## Impact

- New files: `packaging/io.github.tobagin.GtkCefShell.yml`, `packaging/cargo-sources.json`, `download-cef.sh` (updated).
- Modified files: `meson.build` (cargo custom_target, post_install).
- Modified Rust source: `on_schedule_message_pump_work` site replaced by steady main-thread pump.
- New external sources: CEF tarball mirror (`cef-builds.spotifycdn.com`) per-arch.
- New runtime dependencies: `org.gnome.Platform//50`, `org.gnome.Sdk//50`, `org.freedesktop.Sdk.Extension.rust-stable`.
- Sandbox surface: wayland + fallback-x11, pulseaudio, ipc, network, all devices, `xdg-download` filesystem, notification + portal talk-names, `LD_LIBRARY_PATH=/app/lib/cef`.
- Out of scope: `appstream-compose: true`, debug-extension shipping, icon cache regeneration (all deferred to M23).
