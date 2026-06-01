# notification-observer Specification

## Purpose
TBD - created by syncing change m14-notifications-sounds. Update Purpose after archive.

## Requirements
### Requirement: Notification Proxy Observer
The renderer SHALL inject a JavaScript observer that wraps `window.Notification` via a `Proxy` over its constructor. To brand notifications as Karere, the observer SHALL suppress Chromium's native banner and forward the full payload to the browser process for branded re-emission, while still returning a working `Notification`-shaped object to the page (so page code that listens for `click`/`close` continues to function).

#### Scenario: Construction forwards payload and suppresses native banner
- **WHEN** the page invokes `new Notification(title, opts)`
- **THEN** the observer SHALL NOT let Chromium render its own banner for this notification
- **AND** SHALL post `RendererMessage::NotificationSeen { account_id, title, body, icon, tag }` via the M13 IPC channel so the browser can re-emit a Karere-branded notification
- **AND** SHALL return a `Notification`-shaped object whose `close`/`onclick` the page can still use

#### Scenario: Close posts NotificationClosed
- **WHEN** the constructed `Notification` fires its `close` event
- **THEN** the observer SHALL post `RendererMessage::NotificationClosed { tag }` via the M13 IPC channel

### Requirement: Page-Side Close Hook
The observer SHALL expose `window.__karereCloseNotif(tag)` that closes the live `Notification` whose `tag` matches by invoking `.close()` on the cached object.

#### Scenario: Browser-triggered withdrawal
- **WHEN** the browser process calls `Frame::execute_java_script("__karereCloseNotif('<tag>')", "karere://withdraw", 0)`
- **THEN** the observer SHALL look up the cached `Notification` for `<tag>`, SHALL call `.close()` on it, and Chromium SHALL withdraw the platform banner

### Requirement: Browser-Side Tracker
The browser process SHALL maintain a `notifications::Tracker` keyed by tag that records seen tags, drives unread bumps, and triggers sound playback.

#### Scenario: Seen emits a branded notification and updates the tracker
- **WHEN** the browser receives `NotificationSeen { account_id, title, body, icon, tag }`
- **THEN** the tracker SHALL insert `(tag, Instant::now())`, SHALL request an unread increment for `account_id` (consumed by M20 once available), SHALL invoke sound playback if enabled
- **AND** SHALL publish a Karere-branded `gio::Notification` (title, body, icon, default action raising the window) via `gio::Application::send_notification` keyed by `tag`

#### Scenario: Closed clears tracker entry
- **WHEN** the browser receives `NotificationClosed { tag }`
- **THEN** the tracker SHALL remove the entry for `tag`

### Requirement: Withdraw-on-Focus
When the Karere window becomes active, the browser process SHALL iterate cached tags and request page-side closure for each, then clear the tag cache to prevent leaks.

#### Scenario: Focus withdraws all live banners
- **WHEN** `KarereWindow::is_active` transitions to `true`
- **THEN** the browser SHALL call `Frame::execute_java_script("__karereCloseNotif('<tag>')", "karere://withdraw", 0)` for every cached tag and SHALL then clear the tag cache

### Requirement: Tag Cache Reset on Navigation
The tracker SHALL clear its tag cache when the main frame begins a new load to avoid targeting stale frames.

#### Scenario: Reload clears cache
- **WHEN** the main frame fires `OnLoadStart`
- **THEN** the tracker SHALL clear all cached tags
