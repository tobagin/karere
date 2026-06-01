## Why

Every GSetting introduced across M7–M21 (theme, close-button-action, start-in-background, run-on-startup, language, notifications-enabled, notify-messages, notify-sound-enabled, notify-sound-file, notify-preview-*, download-directory, notify-downloads-enabled, notify-download-type, spell-checking-enabled, spell-check-auto-detect, spell-check-languages, permission decisions, reduce-motion, focus-indicators-enhanced, screen-reader-opts, webview-zoom, zoom-controls-headerbar) is currently writable only via `gsettings set`. Karere v3 surfaced these through an `AdwPreferencesDialog` with six sections; v4 must regain that surface so users can configure the shell without a terminal. In parallel, the M8-stubbed `app.show-help-overlay` action has no target dialog — Ctrl+? currently does nothing — so we ship an `AdwShortcutsDialog` describing every accelerator the shell binds. Both dialogs live in blueprint UI files and are wired to existing GActions.

## What Changes

- Add `src/preferences.rs` implementing an `AdwPreferencesDialog` subclass built from a `data/ui/preferences.blp` composite template; structure ported from karere v3's `src/preferences.rs` (611 lines) but with rows updated to match v4 semantics.
- Add `data/ui/preferences.blp` with six pages plus an About footer:
  - **General**: theme dropdown (system/light/dark), close-button-action dropdown (background/quit), start-in-background switch, run-on-startup switch (activates `app.sync-autostart`), language detect-or-pick dropdown.
  - **Notifications**: notifications-enabled, notify-messages, notify-sound-enabled, notify-sound-file dropdown (whatsapp/pop/alert/soft/start), notify-preview-* group with an inline notice "Some settings only apply when Karere controls the popup" (Chromium-native fires the actual popup; previews govern future tray peek).
  - **Downloads**: download-directory row (gtk::Button opening folder picker), notify-downloads-enabled, notify-download-type dropdown (toast/notification/both).
  - **Spellcheck**: spell-checking-enabled, spell-check-auto-detect, language list (GtkListBox with star-pin per language), inline notice "Changing language reloads the page".
  - **Privacy / Permissions**: list of stored (origin, mask, state) decisions from M11, per-row remove button, footer Clear-all button.
  - **Accessibility**: reduce-motion, focus-indicators-enhanced, screen-reader-opts (with restart-required hint), webview-zoom slider (a11y floor), zoom-controls-headerbar switch.
  - **About footer**: version, contributors, license button.
- Add `data/ui/keyboard-shortcuts.blp` implementing an `AdwShortcutsDialog` listing every template-core accelerator from M8 plus Ctrl+F (find-in-page), F12 and Ctrl+Shift+I (devtools), Ctrl+W (close-or-background), Ctrl+B (toggle headerbar zoom-controls if implemented).
- Wire `app.preferences` to present `KarerePreferencesDialog::new(app)` on the active window.
- Wire `app.show-help-overlay` (stubbed in M8) to present the `AdwShortcutsDialog`.
- Register both `.blp` files in `data/resources/resources.gresource.xml` and ensure `blueprint-compiler` runs during the build.

## Capabilities

### New Capabilities
- `preferences-dialog`: AdwPreferencesDialog assembled from a blueprint composite template, with six pages binding every M7–M21 GSetting plus the M11 permission registry and the M20 account-discovery hooks where relevant. About footer surfaces version and license.
- `keyboard-shortcuts-dialog`: AdwShortcutsDialog blueprint enumerating every accelerator the shell binds (template-core actions from M8 plus find/devtools/close/zoom-controls toggle), presented via `app.show-help-overlay`.

### Modified Capabilities
<!-- None: M22 only adds new capabilities. Existing GSettings keys are surfaced, not redefined. -->

## Impact

- New files: `src/preferences.rs`, `data/ui/preferences.blp`, `data/ui/keyboard-shortcuts.blp`.
- Modified files: `src/application.rs` (wire `app.preferences` and `app.show-help-overlay` action handlers to construct/present the dialogs), `data/resources/resources.gresource.xml` (register compiled UI files), `build.rs` or meson rules (invoke `blueprint-compiler` on the two new `.blp` files).
- Build dependency: `blueprint-compiler` must be present at build time; flatpak manifest already lists it for prior `.blp` files.
- Runtime: opening Preferences and toggling theme updates UI immediately; download-directory changes redirect subsequent downloads (M12); star-pinning a spellcheck language persists across restarts (M16); Clear-all permissions empties the M11 registry so the next site request re-prompts; Ctrl+? presents the shortcuts dialog.
- Non-goals: per-account permission editing (lives in M20's account dialog), and any new GSettings keys (M22 only surfaces existing keys).
- Depends on: M7 (gschema), M8 (actions and accelerator wiring), M10 (find/devtools accels), M11 (permission registry read/clear API), M12 (download-directory write), M14 (notification keys), M16 (spell-check language list and pin), M18 (zoom keys), M19 (a11y keys), M20 (account dialog cross-link), M21 (no direct dep, but coexists).
