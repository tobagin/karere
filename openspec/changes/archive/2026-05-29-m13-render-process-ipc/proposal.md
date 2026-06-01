## Why

Most karere v3 features rely on injected JS that talks back to the host. WebKit provided `UserContentManager::add_script` + `register_script_message_handler` for this. CEF replaces that model: scripts must be injected by a `RenderProcessHandler` running in the renderer subprocess, and host communication uses `CefProcessMessage` over the bidirectional browser↔renderer channel. Without this foundation, downstream milestones (M14 mobile responsive, M16 store hook, M17 paste bridge, M20/M21 notification observer) cannot land.

## What Changes

- Add a `RenderProcessHandler` (`src/handlers/render_process.rs`) that injects a build-time-concatenated JS bundle into the main frame on `on_context_created` and parses inbound `CefProcessMessage` payloads on `on_process_message_received`.
- Extend the `App` trait implementation to return the render-process handler (App runs in both browser and renderer processes; `RenderProcessHandler` is supplied via App, not Client).
- Add a typed IPC envelope (`src/ipc.rs`) defining `BrowserMessage` (browser → renderer: `DispatchPasteEvent`, `SetViewportSize`, `CloseNotifByTag`, debug `Ping`) and `RendererMessage` (renderer → browser: `ProfileIdentity`, `ProfileAvatar`, `AwaitingPairing`, `StoreUnavailable`, `NotificationSeen`, `NotificationClosed`, `ConsoleLog`, debug `Pong`) with single-string base64-JSON payload encoding to/from `CefProcessMessage`.
- Add `data/js/bootstrap.js`: always-on hooks for `console.log/warn/error` forwarding plus listeners for `karere:dispatch-paste` and `karere:close-notif` DOM events.
- Add `build.rs` step that scans `data/js/*.js`, concatenates into `$OUT_DIR/injected_bundle.js`, and exposes it for `include_str!`.
- Provide a debug-only `Ping`/`Pong` roundtrip for verification.

## Capabilities

### New Capabilities
- `cef-render-process`: Renderer-subprocess handler that injects the JS bundle into the main frame on context creation and dispatches inbound process messages.
- `cef-process-ipc`: Typed `BrowserMessage`/`RendererMessage` envelope with `CefProcessMessage` (de)serialization, including the debug `Ping`/`Pong` pair.
- `js-injection-pipeline`: Build-time bundling of `data/js/*.js` into a single embedded string plus a runtime bootstrap script that bridges DOM events and console output to the host.

### Modified Capabilities
<!-- None: M13 is purely additive. -->

## Impact

- New files: `src/handlers/render_process.rs`, `src/ipc.rs`, `data/js/bootstrap.js`, `build.rs` (or extension thereof).
- Modified files: `src/handlers/app.rs` (or `cef_runtime.rs`) to wire `App::render_process_handler`.
- Build: `build.rs` introduces a JS bundling step; `OUT_DIR/injected_bundle.js` must be generated before compile.
- Runtime: every browser created will have JS injected into its main frame on each context creation; console output from any loaded page is forwarded to the host process.
- Unblocks: M14, M16, M17, M20, M21.
- Risk: `CefRegisterExtension`/`cefQuery` API surface in cef-rs is unverified; design includes a `send_process_message` fallback if `RegisterExtension` is unavailable.
