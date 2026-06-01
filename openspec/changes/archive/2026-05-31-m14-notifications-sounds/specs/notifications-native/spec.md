## ADDED Requirements

### Requirement: Karere-Branded Notification Flow
The system SHALL present Web Notifications branded as Karere — the banner MUST show the Karere application name and icon, NOT "Chromium". Because Chromium's own `org.freedesktop.Notifications` banner is attributed to Chromium and cannot be reliably re-attributed, the system SHALL intercept `window.Notification` construction, suppress Chromium's native banner, and re-emit the notification from the browser process via the host notification API (`gio::Notification` published through `gio::Application::send_notification` with the `io.github.tobagin.karere` app id) carrying the page-supplied title, body, and icon.

#### Scenario: Background message produces a Karere-branded banner
- **WHEN** the page constructs `new Notification(title, opts)` while the Karere window is unfocused
- **THEN** the system SHALL emit a host notification attributed to Karere (Karere name + icon) with the page-supplied title, body, and icon
- **AND** Chromium SHALL NOT render its own parallel banner for the same notification

#### Scenario: Banner content shows sender, preview, and avatar
- **WHEN** the branded banner is shown
- **THEN** it SHALL present the contact or group name (notification title), the message preview (notification body), and the contact/group profile picture (notification image, taken from `opts.icon`)
- **AND** the desktop SHALL attribute it to the Karere application name and icon (from the `io.github.tobagin.karere` desktop entry)
- **AND** the title/body content SHALL honor the `notify-preview-name` and `notify-preview-message` settings (now enforceable because Karere renders the banner)

#### Scenario: Banner is not attributed to Chromium
- **WHEN** the branded banner is shown
- **THEN** the desktop notification SHALL display the Karere application name and icon, not "Chromium"

#### Scenario: Click raises Karere and opens the chat
- **WHEN** the user clicks the Karere-branded banner
- **THEN** the Karere window SHALL be raised
- **AND** the browser SHALL signal the page (e.g. via `__karereActivateNotif('<tag>')`) so the page-side handler navigates to the originating chat

### Requirement: Permission Default Prompt
On first visit, the notifications permission SHALL default to **prompt** through the M11 permission dialog; the system SHALL NOT auto-allow or auto-deny notifications.

#### Scenario: First-visit prompt
- **WHEN** the page requests notification permission for the first time
- **THEN** the M11 `on_show_permission_prompt` handler SHALL surface a dialog with no preselected outcome and SHALL persist the user's choice for subsequent visits

### Requirement: Global Kill-Switch and Toggles
The system SHALL provide gschema keys for notification behavior, including a global kill-switch.

#### Scenario: Kill-switch disables notifications
- **WHEN** `notifications-enabled` is `false`
- **THEN** the system SHALL deny the notification permission and SHALL NOT play any custom sound

#### Scenario: Message toggle gates banners
- **WHEN** `notify-messages` is `false`
- **THEN** the system SHALL deny notification permission for the WhatsApp Web origin even if `notifications-enabled` is `true`

### Requirement: Preview Keys Control Banner Content
Because Karere now renders the banner itself, the system SHALL honor `notify-preview-name`, `notify-preview-message`, and `notify-preview-length` when composing the branded notification.

#### Scenario: Preview-message off hides the body
- **WHEN** `notify-preview-message` is `false`
- **THEN** the branded banner SHALL omit the message preview (e.g. show a generic "New message" body) while still showing the sender/group name unless `notify-preview-name` is also `false`

#### Scenario: Preview-name off hides the sender
- **WHEN** `notify-preview-name` is `false`
- **THEN** the branded banner title SHALL be a generic label (e.g. "Karere") rather than the contact or group name

