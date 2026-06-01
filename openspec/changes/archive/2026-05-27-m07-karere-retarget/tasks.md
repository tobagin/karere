## 1. Cargo and license metadata

- [x] 1.1 Update `Cargo.toml`: rename package to `karere`, set version `4.0.0-dev`, set license `GPL-3.0-or-later`, set binary name `karere`
- [x] 1.2 Add deps: `gettext-rs = { version = "0.7", features = ["gettext-system"] }`, `toml = "0.8"`, `serde = { version = "1", features = ["derive"] }`
- [x] 1.3 Replace `LICENSE` with the GPL-3.0-or-later text from `/home/tobagin/Projects/karere/LICENSE` (byte-for-byte)
- [x] 1.4 Replace `README.md` with karere v3 README plus a leading note that this is the CEF rewrite
- [x] 1.5 Verify `cargo metadata --format-version=1` reports name=karere, version=4.0.0-dev, license=GPL-3.0-or-later

## 2. Meson build wiring

- [x] 2.1 Update top-level `meson.build`: `project('karere', 'rust', version: '4.0.0-dev', ...)`
- [x] 2.2 Add `i18n.gettext('karere', preset: 'glib')` to `meson.build`
- [x] 2.3 Add `subdir('po')` to `meson.build`
- [x] 2.4 Flip post_install so `gtk_update_icon_cache: true` is restored (icons are now present)

## 3. Copy karere assets verbatim

- [x] 3.1 Copy `data/icons/hicolor/**` from `/home/tobagin/Projects/karere/data/icons/hicolor/` to `data/icons/hicolor/`
- [x] 3.2 Copy the five `.oga` files from `/home/tobagin/Projects/karere/data/sounds/` to `data/sounds/`
- [x] 3.3 Copy every `data/ui/*.blp` from `/home/tobagin/Projects/karere/data/ui/` to `data/ui/`
- [x] 3.4 Copy `/home/tobagin/Projects/karere/data/io.github.tobagin.karere.gschema.xml.in` to `data/io.github.tobagin.karere.gschema.xml.in` (byte-for-byte, no edits)
- [x] 3.5 Copy `/home/tobagin/Projects/karere/po/` (LINGUAS, POTFILES.in, *.po, meson.build) to `po/`

## 4. Rename data identifiers

- [x] 4.1 Rename / recreate desktop file as `data/io.github.tobagin.karere.desktop` with karere identifiers
- [x] 4.2 Rename / recreate metainfo as `data/io.github.tobagin.karere.metainfo.xml` with karere identifiers
- [x] 4.3 Rename gresource XML to `data/karere.gresource.xml` and update resource prefix to `/io/github/tobagin/karere/`
- [x] 4.4 Update `data/meson.build` to install desktop, metainfo, gschema, gresource under new names
- [x] 4.5 Update `data/meson.build` to install sounds under `/app/share/karere/sounds/`

## 5. Blueprint build pipeline

- [x] 5.1 In `build.rs`, detect `blueprint-compiler` via `which`; panic with apt/dnf/flatpak SDK install instructions if missing
- [x] 5.2 In `build.rs`, iterate `data/ui/*.blp` and run `blueprint-compiler compile-file <blp> --output $OUT_DIR/ui/<name>.ui` for each
- [x] 5.3 In `build.rs`, emit `cargo:rerun-if-changed=data/ui/<file>.blp` for each blueprint
- [x] 5.4 Propagate `blueprint-compiler` stderr into cargo error output on nonzero exit
- [x] 5.5 Update gresource source paths so each UI file resolves from `$OUT_DIR/ui/<name>.ui` first, falling back to `data/ui/<name>.ui`

## 6. Rust type renames

- [x] 6.1 Rename `ShellApplication` → `KarereApplication` (file + all references)
- [x] 6.2 Rename `ShellWindow` → `KarereWindow` (file + all references)
- [x] 6.3 Rename `CefGtkArea` → `KarereWebView` (file + all references)
- [x] 6.4 Update `mod` declarations and `pub use` re-exports
- [x] 6.5 Verify `cargo build` passes after renames

## 7. Gettext initialization

- [x] 7.1 In `src/main.rs`, call `setlocale(LocaleCategory::LcAll, "")` before GTK init
- [x] 7.2 In `src/main.rs`, call `bindtextdomain("karere", "/app/share/locale")`
- [x] 7.3 In `src/main.rs`, call `textdomain("karere")`
- [x] 7.4 Match the call ordering and error handling of `/home/tobagin/Projects/karere/src/main.rs:23-80`

## 8. GApplication identity

- [x] 8.1 Update `KarereApplication::new` to register app-id `io.github.tobagin.karere`
- [x] 8.2 Update gresource registration to use `/io/github/tobagin/karere/` prefix
- [x] 8.3 Set the GApplication `resource-base-path` property accordingly

## 9. Flatpak packaging

- [x] 9.1 Rename `packaging/io.github.tobagin.GtkCefShell.yml` to `packaging/io.github.tobagin.karere.yml`
- [x] 9.2 Update `app-id` field in the manifest to `io.github.tobagin.karere`
- [x] 9.3 Update module name from `gtk-cef-shell` to `karere` in the manifest
- [x] 9.4 Confirm `finish-args` keeps `LD_LIBRARY_PATH=/app/lib/cef`
- [x] 9.5 Confirm the `cef-binaries` module from M6 is unchanged

## 10. Verification

- [x] 10.1 `cargo build` runs clean from a fresh checkout
- [x] 10.2 `flatpak-builder --user --install --force-clean build-dir packaging/io.github.tobagin.karere.yml` succeeds
- [x] 10.3 `flatpak run io.github.tobagin.karere --url=https://web.whatsapp.com` opens a window with karere icon + about-dialog name
- [x] 10.4 The WhatsApp Web QR pairing page renders inside the CEF view
- [x] 10.5 Meson POT regeneration extracts at least one string from a `.blp` and at least one from a `.rs` file into `po/karere.pot`
