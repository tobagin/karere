## Why

Karere v3 binds window geometry, theme selection, close-button behavior, and a large app/window action group to GSettings. Karere v4 (CEF edition) must match that surface area before later milestones layer on top: M9 toasts and M12 downloads both require `AdwToastOverlay` to be present in the window tree, M14/M15 need the action group for notification + tray callbacks, and M22 preferences UI binds directly to the same GSettings keys. M8 lands all of that in one milestone so geometry, theme, close-to-background, start-in-background, autostart-via-portal, and the action+accel surface ship together.

## What Changes

- Bind GSettings keys `window-width`, `window-height`, `window-maximized` in `KarereWindow::constructed` so geometry persists across launches.
- Read `close-button-action` GSetting; on `"background"` the close handler calls `set_visible(false)` and returns `Propagation::Stop`; on `"quit"` the existing M4 CEF-close gate runs unchanged. Re-read the key on `connect_changed` so runtime toggles take effect without restart.
- Bind `theme` GSetting (`system | light | dark`) to `adw::StyleManager::default()`'s `color-scheme`; switching the key flips the UI immediately.
- Read `start-in-background` GSetting in `connect_command_line`: when true and the tray is configured, skip `present()` on the constructed window. (Tray itself lands in M15; M8 only gates `present()`.)
- Add `app.sync-autostart` action that calls the XDG Background portal via `ashpd::desktop::background::Background::request_background()` from a static `tokio::runtime::Runtime` (lazy, leaked via `Box::leak`), mirroring karere `src/main.rs:90-114`.
- Add `src/actions.rs` porting the karere action group: `app.quit`, `app.about`, `app.preferences`, `app.show-help-overlay`, `app.sync-autostart`, `app.present-window`, `app.notification-clicked`, `app.switch-account`, `app.set-unread`, `app.refresh-tray-accounts`, `app.open-download`. Actions whose feature lands later (`notification-clicked`, `switch-account`, `set-unread`, `refresh-tray-accounts`, `open-download`) are wired but `g_warning` + return as no-op stubs.
- Add `win.*` actions: `toggle-fullscreen`, `minimize`, `refresh`, `refresh-hard`, `zoom-in`, `zoom-out`, `zoom-reset`, `close`. `refresh*` and `zoom-*` are guarded stubs (`if let Some(host) = ...`) wired against the M9 RequestHandler / M18 zoom interfaces when those land. `win.close` delegates to `close_request` so it respects `close-button-action`.
- Register all accels in `KarereApplication::startup` via `gtk::Application::set_accels_for_action` (full list in design.md).
- Port the karere `adw::AboutDialog` populated from metainfo XML release-notes parser (karere `main.rs:527-553`), reading `/app/share/metainfo/io.github.tobagin.karere.metainfo.xml`.
- Wrap the existing `AdwToolbarView` in `data/ui/window.blp` with an `AdwToastOverlay` (needed by M9/M12). Add `AdwWindowTitle` if not already present.
- Add Cargo deps: `ashpd = { version = "0.13", features = ["gtk4", "background"] }` and `tokio = { version = "1", features = ["rt-multi-thread", "macros", "time"] }`.

## Capabilities

### New Capabilities
- `window-state-persistence`: persist window geometry (width, height, maximized) across launches via GSettings bindings; resolve `close-button-action` and `start-in-background` GSettings at close-time and launch-time respectively.
- `karere-actions`: app-level and window-level action group surface — quit, about, preferences, help-overlay, present-window, sync-autostart, toggle-fullscreen, minimize, refresh, zoom — including stubs for actions whose feature lands in later milestones, plus the full accel table registered via `set_accels_for_action`.
- `theme-binding`: bind the `theme` GSetting (`system | light | dark`) to `adw::StyleManager` color-scheme with live updates on key change.
- `background-mode`: integration with the XDG Background portal via `ashpd` for the `run-on-startup` toggle, plus the close-to-background runtime branch driven by `close-button-action=background`.

### Modified Capabilities
<!-- None. M8 introduces all four capabilities net-new on top of the karere fork laid down by M7. -->

## Impact

- Affected code: `src/actions.rs` (new), `src/application.rs` (action registration, accel table, theme binding, start-in-background gate, tokio runtime singleton), `src/window.rs` (geometry GSettings bind, close-button-action branch, win.* actions), `data/ui/window.blp` (AdwToastOverlay wrap), `Cargo.toml` (ashpd + tokio deps).
- Affected GSettings keys consumed: `window-width`, `window-height`, `window-maximized`, `close-button-action`, `start-in-background`, `run-on-startup`, `theme`. (All keys ship in the karere gschema copied verbatim in M7.)
- Downstream: unblocks M9 (toasts target the new `AdwToastOverlay`), M12 (download toasts), M14 (notification action callbacks land on `app.notification-clicked`), M15 (tray uses `app.present-window` + `app.refresh-tray-accounts`), M18 (zoom actions get real bodies), M20 (multi-account uses `app.switch-account` + `app.set-unread`), M22 (preferences UI binds to the same GSettings keys M8 reads).
- Non-goals (deferred): real zoom logic (M18), reload mechanics + crash recovery (M9), tray construction (M15), notification delivery (M14), multi-account state (M20), preferences UI (M22).
