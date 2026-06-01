## 1. Tray module

- [x] 1.1 Add `ksni = "0.3"` to `Cargo.toml`
- [x] 1.2 Create `src/tray.rs`, port the body of `karere/src/tray.rs` verbatim
- [x] 1.3 Define `KarereTray { state: Arc<Mutex<TrayState>> }` and `TrayState { unread_count: u32, accounts: Vec<AccountSummary> }`
- [x] 1.4 Implement `ksni::Tray` for `KarereTray`: `id()` returns app-id, `title()` returns `Karere`
- [x] 1.5 Implement `icon_name()` to return `<app-id>-tray-symbolic` / `<app-id>-tray-unread-symbolic` (app-id-prefixed for Flatpak export; `<app-id>` from `FLATPAK_ID`/`APP_ID`) based on `state.unread_count`
- [x] 1.6 Implement `tool_tip()` returning `<n> unread` when `unread_count > 0`, else `Karere`
- [x] 1.7 Implement `activate(_x, _y)` (left-click) to invoke `app.present-window`
- [x] 1.8 Implement `menu()`: dynamic `Show/Hide Window` (label from `state.window_visible`), separator, `Preferences`, `Keyboard Shortcuts`, `About Karere`, separator, `Quit` (no per-account entry in M15)

## 2. Tray lifecycle on the tokio runtime

- [x] 2.1 In shell bootstrap, construct `KarereTray` with the shared state and obtain a `ksni::Service` handle
- [x] 2.2 Spawn `service.run()` onto the M8 tokio runtime; retain the `Handle` for `update()` calls
- [x] 2.3 Pass the `Handle` into the GAction registration so handlers can call `handle.update()` after state mutations

## 3. Desktop-environment auto-detect

- [x] 3.1 Read `XDG_CURRENT_DESKTOP` at startup
- [x] 3.2 When it equals `GNOME`, probe for `org.kde.StatusNotifierWatcher` D-Bus owner via `zbus::fdo::DBusProxy::name_has_owner`
- [x] 3.3 If owner is absent and `KARERE_FORCE_TRAY` is not `1`, log `tray skipped (GNOME w/o AppIndicator)` at INFO and skip `Service::run`
- [x] 3.4 If `KARERE_FORCE_TRAY=1`, bypass the skip unconditionally

## 4. Actions

- [x] 4.1 Fill in `app.set-unread <u32>` (M8 stub): write `state.unread_count`, call `handle.update()`
- [x] 4.2 Fill in `app.present-window` (M8 stub): inspect window `is_visible()` and `is_active()`; hide if visible+active, otherwise `present()`
- [x] 4.3 Fill in `app.refresh-tray-accounts` (M8 stub): no-op until M20, but call `handle.update()` so menu re-renders
- [x] 4.4 Register `app.switch-account <string>` as a logging no-op stub

## 5. Unread feed

- [x] 5.1 In M14's `NotificationSeen` IPC handler, read current `unread_count`, activate `app.set-unread` with `current+1`
- [x] 5.2 In the primary chrome window, connect to `notify::is-active`; on transition to `true`, activate `app.set-unread` with `0`

## 6. Assets

- [x] 6.1 Add `data/icons/hicolor/symbolic/apps/io.github.tobagin.karere-tray-symbolic.svg` (+ `.Devel` variant)
- [x] 6.2 Add `data/icons/hicolor/symbolic/apps/io.github.tobagin.karere-tray-unread-symbolic.svg` (+ `.Devel` variant)
- [x] 6.3 Register the new icons in `meson.build` via the `extra_symbolics` per-profile rename loop (`tray`, `tray-unread`)

## 7. Packaging

- [x] 7.1 Add `--talk-name=org.kde.StatusNotifierWatcher` to `finish-args` in `packaging/io.github.tobagin.karere.yml`

## 8. Verify

- [x] 8.1 On KDE Plasma: tray icon appears within 2 s of launch
- [x] 8.2 On XFCE and Cinnamon: tray icon appears
- [x] 8.3 Trigger three `NotificationSeen` events → tray icon switches to unread variant and tooltip reads `3 unread`
- [x] 8.4 Focus the window → tray icon reverts to default variant, tooltip reads `Karere`
- [x] 8.5 Right-click tray → menu lists `Show/Hide Window`, separator, `Preferences`, `Keyboard Shortcuts`, `About Karere`, separator, `Quit`
- [x] 8.6 Select `Show/Hide Window` → window hides; select again → window shows and presents (toggles on visibility)
- [x] 8.7 Select `Quit` → app exits cleanly (M04 shutdown path runs)
- [x] 8.8 On stock GNOME (no AppIndicator extension): log shows `tray skipped (GNOME w/o AppIndicator)`; no D-Bus error appears at WARN
- [x] 8.9 On stock GNOME with `KARERE_FORCE_TRAY=1`: tray service starts (even if the icon is not visible)
- [x] 8.10 Flatpak build on KDE: tray registers successfully (verify `--talk-name=org.kde.StatusNotifierWatcher` is sufficient)
