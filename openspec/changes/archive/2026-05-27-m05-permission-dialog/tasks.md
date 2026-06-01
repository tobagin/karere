## 1. PermissionHandler skeleton

- [x] 1.1 Add `ShellPermissionHandler` as `#[derive(Clone, Default)] struct ShellPermissionHandler;` in `src/handlers/permission.rs`
- [x] 1.2 Wrap with `wrap_permission_handler!` to produce `ShellPermissionHandlerBuilder` and expose `ShellPermissionHandlerBuilder::build() -> PermissionHandler`

## 2. Async media-access prompt

- [x] 2.1 Implement `on_request_media_access_permission(&self, _browser, _frame, requesting_origin, requested_permissions, callback)` returning `1` to signal async handling
- [x] 2.2 Clone the `MediaAccessCallback` and dispatch onto `glib::MainContext::default().spawn_local` so the dialog runs after the C++ callback returns
- [x] 2.3 Inside the async block, build an `adw::AlertDialog` titled "Permission request" with body `"<origin> is requesting access to <perms>."`, responses Deny / Allow (Allow = `Suggested`), default + close response = Deny
- [x] 2.4 `await dialog.choose_future(active_window().as_ref())`; on `"allow"` call `cb.cont(requested_permissions)`, otherwise `cb.cont(0)`

## 3. Helpers

- [x] 3.1 Add `describe_permissions(mask: u32) -> String` walking `cef_permission_request_types_t` bits (`CAMERA_STREAM` -> "camera", `MIC_STREAM` -> "microphone", `GEOLOCATION` -> "location", `NOTIFICATIONS` -> "notifications", `CLIPBOARD` -> "clipboard"), joining with ", " and falling back to "device access"
- [x] 3.2 Add `active_window() -> Option<gtk::Window>` via `gio::Application::default()?.downcast::<gtk::Application>().ok()?.active_window()`

## 4. Wire into ClientBuilder

- [x] 4.1 Add `permission_handler: PermissionHandler` field to the `wrap_client!` struct in `src/handlers/client.rs`
- [x] 4.2 Implement `Client::permission_handler(&self) -> Option<cef::PermissionHandler>` returning `Some(self.permission_handler.clone())`
- [x] 4.3 Pass `ShellPermissionHandlerBuilder::build()` as the new argument in `ClientBuilder::build_for`

## 5. Smoke test

- [x] 5.1 Load `https://webrtc.github.io/samples/src/content/getusermedia/gum/` and click "Open camera"; confirm the dialog shows the page origin and "camera" (or "camera, microphone" when audio is also requested)
- [x] 5.2 Confirm Allow grants the stream (video element renders) and Deny surfaces a `NotAllowedError` in the page
