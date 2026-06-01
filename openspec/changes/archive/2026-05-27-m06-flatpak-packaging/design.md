## Context

GTK CEF Shell embeds the Chromium Embedded Framework as an off-screen renderer driven from a GTK 4 app. The local dev loop runs `download-cef.sh` and `cargo build`, but downstream users need a Flatpak that bundles the right CEF, the right GNOME runtime, and a reproducible Rust build.

Two constraints shape the design:

1. **CEF is too big for Flathub.** The minimal Linux tarball is ~294 MB. We host it as a per-arch source on the upstream Spotify CDN and assemble `/app/lib/cef/` ourselves.
2. **The flatpak-builder sandbox has no network during the main build step.** Cargo must read vendored crates, so we generate `packaging/cargo-sources.json` from `Cargo.lock` and force `CARGO_NET_OFFLINE=true`.

Downstream users want one command (`flatpak-builder --user --install --force-clean build-dir packaging/io.github.tobagin.GtkCefShell.yml`) and a working `flatpak run io.github.tobagin.GtkCefShell --url=…`.

## Goals / Non-Goals

**Goals:**
- A single manifest that builds and installs end-to-end in under 5 minutes once the CEF tarball is cached.
- Deterministic builds (vendored crates, pinned CEF SHA, pinned GNOME runtime).
- A `/app/lib/cef/` layout that satisfies both `cef-dll-sys`'s build.rs (which cmake-builds `libcef_dll_wrapper` against the merged tree) and the CEF runtime loader (which expects `locales/` and `*.pak` next to `libcef.so`).
- Sandbox permissions sufficient for Wayland, X11 fallback, audio, notifications, and the portal — nothing more.

**Non-Goals:**
- `appstream-compose: true` and icon cache regeneration (deferred to M23 once icons + metainfo land).
- Shipping a `.Debug` extension with CEF symbols (deferred to M23).
- Multi-arch CI publishing — local single-arch build is sufficient for this milestone.
- Hosting CEF on Flathub.

## Decisions

### 1. Flat `/app/lib/cef/` tree, not Spotify's split layout

The upstream tarball ships `Release/`, `Resources/`, `include/`, `libcef_dll/`, `cmake/`, and `CMakeLists.txt`. `cef-dll-sys`'s build.rs calls `copy_cef_runtime_files(cef_dir, target_dir)` and expects `<cef_dir>/locales/` to exist directly; at runtime the loader looks for `*.pak` and `locales/` next to `libcef.so`. The `cef-binaries` module merges `Release/*` and `Resources/*` into `/app/lib/cef/` while keeping `include/`, `libcef_dll/`, `cmake/`, and `CMakeLists.txt` at the same root.

Alternatives considered:
- Keep the upstream layout and patch `cef-dll-sys` — rejected, downstream patches on a vendored build script.
- Symlink `Release` and `Resources` into the root — rejected, the cmake build still walks the original tree and double-links libcef.so.

### 2. `archive.json` marker file

`download_cef::check_archive_json` (used by the local dev loop) reads `archive.json` to confirm the CEF directory matches the expected tarball. The flatpak module writes the same JSON (`{"type":"minimal","name":"…","sha1":"…"}`) so the same code path that gates the dev loop also gates the flatpak runtime build.

### 3. Per-arch CEF source

Spotify's CDN publishes `cef_binary_<ver>_linux64_minimal.tar.bz2` and `…_linuxarm64_minimal.tar.bz2`. Each arch source provides the right filename string and integrity hash. x86_64 ships with sha256; aarch64's index only publishes sha1, so the aarch64 source uses sha1 as flatpak-builder allows either.

### 4. Vendored crates via `flatpak-cargo-generator.py`

`packaging/cargo-sources.json` (128 KB, 463 entries) is generated from `Cargo.lock`. Each entry extracts an archive into `cargo/vendor/<crate>` with an inline `.cargo-checksum.json`. The final entry inlines `cargo/config` with `[source.crates-io] replace-with = vendored-sources`. The meson build auto-picks `project_source_root()/cargo` as `CARGO_HOME` when that directory exists, so the same `meson.build` works both for local dev (no vendor dir, online cargo) and for the flatpak (vendor dir present, offline cargo).

### 5. `sh -c` wrapper around the cargo custom_target

Meson's `custom_target` takes a flat argv. Writing `env CARGO_HOME=… CARGO_NET_OFFLINE=true cargo build … && cp …` requires a shell to interpret `&&`. The target is therefore `['sh', '-c', 'env … cargo build … && cp … @OUTPUT@']`.

### 6. Steady main-thread message pump

CEF's `on_schedule_message_pump_work` can be called from any thread. The previous implementation called `glib::timeout_add_local_once`, which panics from a non-main thread. Replaced with an 8 ms `timeout_add_local` installed once on the main thread that calls `cef::do_message_loop_work()`. Slightly more CPU than the schedule-driven pump, but trivially correct and lock-free.

### 7. `appstream-compose: false`

We do not yet ship a valid metainfo + icon set. Letting `appstream-compose` run aborts the build. Re-enabled in M23.

## Risks / Trade-offs

- **CEF tarball changes** → cef-dll-sys integration breaks. Mitigation: pin the exact SHA in the manifest and `download-cef.sh`; the `archive.json` marker provides an extra integrity gate.
- **Crate updates require regenerating `cargo-sources.json`** → easy to forget. Mitigation: add a `tasks.md` checklist item that re-runs `flatpak-cargo-generator.py` whenever `Cargo.lock` changes.
- **8 ms pump wastes CPU when idle** → measurable but small. Mitigation: revisit once CEF upstream documents a thread-safe schedule API; M04 zygote handling still holds inside flatpak so shutdown is clean.
- **`--share=network` in finish-args** → broader than needed, but the shell loads arbitrary URLs by design. Acceptable.
- **No `.Debug` extension** → harder to triage CEF crashes from users. Mitigation: M23 adds it.

## Migration Plan

This milestone is additive — no existing user workflow changes. Contributors who previously ran `download-cef.sh && cargo build` still can; the flatpak path is the new option.

Rollback: revert the `packaging/` directory and the `meson.build` cargo custom_target change. The Rust message-pump fix should stay regardless.

## Open Questions

- Should `cargo-sources.json` regeneration be wired into a CI check that diffs against `Cargo.lock`? Tracked separately; out of scope here.
- Long-term, can we publish CEF as an extension point rather than a baked-in module? Depends on whether GNOME accepts a `org.cef.Runtime` extension upstream.
