## 1. Blueprint files and build wiring

- [ ] 1.1 Create `data/ui/preferences.blp` with an `AdwPreferencesDialog` template containing six `AdwPreferencesPage` children (General, Notifications, Downloads, Spellcheck, Privacy, Accessibility) and an About footer.
- [ ] 1.2 Create `data/ui/keyboard-shortcuts.blp` with an `AdwShortcutsDialog` template grouping rows by category (Navigation, Editing, View, System).
- [ ] 1.3 Register both compiled `.ui` outputs in `data/resources/resources.gresource.xml`.
- [ ] 1.4 Ensure the build invokes `blueprint-compiler` on `data/ui/preferences.blp` and `data/ui/keyboard-shortcuts.blp` (extend existing meson/cargo rule used for prior `.blp` files).

## 2. Preferences dialog Rust subclass

- [ ] 2.1 Add `src/preferences.rs` declaring `KarerePreferencesDialog` as a `glib::Object` subclass of `AdwPreferencesDialog` with the blueprint composite template attribute pointing at the compiled UI.
- [ ] 2.2 Add a `new(app: &KarereApplication) -> Self` constructor that holds a `gio::Settings` reference for the application schema.
- [ ] 2.3 Bind General page rows: theme dropdown, close-button-action dropdown, start-in-background switch, run-on-startup switch, language detect-or-pick dropdown.
- [ ] 2.4 On `run-on-startup` change, activate the `app.sync-autostart` action.
- [ ] 2.5 Bind Notifications page rows: notifications-enabled, notify-messages, notify-sound-enabled, notify-sound-file dropdown, notify-preview-* group; render the inline notice "Some settings only apply when Karere controls the popup".
- [ ] 2.6 Bind Downloads page rows: download-directory (button opens `gtk::FileDialog::select_folder` and writes to the GSetting), notify-downloads-enabled switch, notify-download-type dropdown.
- [ ] 2.7 Bind Spellcheck page rows using the keys M16 reuses: `enable-spell-checking` switch, `auto-detect-language` switch, `enable-auto-correct` switch, and a language list mirroring the headerbar dropdown — rows from `spellcheck::KNOWN_LANGUAGES` with star toggles writing `favorite-spell-check-languages` (favorites on top), selection writing `spell-checking-languages` and calling `KarereWebView::set_spellcheck_languages` for a live switch. NO "reloads the page" notice (live switch performs no reload). Reuse the `spellcheck_ui` model/sorter/factory from M16. (Auto-correct behavior itself lands in m16-1-osr-context-menu; here only the toggle.)
- [ ] 2.8 Bind Privacy page rows: read the M11 permission registry on dialog open, render one row per (origin, mask, state) entry with a per-row remove button calling the M11 remove API, and a footer Clear-all button calling the M11 clear API.
- [ ] 2.9 Bind Accessibility page rows: reduce-motion, focus-indicators-enhanced, screen-reader-opts (with restart-required subtitle), webview-zoom slider, zoom-controls-headerbar switch.
- [ ] 2.10 Populate the About footer with `env!("CARGO_PKG_VERSION")` and a license button that opens the bundled license document.

## 3. Shortcuts dialog Rust glue

- [ ] 3.1 In `data/ui/keyboard-shortcuts.blp`, add `AdwShortcutsItem` rows for every accelerator registered by M8's template-core action wiring.
- [ ] 3.2 Add rows for Ctrl+F (find-in-page), F12 (devtools), Ctrl+Shift+I (devtools), Ctrl+W (close-or-background), and Ctrl+B (toggle headerbar zoom-controls).
- [ ] 3.3 In `src/application.rs`, replace the stub `app.show-help-overlay` handler with one that builds the `AdwShortcutsDialog` from the resource path and calls `dialog.present(Some(&active_window))`.

## 4. Preferences action wiring

- [ ] 4.1 In `src/application.rs`, register a handler for the existing `app.preferences` action (or add the action if not yet present) that constructs `KarerePreferencesDialog::new(&self)` and calls `dialog.present(Some(&active_window))`.
- [ ] 4.2 Add `app.preferences` to the application menu so users can reach it without an accelerator.
- [ ] 4.3 M15 follow-up: the tray menu's `Preferences` item already activates `app.preferences` (currently a stub that only logs at WARN). Once 4.1 lands, confirm the tray `Preferences` item opens the dialog — no tray-side change needed, it shares the action.

## 5. Verification

- [ ] 5.1 Manual verify: open Preferences, change theme to Dark, observe immediate libadwaita color-scheme change.
- [ ] 5.2 Manual verify: in Downloads, pick `/tmp/karere-dl` via the folder picker, trigger a download, confirm the file lands in `/tmp/karere-dl`.
- [ ] 5.3 Manual verify: in Spellcheck, star Portuguese (Brazil), restart the app, reopen Preferences, confirm it is at the top of both the Preferences list and the headerbar dropdown; select a different language and confirm the page does NOT reload and underlines switch live.
- [ ] 5.7 Manual verify: toggle "Enable Auto-Correct" and confirm `enable-auto-correct` updates (`gsettings get`); behavior verified under m16-1-osr-context-menu.
- [ ] 5.4 Manual verify: in Privacy, click Clear-all, confirm the M11 registry is empty (check via `gsettings get` of the underlying key or by visiting a site whose permission was previously stored and observing the M5 prompt reappears).
- [ ] 5.5 Manual verify: press Ctrl+? and confirm the `AdwShortcutsDialog` opens listing every accelerator from section 3 above.
- [ ] 5.6 Manual verify: toggle `run-on-startup` on, confirm the autostart desktop file is regenerated under `~/.config/autostart/`.

## 6. Documentation

- [ ] 6.1 Note the blueprint-compiler build dependency in the developer setup notes alongside any prior `.blp` notes (no new flatpak manifest changes required).
- [ ] 6.2 Reference M22 from M19's section-5 documentation pointer (the accessibility milestone deferred its UI surface to M22).
