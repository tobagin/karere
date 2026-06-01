# permission-prompt-extended Specification

## Purpose
TBD - created by archiving change m11-permission-persistence. Update Purpose after archive.
## Requirements
### Requirement: Handle on_show_permission_prompt
The shell SHALL implement `PermissionHandler::on_show_permission_prompt(_browser, prompt_id, requesting_origin, requested_permissions, callback)` so notifications, geolocation, midi-sysex, and clipboard-read requests are surfaced via the same libadwaita dialog as media-access requests. The method MUST return `1` to indicate asynchronous handling and resolve the callback with `CEF_PERMISSION_RESULT_ACCEPT` or `CEF_PERMISSION_RESULT_DENY`.

#### Scenario: Notifications request triggers the dialog
- **WHEN** a page calls `Notification.requestPermission()`
- **THEN** CEF invokes `on_show_permission_prompt` with the notifications bit set
- **AND** the handler returns `1`
- **AND** the shell shows an `adw::AlertDialog` whose body reads "<origin> is requesting access to notifications."
- **AND** Allow resolves the callback with `CEF_PERMISSION_RESULT_ACCEPT`, Deny with `CEF_PERMISSION_RESULT_DENY`

#### Scenario: Geolocation request via show_permission_prompt
- **WHEN** a page calls `navigator.geolocation.getCurrentPosition`
- **THEN** the handler shows the dialog with body "<origin> is requesting access to location."
- **AND** the user choice is forwarded via `callback.cont` with the corresponding `CEF_PERMISSION_RESULT_*` value

### Requirement: Consult the store before prompting
The shell SHALL call `permissions_store::get(origin, requested_permissions)` at the top of both `on_request_media_access_permission` and `on_show_permission_prompt`. When the store returns `Decision::Allow` or `Decision::Deny` the handler MUST resolve the callback immediately and skip the dialog. When the store returns `Decision::AskAll` or `Decision::AskMixed` the handler MUST show the dialog.

#### Scenario: Stored Allow skips the dialog
- **WHEN** `get(origin, mask)` returns `Decision::Allow`
- **THEN** the handler calls `callback.cont(CEF_PERMISSION_RESULT_ACCEPT)` (media path uses `cont(requested_permissions)`; show-prompt path uses the documented Accept constant)
- **AND** no dialog is shown
- **AND** the method still returns `1` to satisfy CEF's async contract

#### Scenario: Stored Deny skips the dialog
- **WHEN** `get(origin, mask)` returns `Decision::Deny`
- **THEN** the handler calls `callback.cont(CEF_PERMISSION_RESULT_DENY)` (media path uses `cont(0)`)
- **AND** no dialog is shown

#### Scenario: AskMixed forces a fresh prompt
- **WHEN** `get(origin, mask)` returns `Decision::AskMixed`
- **THEN** the handler shows the full dialog covering every bit in the request, regardless of which bits were already stored
- **AND** the user's choice overwrites the previous stored values for those bits

### Requirement: Decisions are remembered automatically
The shell MUST persist the user's choice after the dialog resolves by calling `permissions_store::set(origin, mask, decision)` — every Allow/Deny is remembered (browser-style), with no opt-in checkbox. The next identical request for the same origin and mask MUST short-circuit without a dialog.

#### Scenario: Choice persists without a checkbox
- **WHEN** the user selects Allow
- **THEN** the handler calls `permissions_store::set(origin, mask, Decision::Allow)`
- **AND** the next identical request for the same origin and mask short-circuits without showing a dialog, including after a restart

### Requirement: Extended describe_permissions coverage
The shell SHALL extend `describe_permissions(mask)` so that every CEF permission bit surfaced by `on_show_permission_prompt` (notifications, geolocation, midi-sysex, clipboard-read) maps to a human label. Unknown bits MUST continue to render as "device access" so the dialog body is never empty.

#### Scenario: midi-sysex label
- **WHEN** the request mask includes the midi-sysex bit
- **THEN** `describe_permissions` includes "midi" (or equivalent label) in the joined output
- **AND** the dialog body is composed using that string

#### Scenario: Unknown bit fallback
- **WHEN** a future CEF release introduces a permission bit not yet labelled
- **THEN** `describe_permissions` returns "device access" for that bit
- **AND** Allow still records the bit with its raw integer value in the store

