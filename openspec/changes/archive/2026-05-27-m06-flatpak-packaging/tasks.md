## 1. CEF runtime module

- [x] 1.1 Identify the correct CEF build (`148.0.8+g18e00ea+chromium-148.0.7778.96`) and update `download-cef.sh` to fetch it
- [x] 1.2 Fix the `download-cef.sh` symlink target so URL-encoded vs decoded directory names align
- [x] 1.3 Capture sha256 for the x86_64 minimal tarball and sha1 for the aarch64 minimal tarball
- [x] 1.4 Author the `cef-binaries` module (buildsystem: simple) that merges `Release/*` and `Resources/*` into a flat `/app/lib/cef/`
- [x] 1.5 Write the `archive.json` marker (`{"type":"minimal","name":"…","sha1":"…"}`) at `/app/lib/cef/archive.json`
- [x] 1.6 Verify `cef-dll-sys`'s build script (`copy_cef_runtime_files`) succeeds against the merged tree

## 2. Cargo vendoring

- [x] 2.1 Run `flatpak-cargo-generator.py Cargo.lock -o packaging/cargo-sources.json`
- [x] 2.2 Confirm the final entry inlines `cargo/config` with `[source.crates-io] replace-with = vendored-sources`
- [x] 2.3 Update `meson.build` to auto-select `project_source_root()/cargo` as `CARGO_HOME` when present
- [x] 2.4 Set global manifest env `CARGO_HOME=/run/build/gtk-cef-shell/cargo` and `CARGO_NET_OFFLINE=true`

## 3. Meson integration

- [x] 3.1 Rewrite the `cargo-build` `custom_target` to use `sh -c 'env … cargo build … && cp … @OUTPUT@'`
- [x] 3.2 Remove `gtk_update_icon_cache: true` from `gnome.post_install`
- [x] 3.3 Confirm `-Dprofile=default` is passed by the `gtk-cef-shell` module

## 4. Flatpak manifest body

- [x] 4.1 Target runtime `org.gnome.Platform//50` with sdk `org.gnome.Sdk//50`
- [x] 4.2 Add sdk-extension `org.freedesktop.Sdk.Extension.rust-stable` and wire `PATH`/`append-path`
- [x] 4.3 Set global env `CEF_PATH=/app/lib/cef`
- [x] 4.4 Set `appstream-compose: false`
- [x] 4.5 Configure finish-args: `--share=ipc`, `--share=network`, `--socket=wayland`, `--socket=fallback-x11`, `--socket=pulseaudio`, `--device=all`, `--filesystem=xdg-download`, `--talk-name=org.freedesktop.Notifications`, `--talk-name=org.freedesktop.portal.Desktop`, `--env=LD_LIBRARY_PATH=/app/lib/cef`
- [x] 4.6 Add the `gtk-cef-shell` module with the two sources (`type: dir, path: ..` and `cargo-sources.json`); no `--share=network`

## 5. Message pump fix (surfaced by flatpak QA)

- [x] 5.1 Reproduce the panic from `glib::timeout_add_local_once` invoked off-main-thread by `on_schedule_message_pump_work`
- [x] 5.2 Drop the callback body so the scheduling hook becomes a no-op
- [x] 5.3 Install a single 8 ms `glib::timeout_add_local` on the main thread that calls `cef::do_message_loop_work()`
- [x] 5.4 Confirm M04 zygote shutdown still passes inside the flatpak

## 6. Verification

- [x] 6.1 Run `flatpak-builder --user --install --force-clean build-dir packaging/io.github.tobagin.GtkCefShell.yml` to completion
- [x] 6.2 Confirm a cached rebuild finishes in under five minutes
- [x] 6.3 Run `flatpak run io.github.tobagin.GtkCefShell --url=https://example.com` and verify the page renders
- [x] 6.4 Verify no orphan zygote processes remain after closing the flatpak window
