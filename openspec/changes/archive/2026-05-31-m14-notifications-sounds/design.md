## Context

Karere v3 used ashpd to push notifications via XDG portal. With CEF/Chromium owning the renderer, Web Notifications flow through `org.freedesktop.Notifications` natively — but that banner is attributed to **Chromium**, not Karere, and cannot be reliably re-attributed. To brand notifications as Karere (and to control their content), we intercept `window.Notification`, suppress Chromium's native banner, forward the full payload to the browser process, and re-emit a branded `gio::Notification` keyed by tag. The same observer also feeds unread counters, the tray badge, focus-driven withdrawal, and optional custom sounds.

M11 already wired `on_show_permission_prompt` for `CEF_PERMISSION_TYPE_NOTIFICATIONS`. M13 added a bidirectional IPC channel between renderer JS and the browser process. M15 (tray) and M20 (accounts) depend on the seen/closed feed produced here.

## Goals / Non-Goals

**Goals:**
- Present notifications branded as Karere (app name + icon) showing sender/group name, message preview, and profile picture, by intercepting `window.Notification` and re-emitting a host `gio::Notification`.
- Track live tags from the page for unread counters and withdraw-on-focus.
- Permission default is **prompt** on first visit; user-controlled thereafter.
- Honor `notify-preview-name` / `notify-preview-message` / `notify-preview-length` when composing the banner (now that Karere renders it).
- Provide an optional custom sound layer gated by `notify-sound-enabled`.

**Non-Goals:**
- Tray badge counts (M15 consumes the seen feed).
- Per-account unread tallies (M20 owns the per-account counter).
- Persisting notification history across restarts.

## Decisions

1. **Intercept + re-emit branded.** The observer `Proxy` over `window.Notification` suppresses Chromium's native banner and forwards the full payload; the browser re-emits a `gio::Notification` (sender name as title, preview as body, `opts.icon` avatar as the notification image) via `gio::Application::send_notification` keyed by tag, so the desktop attributes it to the Karere `.desktop` (app name + icon). The returned object is `Notification`-shaped so page `onclick`/`close` still work.
2. **Observer forwards the full payload.** It posts `NotificationSeen { account_id, title, body, icon, tag }` and `NotificationClosed { tag }` via M13 IPC. The icon is the avatar URL the page supplies; the browser fetches/decodes it into the `gio::Notification` image.
3. **Withdraw-on-focus uses host withdrawal.** When `KarereWindow::is_active` becomes true, the browser calls `gio::Application::withdraw_notification('<tag>')` for each cached tag, then clears the cache. (A page-side `__karereCloseNotif` hook is retained only to keep page state in sync.)
4. **Click routing.** The branded notification's default action raises the Karere window and signals the page (`__karereActivateNotif('<tag>')` via `Frame::execute_java_script`) so the page opens the originating chat.
5. **Sound playback is a separate path.** `Tracker::on_seen` triggers `play_sound()` only when `notify-sound-enabled` is true. Files are extracted from gresource to `$XDG_RUNTIME_DIR/karere/sounds/` once per session, then played via `gio::Subprocess::spawn(["paplay", path])` with a `gst-launch-1.0 playbin uri=file://...` fallback.
6. **Chromium sound suppression is conditional.** If Chromium also plays a sound and double-audio is observed, append `--disable-notification-sound` (exact switch to be verified) in `cef_runtime::on_before_command_line_processing`. Long-term, prefer per-site `RequestContext::set_preference` once M20 lands.
7. **Permission default is prompt.** The M11 dialog must not auto-allow; this change confirms that and is otherwise a no-op for the permission handler.
8. **`notify-preview-*` keys are enforceable.** Because Karere composes the banner, `notify-preview-name`/`notify-preview-message`/`notify-preview-length` control the rendered title/body directly (sender name hidden, body replaced with a generic string, or length-truncated) rather than being tray-peek-only.

## Risks / Trade-offs

- **Avatar fetch.** `opts.icon` is often a `blob:`/`data:` URL or an authenticated WhatsApp URL only valid in the renderer. The observer SHOULD resolve it to bytes renderer-side (e.g. canvas/`fetch`→data URL) and forward those, since the browser process cannot re-fetch an authenticated/blob URL. Fallback to the Karere icon when the avatar cannot be resolved.
- **Lost native click routing.** Re-emitting our own notification means the click no longer flows through Chromium to the page automatically; we reconstruct it by raising the window and calling `__karereActivateNotif('<tag>')`. If the page exposes no open-by-tag hook, clicking only raises the window.
- **Tag collisions.** Tags are page-supplied strings; two banners with the same tag share an entry. WhatsApp Web reuses tags per chat, so the `HashMap<String, Instant>` is acceptable, but `on_seen` must overwrite cleanly.
- **Lost `close` events.** If the page never fires `close` (e.g., user dismisses outside Chromium's awareness), tags can leak. Mitigation: on focus, after the JS sweep, clear the map.
- **Double sounds.** Chromium's own audio + our `paplay` could overlap. Mitigation: empirical check; fall back to `--disable-notification-sound` switch.
- **`paplay` missing.** Some minimal hosts lack PulseAudio CLI. Mitigation: `gst-launch-1.0` fallback, then silent no-op with a warn log.
- **Frame survival.** `Frame::execute_java_script` requires a valid main frame; during reloads the cached tags may target a dead frame. Mitigation: clear the tag cache on `OnLoadStart`.
- **Permission UX.** Default-prompt means users see a CEF dialog on first message; acceptable per locked decision but worth callouts in onboarding (M19).

