# WebKit Notification Bridge

## ADDED Requirements

### Requirement: WebKit Notification Bridge Class Creation
The application SHALL provide a `WebKitNotificationBridge` class in `src/managers/WebKitNotificationBridge.vala` that bridges WebKit notification permissions and events to native notifications.

#### Scenario: Notification bridge instantiation
**GIVEN** the application creates a new Window
**WHEN** WebKitNotificationBridge is instantiated
**THEN** the manager receives Settings, NotificationManager, and parent window references
**AND** the manager is ready to handle permission requests

### Requirement: WebView Permission Handling Setup
The notification bridge SHALL connect to WebView permission request signals.

#### Scenario: Setup with WebView
**GIVEN** WebKitNotificationBridge is instantiated
**WHEN** setup() is called with a WebKit.WebView
**THEN** the manager connects to permission_request signal
**AND** the manager is ready to handle notification permissions

### Requirement: Notification Permission Request Handling
The notification bridge SHALL handle WebKit notification permission requests by checking saved permissions or prompting the user.

#### Scenario: First-time notification permission request
**GIVEN** WebKit requests notification permission
**AND** GSettings shows web-notification-permission-asked=false
**WHEN** on_permission_request() is called
**THEN** a native permission dialog is shown
**AND** the request is not immediately granted or denied

#### Scenario: Previously granted permission
**GIVEN** WebKit requests notification permission
**AND** GSettings shows web-notification-permission-asked=true
**AND** GSettings shows web-notification-permission-granted=true
**WHEN** on_permission_request() is called
**THEN** the permission request is immediately allowed
**AND** notification handler is set up
**AND** no dialog is shown

#### Scenario: Previously denied permission
**GIVEN** WebKit requests notification permission
**AND** GSettings shows web-notification-permission-asked=true
**AND** GSettings shows web-notification-permission-granted=false
**WHEN** on_permission_request() is called
**THEN** the permission request is immediately denied
**AND** no dialog is shown

#### Scenario: Non-notification permission request
**GIVEN** WebKit requests a non-notification permission (e.g., geolocation)
**WHEN** on_permission_request() is called
**THEN** the handler returns false
**AND** the permission request is not handled by this bridge

### Requirement: Native Permission Dialog
The notification bridge SHALL present a native Adwaita permission dialog for user consent.

#### Scenario: Show permission dialog
**GIVEN** a first-time notification permission request
**WHEN** show_notification_permission_dialog() is called
**THEN** an Adw.AlertDialog is created
**AND** the dialog has title "WhatsApp Web Notification Permission"
**AND** the dialog has body text explaining the permission
**AND** the dialog has "Deny" and "Allow" responses
**AND** "Allow" is the suggested response
**AND** "Allow" is the default response
**AND** the dialog is presented on the parent window

#### Scenario: User grants permission via dialog
**GIVEN** the permission dialog is shown
**WHEN** user clicks "Allow"
**THEN** GSettings web-notification-permission-asked is set to true
**AND** GSettings web-notification-permission-granted is set to true
**AND** the WebKit permission request is allowed
**AND** notification handler is set up
**AND** an info log message is recorded

#### Scenario: User denies permission via dialog
**GIVEN** the permission dialog is shown
**WHEN** user clicks "Deny"
**THEN** GSettings web-notification-permission-asked is set to true
**AND** GSettings web-notification-permission-granted is set to false
**AND** the WebKit permission request is denied
**AND** an info log message is recorded

#### Scenario: User closes dialog without responding
**GIVEN** the permission dialog is shown
**WHEN** user closes the dialog (ESC key or close button)
**THEN** the "deny" response is triggered (close response)
**AND** permission is denied

### Requirement: WebKit to Native Notification Bridging
The notification bridge SHALL convert WebKit notifications to native desktop notifications.

#### Scenario: Setup notification handler
**GIVEN** notification permission is granted
**WHEN** setup_notification_handler() is called
**THEN** the manager connects to WebView show_notification signal
**AND** an info log message is recorded

#### Scenario: WebKit notification received
**GIVEN** notification handler is set up
**WHEN** WebKit emits show_notification with a notification
**THEN** the notification title and body are extracted
**AND** NotificationManager.send_notification() is called with title, body, and icon
**AND** the WebKit notification is connected for click/close events
**AND** the WebKit notification is closed
**AND** the handler returns true (handled)

#### Scenario: Notification click handling
**GIVEN** a native notification is displayed from WebKit
**WHEN** the notification is clicked
**THEN** the parent window is presented (focused)
**AND** the WebKit notification clicked event is triggered
**AND** a debug log message is recorded

#### Scenario: Notification close handling
**GIVEN** a native notification is displayed from WebKit
**WHEN** the notification is closed
**THEN** the WebKit notification closed event is triggered
**AND** a debug log message is recorded

#### Scenario: Notification without body
**GIVEN** WebKit emits a notification with only a title
**WHEN** on_webkit_notification() is called
**THEN** the body defaults to empty string
**AND** the notification is sent with title and empty body

### Requirement: Settings Persistence
The notification bridge SHALL persist permission decisions to GSettings.

#### Scenario: Save permission grant decision
**GIVEN** user grants permission via dialog
**WHEN** the dialog response handler runs
**THEN** GSettings web-notification-permission-asked is set to true
**AND** GSettings web-notification-permission-granted is set to true

#### Scenario: Save permission deny decision
**GIVEN** user denies permission via dialog
**WHEN** the dialog response handler runs
**THEN** GSettings web-notification-permission-asked is set to true
**AND** GSettings web-notification-permission-granted is set to false

#### Scenario: Settings unavailable handling
**GIVEN** Settings reference is null
**WHEN** permission request is received
**THEN** a permission dialog is shown (no saved state to check)
**AND** permission decision is not persisted
**AND** warnings are logged

### Requirement: Error Handling and Logging
The notification bridge SHALL handle errors gracefully and provide debug logging.

#### Scenario: Null NotificationManager handling
**GIVEN** NotificationManager reference is null
**WHEN** a WebKit notification is received
**THEN** a warning is logged
**AND** no native notification is sent

#### Scenario: Permission request logging
**GIVEN** a notification permission request is received
**WHEN** the request is processed
**THEN** an info log message records "WhatsApp requesting notification permission"

#### Scenario: Saved permission decision logging
**GIVEN** a saved permission decision exists
**WHEN** the decision is applied
**THEN** an info log message records "Using saved permission: granted" or "denied"

#### Scenario: WebKit notification logging
**GIVEN** a WebKit notification is received
**WHEN** on_webkit_notification() is called
**THEN** an info log message records "WebKit notification received: {title}"
