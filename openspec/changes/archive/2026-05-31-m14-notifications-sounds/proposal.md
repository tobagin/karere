## Why

Karere v3 routed notifications through the ashpd portal. CEF/Chromium routes Web Notifications through `org.freedesktop.Notifications`, but that banner is attributed to **Chromium**, not Karere. To brand notifications as Karere (app name + icon) and control their content (sender/group name, message preview, profile picture), we intercept `window.Notification`, suppress Chromium's native banner, and re-emit a branded `gio::Notification` from the browser process. Permission default is **prompt** on first visit (locked decision). The same observer feeds unread counts, withdraws banners on focus, and triggers optional custom sounds.

## What Changes

- Confirm M11 permission handler defaults notifications to prompt (no auto-allow).
- Add a JS observer (`data/js/notification_observer.js`) that Proxies `window.Notification`, suppresses Chromium's native banner, resolves the avatar (`opts.icon`) to bytes, and posts `NotificationSeen { title, body, icon, tag }` / `NotificationClosed` via M13 IPC.
- Re-emit a Karere-branded `gio::Notification` (sender/group name, preview, profile picture; app name + icon from the desktop entry) via `gio::Application::send_notification`, honoring `notify-preview-*`. Clicking raises the window and calls `__karereActivateNotif('<tag>')` to open the chat.
- Add `src/notifications.rs` with a `Tracker` (live tags, unread bumps, sound trigger, branded emit) and `on_focus_gained` that withdraws banners via `gio::Application::withdraw_notification('<tag>')` when the window becomes active.
- Custom sound playback via `paplay` (fallback `gst-launch-1.0`); extract `.oga` from gresource to `$XDG_RUNTIME_DIR/karere/sounds/` once. If Chromium double-plays, append `--disable-notification-sound` in `cef_runtime::on_before_command_line_processing` (exact switch to be verified).
- gschema keys: `notifications-enabled`, `notify-messages`, `notify-sound-enabled`, `notify-sound-file`, `notify-preview-name`, `notify-preview-message`, `notify-preview-length` (with documented caveat that preview-* settings are not enforceable while Chromium renders — they apply to the future tray peek UI only).

## Capabilities

### New Capabilities
- `notifications-native`: Karere-branded notifications (app name + icon, sender/group name, preview, profile picture) re-emitted from intercepted Web Notifications, with permission default prompt, gschema toggles, and a global kill-switch.
- `notification-observer`: JS Proxy over `window.Notification` that suppresses the native banner and forwards the full payload, plus a browser-side `Tracker` that re-emits the branded notification and records seen/closed tags for unread tracking and focus-driven withdrawal.
- `notification-sounds`: Optional custom sound playback via `paplay` from gresource-extracted `.oga` files, gated on `notify-sound-enabled`, with Chromium sound suppression if needed.

### Modified Capabilities
<!-- None: M11 permission prompt is reused as-is. -->

## Impact

- New: `src/notifications.rs`, `data/js/notification_observer.js`, sound assets under `data/sounds/`.
- Modified: `src/handlers/permission.rs` (confirm prompt default), `src/cef_runtime.rs` (`on_before_command_line_processing` for sound suppression switch), `src/window.rs` (wire `is-active` to `on_focus_gained`), gschema XML.
- Dependencies: relies on M11 (permission prompts), M13 (JS↔Rust IPC), and feeds M15 (tray badge) and M20 (per-account unread).

