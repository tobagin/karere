## Context

System trays on Linux are not a single API: KDE/XFCE/Cinnamon/Budgie/Pantheon implement the freedesktop `StatusNotifierItem` (SNI) D-Bus protocol, while GNOME deliberately dropped tray-icon support and relegates SNI to third-party extensions (AppIndicator, TopIcons Fix). The `ksni` crate (0.3) speaks SNI over `zbus` and runs entirely from a Tokio task. Karere v3's `src/tray.rs` already proved the integration model: a single `KarereTray` struct implementing `ksni::Tray`, a shared mutable state for unread counts, and GAction wiring for the menu items. M15 ports that model verbatim, swapping app-id-specific strings/icons, and adds an auto-detect policy so the binary behaves correctly under GNOME.

## Goals / Non-Goals

**Goals:**
- Tray icon visible on KDE Plasma, XFCE, Cinnamon, Budgie, and Pantheon out of the box.
- Tray icon shows an unread badge variant when unread count > 0.
- Tray tooltip shows the unread count when non-zero.
- Right-click menu offers Show/Hide and Quit; left-click toggles window visibility.
- On GNOME without an SNI extension, the tray is silently skipped (no D-Bus errors logged at WARN level).
- `KARERE_FORCE_TRAY=1` overrides the GNOME skip.
- The action surface (`app.set-unread`, `app.present-window`, `app.refresh-tray-accounts`, `app.switch-account`) is wired now so M14 (notifications) and M20 (accounts) can target it without further plumbing.

**Non-Goals:**
- Per-account avatars in the submenu — M20 wires `AccountManager::get_accounts_sorted` into `app.refresh-tray-accounts`.
- Custom tray icons per account (M20+).
- A preferences toggle to disable the tray (M22 adds the row; the gschema key is out of scope here — env override is sufficient for v4.0).
- Supporting the legacy XEmbed tray protocol (`_NET_SYSTEM_TRAY_S0`). SNI-only.

## Decisions

**1. Use `ksni = "0.3"`, not a hand-rolled `zbus` implementation.**
`ksni` already handles `StatusNotifierWatcher` registration, the `DBusMenu` protocol, icon path/name negotiation, and reconnect-on-bus-restart. Hand-rolling would duplicate hundreds of lines for no benefit. Alternative considered: `appindicator` C bindings via `libappindicator3-1` — rejected because the C library is unmaintained, GTK3-only, and adds a build-time system dependency.

**2. Auto-detect via `XDG_CURRENT_DESKTOP`, not via a runtime D-Bus probe.**
Reading `XDG_CURRENT_DESKTOP` is synchronous and free. Probing for an `org.kde.StatusNotifierWatcher` D-Bus owner would be more accurate (it catches GNOME-with-AppIndicator-extension cases) but adds an async startup step. Compromise: read `XDG_CURRENT_DESKTOP` first; on GNOME, additionally probe for an existing `StatusNotifierWatcher` D-Bus name owner before deciding. The probe runs on the tokio runtime started in M01.

**3. `KARERE_FORCE_TRAY=1` overrides the skip.**
Power users on GNOME with custom setups (e.g., a tiling window manager replacing the panel) can force the tray. Documented via a comment in `src/tray.rs` and the M15 release notes; no UI surface.

**4. Tray runs on the tokio runtime, not on a dedicated `std::thread`.**
The runtime was introduced in M8 for portal calls; `ksni::Service::run` is async and integrates naturally. Reusing the runtime avoids a second async executor and keeps shutdown coherent (the runtime joins on `App::shutdown`).

**5. Cross-thread state via `Arc<Mutex<TrayState>>`.**
`TrayState { unread_count: u32, accounts: Vec<AccountSummary> }`. The tray task reads on every `icon_name`/`menu`/`tool_tip` call (these are pulled by `ksni` on its own schedule); GAction handlers write under a short-held lock. `ksni::Handle::update()` is invoked from the GAction handler after mutation to push a refresh. Alternative considered: `tokio::sync::watch` channel — rejected because `ksni::Tray` trait methods are sync and would need `block_on`.

**6. Window visibility toggle lives behind `app.present-window`, not behind the tray directly.**
This keeps the tray module ignorant of GTK; M8 already established the action surface for window control. The action handler inspects `window.is_visible()` and `window.is_active()`: if visible-and-active, `hide()`; otherwise `present()`.

**7. Unread reset on focus, not on first message-view scroll.**
M14's notification path increments via `app.set-unread current+1`. The window subscribes to `is-active` notifications; on `true`, it activates `app.set-unread 0`. Approximates v3 behavior closely enough; per-account unread tracking is M20's responsibility.

**8. Two distinct icons (`karere-tray-symbolic`, `karere-tray-unread-symbolic`) rather than a numerical badge overlay.**
Symbolic SVGs theme correctly across desktops; overlay badges with numerals require per-DE Painter hacks (`ksni` only takes an icon name or pixmap). The unread variant is a visual cue, not a count display — the actual number lives in the tooltip and menu. Alternative considered: dynamically generated pixmap with the number rendered via cairo — rejected as over-engineering for v4.0.

**9. Flatpak `--talk-name=org.kde.StatusNotifierWatcher` is required.**
Without this finish-arg, the sandboxed app cannot register its tray item on KDE. XFCE/Cinnamon's `StatusNotifierWatcher` also lives under the `org.kde` name (by spec). No additional D-Bus rule needed for the menu protocol.

## Risks / Trade-offs

- [Risk] `ksni` 0.3 has had occasional Wayland-vs-X11 icon-path bugs → Mitigation: rely solely on `icon_name` (themed lookup), not on `icon_pixmap`. Symbolic icons must be installed under `data/icons/hicolor/symbolic/apps/`.
- [Risk] On GNOME with an AppIndicator extension that came and went (extension disabled mid-session), the tray icon disappears with no recovery → Acceptable: SNI is a stateless protocol; the next app restart re-detects. Documented as known limitation.
- [Risk] `ksni::Handle::update()` called from a GAction handler can race with the tray task's polling read of state → Mitigation: the `Mutex` ensures atomicity per field; `ksni` calls `icon_name`/`tool_tip`/`menu` in sequence after `update`, so the snapshot is consistent.
- [Trade-off] No `XEmbed` fallback means very old desktops (e.g., LXDE without an SNI host) get no tray. Acceptable for v4.0 — Karere v3 already required SNI.
- [Risk] The `KARERE_FORCE_TRAY=1` override could surprise users who set it once and forget → Acceptable: the env var is documented as opt-in; the log line `tray skipped (GNOME w/o AppIndicator)` makes the alternative state visible.
- [Risk] `app.set-unread` racing with `app.present-window` (both modifying state from different code paths) → Mitigation: serialize through GLib's main context (GActions dispatch on the main thread already); the `Arc<Mutex<TrayState>>` covers cross-thread reads from the tokio task.

## Migration Plan

- No data migration: tray state is ephemeral, rebuilt on each launch.
- Existing installs upgrade transparently; no gschema changes.
- Users on GNOME see no tray (same as v3 without the extension). Users on KDE/XFCE/Cinnamon see the tray immediately.
- Rollback: revert the binary; the `--talk-name` finish-arg in the Flatpak manifest is harmless if the corresponding code is absent.

## Open Questions

- ~~Should the Show/Hide label be dynamic?~~ **Resolved:** the label is dynamic — `Hide Window` when the window is visible, `Show Window` otherwise — synced from the window's `notify::visible` into `TrayState.window_visible`. The M15 menu is app-level (Show/Hide, Preferences, Keyboard Shortcuts, About, Quit) with no per-account entry; per-account UI moves to M20. Tray icons are app-id-prefixed (`<app-id>-tray[-unread]-symbolic`) because Flatpak only exports `$FLATPAK_ID*` icons to the host icon theme.
- Per-account unread aggregation: when M20 lands, does the tray show the sum across accounts, or the active account's count? Defer to M20 design; M15 just exposes the single-count surface.
- Should `app.present-window` cycle through windows when multiple are open (e.g., DevTools detached)? For now, target the primary chrome window only; multi-window cycling is M21+ territory.
