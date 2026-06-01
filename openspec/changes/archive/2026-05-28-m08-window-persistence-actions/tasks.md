## 1. Cargo dependencies

- [x] 1.1 Add `ashpd = { version = "0.13", features = ["gtk4", "background"] }` to `Cargo.toml`.
- [x] 1.2 Add `tokio = { version = "1", features = ["rt-multi-thread", "macros", "time"] }` to `Cargo.toml`.
- [x] 1.3 Run `cargo build` to confirm both crates resolve with the requested features and the project still links.

## 2. Window geometry persistence

- [x] 2.1 In `KarereWindow::constructed`, obtain the karere `gio::Settings` instance.
- [x] 2.2 Bind `window-width` ↔ `default-width`, `window-height` ↔ `default-height`, `window-maximized` ↔ `maximized` using `Settings::bind` with `SettingsBindFlags::DEFAULT`.
- [x] 2.3 Manually verify: resize the window, close, relaunch — geometry restored. Repeat for maximized state.

## 3. Close-button-action branch

- [x] 3.1 In `KarereWindow::constructed`, replace/augment the existing `connect_close_request` so it reads `settings.string("close-button-action")` per invocation.
- [x] 3.2 Branch `"background"` → `obj.set_visible(false); glib::Propagation::Stop`.
- [x] 3.3 Branch `"quit"` (and any unknown value) → fall through to the existing M4 CEF-close gate.
- [x] 3.4 Install `settings.connect_changed(Some("close-button-action"), ...)` that logs the new value at info level.
- [x] 3.5 Manually verify: set `close-button-action=background`, click close, window hides; `gio activate` re-presents the same window.

## 4. AdwToastOverlay in window blueprint

- [x] 4.1 Edit `data/ui/window.blp`: wrap the root `AdwToolbarView` in an `AdwToastOverlay` (overlay outermost). Add `AdwWindowTitle` if missing.
- [x] 4.2 Expose the overlay as `#[template_child] toast_overlay: TemplateChild<adw::ToastOverlay>` on `KarereWindow`.
- [x] 4.3 Confirm `meson compile` regenerates the gresource and the build still succeeds.

## 5. Theme binding

- [x] 5.1 Add a helper `fn map_theme(value: &str) -> adw::ColorScheme` covering `"system" | "light" | "dark"` with a `g_warning` fallback to `Default`.
- [x] 5.2 In `KarereApplication::startup`, read `theme` GSetting and call `adw::StyleManager::default().set_color_scheme(map_theme(...))`.
- [x] 5.3 Install `settings.connect_changed(Some("theme"), ...)` that re-runs the mapping on the default `StyleManager`.
- [x] 5.4 Manually verify: `gsettings set io.github.tobagin.karere theme dark` flips the UI live without restart.

## 6. Action group: app.*

- [x] 6.1 Create `src/actions.rs` and a `register_app_actions(app: &KarereApplication)` entry point.
- [x] 6.2 Implement `app.quit` calling the existing M4 shutdown flow.
- [x] 6.3 Implement `app.about` opening an `adw::AboutDialog`. Port the metainfo XML parser from karere `src/main.rs:527-553` to fill release-notes; read `/app/share/metainfo/io.github.tobagin.karere.metainfo.xml`.
- [x] 6.4 Implement `app.preferences` as a stub `g_warning` (M22 owns the body).
- [x] 6.5 Implement `app.show-help-overlay` showing the help overlay if a blueprint exists, otherwise a `g_warning` stub.
- [x] 6.6 Implement `app.present-window` calling `present()` on the primary window if non-null.
- [x] 6.7 Register stub actions `app.notification-clicked`, `app.switch-account`, `app.set-unread`, `app.refresh-tray-accounts`, `app.open-download` — each `g_warning!("action ... not yet implemented (milestone Mxx)")`.

## 7. Action group: win.*

- [x] 7.1 In `KarereWindow::constructed`, register `win.toggle-fullscreen` that flips `is_fullscreen()`.
- [x] 7.2 Register `win.minimize` calling `window.minimize()`.
- [x] 7.3 Register `win.close` calling `window.close()` (so the close-request handler runs and respects `close-button-action`).
- [x] 7.4 Register stub `win.refresh`, `win.refresh-hard`, `win.zoom-in`, `win.zoom-out`, `win.zoom-reset` — guarded `if let Some(host) = ...` blocks with `g_warning` no-ops until M9 / M18 land.

## 8. Accelerator table

- [x] 8.1 In `KarereApplication::startup`, call `set_accels_for_action` for: `app.quit` `<Primary>q`; `app.preferences` `<Primary>comma`; `app.show-help-overlay` `<Primary>question`.
- [x] 8.2 Call `set_accels_for_action` for: `win.toggle-fullscreen` `F11`+`<Alt>Return`; `win.minimize` `<Primary>m`; `win.close` `<Primary>w`.
- [x] 8.3 Call `set_accels_for_action` for: `win.refresh` `<Primary>r`+`F5`; `win.refresh-hard` `<Primary><Shift>r`.
- [x] 8.4 Call `set_accels_for_action` for: `win.zoom-in` `<Primary>plus`+`<Primary>equal`+`<Primary>KP_Add`; `win.zoom-out` `<Primary>minus`+`<Primary>KP_Subtract`; `win.zoom-reset` `<Primary>0`+`<Primary>KP_0`.

## 9. Background portal + tokio runtime

- [x] 9.1 In `src/actions.rs`, declare a `static RUNTIME: OnceLock<&'static tokio::runtime::Runtime> = OnceLock::new();` plus a `fn runtime() -> &'static tokio::runtime::Runtime` accessor that constructs and `Box::leak`s on first call.
- [x] 9.2 Implement `app.sync-autostart`: read `run-on-startup` GSetting, spawn an async task on `runtime()` that calls `ashpd::desktop::background::Background::request_background()`.
- [x] 9.3 Log success/failure of the portal call at info/warn level. Do not block the main loop.

## 10. start-in-background gate

- [x] 10.1 In `KarereApplication::connect_command_line` (after window construction), introduce a `fn tray_configured() -> bool` returning `false` for now (M15 will replace).
- [x] 10.2 Read `start-in-background` GSetting. If `true && tray_configured()`, skip `present()`. Otherwise call `present()` as today.
- [x] 10.3 If gate triggers, log a `g_info` line stating the window was kept hidden.

## 11. Verification

- [x] 11.1 Resize → close → relaunch: geometry restored.
- [x] 11.2 `gsettings set io.github.tobagin.karere close-button-action background` then close → window hides; `flatpak run io.github.tobagin.karere --gapplication-action present-window` re-presents. (Verified via 2nd-launch command-line forwarding, which re-presents the hidden window.)
- [x] 11.3 `gsettings set io.github.tobagin.karere theme dark` flips UI live.
- [x] 11.4 `flatpak run io.github.tobagin.karere -- --gapplication-action quit` exits cleanly with status 0.
- [x] 11.5 `Ctrl+M` minimizes the window; `F11` toggles fullscreen; `Ctrl+W` honours `close-button-action`.
- [x] 11.6 Activate `app.sync-autostart` with `run-on-startup=true` — portal dialog appears; UI stays responsive during the call.
- [x] 11.7 Trigger each stub action and confirm a `g_warning` is logged with no crash.
