## Why

CEF defaults to denying media-access requests (camera/microphone/clipboard/notifications/location) when no `PermissionHandler` is registered, so pages calling `getUserMedia` or similar APIs silently fail with `NotAllowedError`. We need a user-facing prompt routed through libadwaita so the user can accept or deny the request per call.

## What Changes

- Add `ShellPermissionHandler` (empty `#[derive(Clone, Default)]` state) and its `ShellPermissionHandlerBuilder` via the `wrap_permission_handler!` macro in `src/handlers/permission.rs`.
- Implement `on_request_media_access_permission` so it returns `1` (asynchronous response) and trampolines through `glib::MainContext::default().spawn_local` to display an `adw::AlertDialog` whose body reads "<origin> is requesting access to <perms>." with Deny / Allow responses (Allow = `Suggested`, default + close = Deny). On Allow, call `MediaAccessCallback::cont(requested_permissions)`; on Deny, `cont(0)`.
- Add helper `describe_permissions(mask)` that maps the `cef_permission_request_types_t` bitmask to human strings (`camera`, `microphone`, `location`, `notifications`, `clipboard`) and falls back to `device access`.
- Add helper `active_window()` that pulls the active `gtk::Window` from `gio::Application::default()` downcast to `gtk::Application`.
- Extend `wrap_client!` in `src/handlers/client.rs` with a `permission_handler: PermissionHandler` field and return `Some(self.permission_handler.clone())` from `Client::permission_handler`; wire `ShellPermissionHandlerBuilder::build()` through `ClientBuilder::build_for`.

## Capabilities

### New Capabilities
- `permission-prompt`: user-facing approval flow for page-initiated CEF media-access permission requests (camera / microphone / clipboard / notifications / location), rendered as a libadwaita modal dialog.

### Modified Capabilities
<!-- none; no prior capability specs exist in openspec/specs/ -->

## Impact

- New file: `src/handlers/permission.rs`.
- Modified: `src/handlers/client.rs` (new field on `wrap_client!` struct, new getter, new constructor argument in `build_for`), `src/handlers/mod.rs` (module declaration).
- No new crate dependencies (`libadwaita`, `gtk`, `gio`, `glib`, `cef` already used elsewhere).
- No persistence layer, no settings schema, no IPC.
- Non-goals (deferred): per-origin allowlist or remembered decisions, `on_show_permission_prompt` (notifications / midi / geolocation Chromium-UI path) — handled by M11.
