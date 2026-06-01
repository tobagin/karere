## Context

M7 forked the project into Karere v4 and copied karere assets (gschema, blueprints, po, icons) verbatim. M8 is the first milestone that consumes the karere GSettings schema in code. The karere v3 reference implementation already encodes every decision below in `src/main.rs:90-114` (tokio runtime + portal), `src/main.rs:117-465` (action group), `src/main.rs:527-553` (about dialog metainfo parser), and the corresponding `KarereWindow::constructed` geometry binding. M8 ports this behaviour to the CEF shell with one architectural change: any action whose target feature has not yet landed (`refresh*`, `zoom-*`, `notification-clicked`, `switch-account`, `set-unread`, `refresh-tray-accounts`, `open-download`) is wired as a stub now so later milestones only have to fill the body.

The M4 close gate (CEF lifecycle teardown) is already in place; M8 must layer the `close-button-action=background` branch in front of it without breaking the quit path. M9 toasts and M12 download toasts both target an `AdwToastOverlay` that does not yet exist in `data/ui/window.blp` — M8 is the natural place to introduce it because the window tree is already being touched.

## Goals / Non-Goals

**Goals:**
- Geometry survives close-and-relaunch via `window-width`, `window-height`, `window-maximized` GSettings bindings.
- `close-button-action=background` hides the window without tearing down CEF; the existing M4 quit-flow still runs when the key is `quit`.
- `theme` GSetting switches `adw::StyleManager` color-scheme live, no restart.
- `start-in-background=true` plus a configured tray suppresses the initial `present()`; with no tray configured, the gate is a no-op and the window presents normally.
- `app.sync-autostart` action drives the XDG Background portal via `ashpd`, off the GTK main loop, via a static tokio runtime.
- Full action group + accel table registered, with stubs for downstream-milestone actions so wiring is complete from day one.
- `AdwToastOverlay` exists in the window tree ready for M9/M12 consumers.

**Non-Goals:**
- Real zoom logic (M18 owns `web_view.set_zoom_level`).
- Real reload logic (M9 owns RequestHandler reload + crash recovery).
- Tray construction (M15) — M8 only gates `present()` behind the `start-in-background` GSetting and assumes the tray-configured check returns false for now.
- Notification delivery (M14), multi-account state (M20), preferences UI (M22), download manager wiring (M12).
- Migrating window state away from GSettings (e.g., to a state file) — karere uses GSettings and v4 matches.

## Decisions

### D1: Geometry persistence uses `Settings::bind`, not manual save-on-close
**Choice**: In `KarereWindow::constructed`, call `settings.bind("window-width", &self, "default-width", SettingsBindFlags::DEFAULT)` for width, height, and `maximized`.
**Why**: Karere v3 uses the same pattern. `Settings::bind` is bidirectional and survives crashes; manual save-on-close loses state if the process is killed (which CEF can do during shutdown).
**Alternatives considered**: (a) Manual `connect_close_request` save — rejected, loses state on SIGKILL. (b) State file in `XDG_STATE_HOME` — rejected, diverges from karere and from M22 prefs UI which binds the same keys.

### D2: `close-button-action` resolved at close-time, not cached
**Choice**: `connect_close_request` reads `settings.string("close-button-action")` on every invocation and also installs `connect_changed("close-button-action", ...)` to log the new value. No cached copy on the window struct.
**Why**: Avoids stale state when the user toggles the preference and immediately closes the window. Reading a GSetting is cheap (gvariant cache in glib).
**Alternatives considered**: Cache in a `Cell<CloseAction>` and update on `connect_changed` — adds a field and a parse step for negligible benefit.

### D3: Theme binding goes through `adw::StyleManager`, not CSS
**Choice**: In `KarereApplication::startup`, look up `adw::StyleManager::default()`, map `theme` GSetting values to `adw::ColorScheme::{Default, ForceLight, ForceDark}`, call `set_color_scheme(...)`, and re-do the mapping on `connect_changed("theme", ...)`.
**Why**: This is the libadwaita-blessed path and the only one that interacts correctly with the system "Prefer Dark" toggle. Karere v3 does the same.
**Alternatives considered**: GTK CSS provider swap — rejected, fights libadwaita and breaks system theme propagation.

### D4: `start-in-background` gate is a runtime check in `connect_command_line`
**Choice**: After the window is constructed, query `settings.boolean("start-in-background")` AND a `tray_configured()` helper (returns `false` for now, true once M15 lands). Skip `present()` only if both are true.
**Why**: Matches karere v3. Without the tray check, a `start-in-background=true` user with no tray would have no way to bring the window back. The double-gate keeps the bad-UX state unreachable.
**Alternatives considered**: Always honour `start-in-background` even without tray — rejected, traps the user. Use a CLI flag instead — rejected, the GSetting is the contract M22 binds against.

### D5: Background portal call uses a static, leaked tokio runtime
**Choice**: A `OnceLock<&'static tokio::runtime::Runtime>` lives in `actions.rs`. First call to `app.sync-autostart` constructs a `Runtime::new()`, wraps it in `Box::leak`, and stores the pointer. The action body does `runtime.spawn(async move { Background::request_background(...).await })` and returns immediately.
**Why**: `ashpd` is async-first; the GTK main loop is not. Karere v3 uses this exact pattern (`main.rs:90-114`). Leaking is fine because the runtime needs to outlive the application for portal callbacks. One runtime per process, not per action call.
**Alternatives considered**: (a) `glib::MainContext::spawn_local` with `ashpd`'s glib feature — workable but karere already picked tokio and we want one runtime story for M14 too. (b) Block on portal — rejected, freezes the UI.

### D6: Stub actions log `g_warning` and return, not panic
**Choice**: `app.notification-clicked`, `app.switch-account`, `app.set-unread`, `app.refresh-tray-accounts`, `app.open-download`, `win.refresh`, `win.refresh-hard`, `win.zoom-in`, `win.zoom-out`, `win.zoom-reset` are registered with bodies that emit a `g_warning!("action X not yet implemented (milestone MY)")` and return.
**Why**: The action surface needs to be present from M8 so accels, menus, and DBus callers don't 404. `g_warning` is grep-able in journalctl and won't crash the app.
**Alternatives considered**: Don't register stubs — rejected, breaks accel registration and forces menu blueprints to be conditional.

### D7: `AdwToastOverlay` is the new root of the window content area
**Choice**: `data/ui/window.blp` becomes `AdwToolbarView` wrapped in `AdwToastOverlay` (overlay outermost, toolbar-view as child). A template-child `toast_overlay: TemplateChild<AdwToastOverlay>` on `KarereWindow` exposes it.
**Why**: M9 toasts and M12 download notifications need `add_toast`. Adding it now avoids touching the window blueprint twice.

### D8: Full accel table
Registered in `KarereApplication::startup`:

| Action | Accels |
|---|---|
| `app.quit` | `<Primary>q` |
| `app.preferences` | `<Primary>comma` |
| `app.show-help-overlay` | `<Primary>question` |
| `win.toggle-fullscreen` | `F11`, `<Alt>Return` |
| `win.minimize` | `<Primary>m` |
| `win.refresh` | `<Primary>r`, `F5` |
| `win.refresh-hard` | `<Primary><Shift>r` |
| `win.zoom-in` | `<Primary>plus`, `<Primary>equal`, `<Primary>KP_Add` |
| `win.zoom-out` | `<Primary>minus`, `<Primary>KP_Subtract` |
| `win.zoom-reset` | `<Primary>0`, `<Primary>KP_0` |
| `win.close` | `<Primary>w` |

`app.about` is menu-only, no accel.

### D9: About dialog reads metainfo XML at runtime
**Choice**: `app.about` opens an `adw::AboutDialog`. Release notes are parsed from `/app/share/metainfo/io.github.tobagin.karere.metainfo.xml` using a small XML reader (port of karere `main.rs:527-553`).
**Why**: Single source of truth — the metainfo file already ships release notes for AppStream/GNOME Software, no point duplicating into Rust strings.

## Risks / Trade-offs

- **[Risk]** Leaked tokio runtime shows up in valgrind as a memory leak → **Mitigation**: deliberate; document at the `OnceLock` site. The runtime needs to outlive `main`'s scope for portal callbacks.
- **[Risk]** `start-in-background=true` with `tray_configured()` returning `false` for the entire M8..M15 window means the GSetting is silently ignored → **Mitigation**: log `g_info!` at startup when the gate is skipped because tray is not configured; document in proposal Non-Goals.
- **[Risk]** Two-way `Settings::bind` on `default-width` races with the user resizing during shutdown → **Mitigation**: karere v3 ships this same binding without issue; GSettings serialises writes on the main loop.
- **[Risk]** `close-button-action=background` plus no tray = window unreachable → **Mitigation**: same gate as D4; until M15 lands, only the `background` value with a configured tray is honoured. Bare `close-button-action=background` without tray still hides — accept as user-chosen footgun documented in M22 prefs UI.
- **[Risk]** ashpd 0.13 API drift vs karere's pinned version → **Mitigation**: pin `ashpd = "=0.13"` if needed; karere v3 is on the same major.
- **[Trade-off]** Stub action bodies mean some accels (Ctrl+R, F11, etc.) appear in the UI but only some have effect in M8. Acceptable: prefs UI in M22 will list which work.

## Migration Plan

This is a forward-looking change on top of M7's fork — there is no prior CEF-shell deployment with the old action group. Deploy path:
1. Merge M8 onto the M7 branch.
2. `meson compile` regenerates the gresource with the new `window.blp` (toast overlay).
3. First launch reads default GSettings values shipped by the karere gschema; geometry binds populate the keys on first resize.
4. No data migration required: the karere gschema keys are net-new for the v4 binary.

Rollback: revert the M8 commit. GSettings keys remain in the schema but become unread; no on-disk format change.

## Open Questions

- Should `app.sync-autostart` surface the portal result as a toast (`AdwToast` via the new overlay) for failure cases? Karere v3 silently logs. Defer to M22 prefs UI which is the only caller.
- `win.refresh-hard` semantics in CEF (CefBrowser has `Reload` and `ReloadIgnoreCache`) — defer to M9 when RequestHandler lands; stub body is fine now.
