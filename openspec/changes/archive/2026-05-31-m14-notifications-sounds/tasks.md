## 1. Permission handler confirmation

- [x] 1.1 Audit `src/handlers/permission.rs::on_show_permission_prompt` to confirm the `CEF_PERMISSION_TYPE_NOTIFICATIONS` branch defaults to **prompt** with no preselected outcome.
- [x] 1.2 Add a unit/integration check that asserts the dialog does not auto-allow or auto-deny notifications on first visit.
- [x] 1.3 Ensure persisted permission decisions are keyed per origin and survive a Karere restart.

## 2. JS observer (`data/js/notification_observer.js`)

- [x] 2.1 Capture `OrigNotification = window.Notification` and replace `window.Notification` with a `Proxy` whose `construct` trap suppresses Chromium's native banner (do NOT construct the real `Notification`) and returns a `Notification`-shaped stub exposing `close()`, `onclick`, `onclose`, and `tag`.
- [x] 2.2 In the `construct` trap, post `RendererMessage::NotificationSeen { account_id, title, body, icon, tag }` through M13 IPC, deriving `tag` from `opts?.tag`, `body` from `opts?.body`, and resolving `opts?.icon` to bytes/data-URL renderer-side (so the browser need not re-fetch a blob/authed URL).
- [x] 2.3 Maintain a `Map<tag, stub>` of live stubs; on construction add the entry, and provide a way for the page/host to fire `close` → post `NotificationClosed { tag }` and drop the entry.
- [x] 2.4 Define `window.__karereCloseNotif(tag)` (sync page state on host withdrawal) and `window.__karereActivateNotif(tag)` (dispatch the stub's `click` so the page opens the chat).
- [x] 2.5 Inject the observer script at frame load via the existing renderer bootstrap (M13) so it runs before any page script constructs a `Notification`.

## 3. Browser-side `notifications::Tracker`

- [x] 3.1 Create `src/notifications.rs` with `pub struct Tracker { live: Mutex<HashMap<String, Instant>> }` and constructors wired into the application state.
- [x] 3.2 Implement `Tracker::on_seen(&self, tag, title, body, icon, account_id)` to insert/overwrite the tag, request an unread bump for `account_id` (stub for M20), call `play_sound()` when `notify-sound-enabled` is true, and emit the branded notification (3.7).
- [x] 3.3 Implement `Tracker::on_closed(&self, tag)` to remove the entry.
- [x] 3.4 Implement `Tracker::on_focus_gained(&self, app, frame)` to drain tags, call `gio::Application::withdraw_notification('<tag>')` for each (and `__karereCloseNotif` to sync page state), then clear the cache.
- [x] 3.5 Implement `Tracker::on_load_start()` to clear the cache on main-frame navigation.
- [x] 3.6 Wire IPC handlers to dispatch `NotificationSeen`/`NotificationClosed` into the tracker.
- [x] 3.7 Implement branded emit: build a `gio::Notification` with title = sender/group name, body = preview, image = decoded `opts.icon` avatar (fallback Karere icon), a default action that raises the window and calls `__karereActivateNotif('<tag>')`; publish via `gio::Application::send_notification(Some(tag), &notif)`. Compose title/body per `notify-preview-name`/`notify-preview-message`/`notify-preview-length`.

## 4. Window focus integration

- [x] 4.1 Connect `KarereWindow::is_active` notify in `src/window.rs` and call `notifications::Tracker::on_focus_gained` with the active CEF frame when the property transitions to `true`.
- [x] 4.2 Add a debounce so rapid focus toggles do not stampede `execute_java_script` calls.

## 5. Sound playback

- [x] 5.1 Add gresource entries under `data/sounds/` for the bundled sounds (`whatsapp`, `alert`, etc.) and expose them in the gresource XML.
- [x] 5.2 Implement `play_sound(name: &str)` that extracts `<gresource>/sounds/<name>.oga` to `$XDG_RUNTIME_DIR/karere/sounds/<name>.oga` once per session and caches the path.
- [x] 5.3 Spawn `paplay <path>` via `gio::Subprocess::spawn`; on `NotFound`, fall back to `gst-launch-1.0 playbin uri=file://<path>`.
- [x] 5.4 If neither backend is available, log a one-shot warning and return without raising an error.
- [x] 5.5 Verify whether Chromium plays its own notification sound; if so, append the verified `--disable-notification-sound` switch (or successor) in `src/cef_runtime.rs::on_before_command_line_processing`.

## 6. gschema additions

- [x] 6.1 Add gschema keys: `notifications-enabled` (bool, default `true`), `notify-messages` (bool, default `true`), `notify-sound-enabled` (bool, default `true`), `notify-sound-file` (string, default `"whatsapp"`), `notify-preview-name` (bool, default `true`), `notify-preview-message` (bool, default `true`), `notify-preview-length` (enum: `short|medium|long`, default `medium`).
- [x] 6.2 Document inline in the gschema XML that `notify-preview-*` keys do not affect Chromium-rendered banners and are reserved for the future tray peek UI.
- [x] 6.3 Have the permission handler consult `notifications-enabled` and `notify-messages` and deny the permission request when either is `false`.

## 7. Verification

- [x] 7.1 With the window unfocused, send a WhatsApp message and confirm a Karere-branded banner appears (Karere app name + icon, sender/group name, message preview, profile picture) and NO Chromium banner. Verified in flatpak: branded banner with round avatar, "View message" button, no Chromium "Settings" banner.
- [x] 7.2 Click the banner and confirm the window raises and the page opens the correct chat (`__karereActivateNotif`).
- [x] 7.3 Focus Karere and confirm the banner withdraws within ~200 ms via `withdraw_notification`.
- [x] 7.6 Toggle `notify-preview-message=false` (and `notify-preview-name=false`) and confirm the banner hides the body (and sender name).
- [x] 7.4 Set `notify-sound-enabled=false` and confirm no sound plays; set `notify-sound-file=alert` and confirm the alert sound plays. (Default flipped to `false` — WhatsApp Web plays its own in-page ding, so a Karere sound is a double-ding; opt-in only.)
- [x] 7.5 Toggle `notifications-enabled=false` and confirm permission is denied and no sound plays.

## Implementation notes — how interception actually works (CDP)

The original design (suppress + re-emit by Proxying `window.Notification` from
the build-time injected bundle) does NOT reliably work: the page-injected
observer loses the race against WhatsApp's own capture of `window.Notification`,
and CEF 148 exposes **no** notification API to intercept it natively (searched
the whole cef / cef-dll-sys surface — only `CEF_PERMISSION_TYPE_NOTIFICATIONS`,
content-setting flags, and UI theme colors; no `cef_notification_handler_t`).

**What WhatsApp actually does (confirmed by CDP tracing across all target
types):** it raises message notifications with **`new Notification(title, opts)`
in the PAGE realm** — not the service worker, not `showNotification`, not the
Push API.

**Solution — CDP (`src/cdp.rs`):** a background thread connects to the
browser-level CDP endpoint (the existing `--remote-debugging-port`),
`Target.setAutoAttach{flatten}`, and on each attached target evaluates a patch:
- **page** target → `PAGE_PATCH`: Proxy `window.Notification` (suppress native
  banner, forward payload). This is the one that matters.
- **service_worker** target → `SW_PATCH`: override `registration.showNotification`
  (defensive; WhatsApp doesn't currently use it).
Re-applied on every `Runtime.executionContextCreated` for the session (globals
ready only then). **Ordering:** the session must be recorded BEFORE
`Runtime.enable`, which synchronously emits `executionContextCreated` for the
existing context. Payload forwarded via `console.log("__KARERE_NOTIF__:"+json)`
→ host reads `Runtime.consoleAPICalled` (no per-session binding lifecycle, unlike
`Runtime.addBinding`). Host parses → `Tracker::on_seen` → branded
`gio::Notification`. Works inside the flatpak sandbox (CDP port is loopback).

See memory `cef-notifications-cdp.md` for the full rationale + dead ends.

## Polish applied during verification

- **Round avatar** (`notifications::round_avatar`): decode WhatsApp's avatar
  (JPEG/PNG/WebP via `gdk-pixbuf`), scale to 96², circular alpha mask with a 1px
  feathered edge, re-encode PNG — matches WhatsApp Web's round avatars. Falls
  back to raw bytes, then the themed Karere icon.
- **No double ding:** `notify-sound-enabled` default flipped to `false`.
  WhatsApp Web plays its own in-page `<audio>` ding (independent of the desktop
  notification); a Karere sound on top is always a double-ding. Custom-sound
  machinery stays, opt-in.
- **Input focus on launch** (`web_view.rs` click handler): also call
  `set_focus(true)` on click, not only via the `EventControllerFocus` enter
  signal — on launch the GLArea may already hold GTK focus, so `grab_focus` is a
  no-op and the enter signal never fires, leaving CEF unfocused (no input caret
  until the window is de/re-focused).

## Dead code retained for reference

`src/handlers/sw_notify.rs` + `data/sw-notify-shim.js` (the reverted
serviceworker.js-rewrite approach) are kept `#![allow(dead_code)]`, not wired in.

