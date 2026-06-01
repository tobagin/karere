# permission-prompt Specification

## Purpose

Defines the user-facing approval flow for page-initiated CEF media-access
permission requests (camera / microphone / clipboard / notifications /
location), rendered as a libadwaita modal dialog so pages calling
`getUserMedia` (and similar APIs) no longer fail silently with
`NotAllowedError`.

## Requirements

### Requirement: Permission prompt for page-initiated media access
The shell SHALL display a libadwaita modal dialog whenever CEF invokes `on_request_media_access_permission`, and MUST resolve the `MediaAccessCallback` asynchronously with the user's choice: the full requested bitmask on Allow, or `0` on Deny.

#### Scenario: Camera request from a page
- **WHEN** a loaded page calls `navigator.mediaDevices.getUserMedia({ video: true })`
- **THEN** CEF invokes the shell's `PermissionHandler::on_request_media_access_permission`
- **AND** the handler returns `1` to indicate an asynchronous response
- **AND** an `adw::AlertDialog` titled "Permission request" appears with body text "<origin> is requesting access to camera." and Deny / Allow responses (Allow styled `Suggested`, default and close response = Deny)
- **AND** when the user selects Allow the handler calls `MediaAccessCallback::cont(requested_permissions)` so the page receives the requested `MediaStream`

#### Scenario: User denies a microphone request
- **WHEN** a page requests microphone access and the user picks Deny (or dismisses the dialog)
- **THEN** the handler calls `MediaAccessCallback::cont(0)`
- **AND** the page observes a `NotAllowedError` from the failing `getUserMedia` promise

#### Scenario: Combined camera + microphone request
- **WHEN** a page requests `{ video: true, audio: true }` in a single call
- **THEN** the dialog body reads "<origin> is requesting access to camera, microphone."
- **AND** Allow grants the full combined bitmask (camera and microphone) in one `cont` call

#### Scenario: Unknown permission bits
- **WHEN** CEF requests a permission bit not recognised by `describe_permissions`
- **THEN** the dialog body falls back to "<origin> is requesting access to device access."
- **AND** Allow still passes the full original `requested_permissions` mask to `MediaAccessCallback::cont`

### Requirement: PermissionHandler wired into Client
The shell's `Client` SHALL expose a `PermissionHandler` so CEF routes media-access prompts to `ShellPermissionHandler` instead of using the default deny-all behaviour.

#### Scenario: Client returns the permission handler
- **WHEN** CEF calls `Client::permission_handler` on the shell's client
- **THEN** the method returns `Some(handler)` referencing the `ShellPermissionHandlerBuilder`-built handler that was constructed in `ClientBuilder::build_for`
