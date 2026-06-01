## Why

Karere v3 shipped a system tray icon with an unread badge, an account submenu, and Show/Hide/Quit menu actions (`src/tray.rs`, 188 lines). v4 needs parity. The `ksni` crate (0.3) implements the platform-agnostic `StatusNotifierItem` D-Bus protocol used by KDE Plasma, XFCE, Cinnamon, Budgie, and Pantheon. GNOME does not implement the spec natively, so the shell auto-detects the desktop environment and skips the tray on GNOME unless an AppIndicator-style extension is installed or the user opts in via `KARERE_FORCE_TRAY=1`.

## What Changes

- Add `src/tray.rs`: a verbatim port of the Karere v3 `tray.rs` with strings and icons swapped to the v4 app-id. Defines a `KarereTray` struct implementing `ksni::Tray` (icon, title, tooltip, menu, left-click activate).
- Tray icon name flips between `karere-tray-symbolic` and `karere-tray-unread-symbolic` based on a `has_unread` flag held in a shared `Arc<Mutex<TrayState>>`.
- Tooltip reads `<n> unread` when `unread_count > 0`, otherwise `Karere`.
- Menu items: per-account submenu placeholder (a single `Karere` entry until M20 wires `AccountManager::get_accounts_sorted`), separator, `Show / Hide Karere` (invokes `app.present-window`), `Quit` (invokes `app.quit`).
- Host `ksni::Service::run` on the tokio runtime introduced in M8.
- Auto-detect-DE policy in `src/main.rs` (or shell bootstrap): inspect `XDG_CURRENT_DESKTOP`; when it equals `GNOME` and no AppIndicator D-Bus name owner is detected, log `tray skipped (GNOME w/o AppIndicator)` and do not start the service. The env override `KARERE_FORCE_TRAY=1` always starts the tray.
- Fill in M8-stubbed actions:
  - `app.set-unread <count>` — write the new count into the shared `TrayState` and request a tray refresh.
  - `app.present-window` — toggle window visibility (show + present, or hide if already visible and active).
  - `app.refresh-tray-accounts` — rebuild the account submenu; no-op until M20 provides accounts.
  - `app.switch-account <id>` — no-op stub so menu items can target it.
- Unread feed: M14's `NotificationSeen` IPC handler calls `app.activate_action("set-unread", current+1)`. Window `is-active` becoming `true` (focus gained) activates `set-unread` with `0`.
- `Cargo.toml`: add `ksni = "0.3"`.
- `packaging/io.github.tobagin.karere.yml`: add `--talk-name=org.kde.StatusNotifierWatcher` to `finish-args`.

## Capabilities

### New Capabilities
- `tray-sni`: `StatusNotifierItem` tray icon via `ksni`, including the unread-count state machine, DE auto-detect skip policy, `KARERE_FORCE_TRAY` override, and the `app.set-unread` / `app.present-window` actions that drive it.
- `tray-account-menu`: Tray context menu structure (per-account submenu placeholder, Show/Hide, Quit) and the `app.refresh-tray-accounts` / `app.switch-account` stub actions that M20 fills in.

### Modified Capabilities
<!-- None: M15 introduces tray capabilities; M8 only stubbed the action names. -->

## Impact

- Code: new `src/tray.rs`; edits to `src/main.rs` (or shell bootstrap) to start the tray service, and to the action-registration site for the four actions.
- Assets: two new symbolic icons (`karere-tray-symbolic`, `karere-tray-unread-symbolic`) shipped under `data/icons/`.
- Dependencies: `ksni = "0.3"` added to `Cargo.toml`.
- Packaging: `--talk-name=org.kde.StatusNotifierWatcher` added to the Flatpak manifest's `finish-args`.
- Runtime: on KDE/XFCE/Cinnamon the tray icon appears at startup; on GNOME the tray is silently skipped unless an extension or `KARERE_FORCE_TRAY=1` is present.
- Unblocks: M20 (account manager) plugs into `app.refresh-tray-accounts` and `app.switch-account`; M14's notification path now has a tray sink for unread counts.
