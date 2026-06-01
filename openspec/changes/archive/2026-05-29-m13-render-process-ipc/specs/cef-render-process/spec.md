## ADDED Requirements

### Requirement: Render-process handler registration
The application SHALL provide a `RenderProcessHandler` returned from the `App::render_process_handler` trait method so that CEF invokes it inside the renderer subprocess.

#### Scenario: Handler is returned in renderer process
- **WHEN** CEF starts a renderer subprocess and invokes `App::render_process_handler`
- **THEN** the application returns `Some(handler)` constructed via `ShellRenderProcessHandlerBuilder`
- **AND** the handler is wrapped using the `wrap_render_process_handler!` macro provided by cef-rs

#### Scenario: Handler is not required in the browser process
- **WHEN** CEF invokes `App::render_process_handler` in the browser process
- **THEN** the application MAY return `None` without affecting browser-process functionality

### Requirement: Main-frame JS injection on context creation
The render-process handler SHALL inject the build-time-bundled JS into the main frame when its V8 context is created.

#### Scenario: Main frame context created
- **WHEN** `on_context_created` fires for a frame and `frame.is_main()` returns true
- **THEN** the handler calls `frame.execute_java_script(EMBED_BUNDLE, "karere://bootstrap", 0)` where `EMBED_BUNDLE` is the embedded concatenated bundle

#### Scenario: Subframe context created
- **WHEN** `on_context_created` fires for a frame and `frame.is_main()` returns false
- **THEN** the handler MUST NOT inject the bundle

### Requirement: Inbound process message dispatch
The render-process handler SHALL parse inbound `CefProcessMessage` payloads through the typed envelope and dispatch them to per-variant handlers.

#### Scenario: Known variant received
- **WHEN** `on_process_message_received` fires with a `CefProcessMessage` whose name matches a `BrowserMessage` variant
- **THEN** the handler parses the payload via `BrowserMessage::try_from_cef_message`
- **AND** routes to the matching dispatcher (e.g., `DispatchPasteEvent`, `SetViewportSize`, `CloseNotifByTag`, `Ping`)

#### Scenario: Unknown variant received
- **WHEN** `on_process_message_received` fires with a name that does not match any `BrowserMessage` variant
- **THEN** the handler logs an `unknown message` warning including the received name and returns `false`

#### Scenario: Malformed payload
- **WHEN** the message name is known but the base64-JSON payload fails to decode or deserialize
- **THEN** the handler logs the parse error and returns `false` without crashing the renderer
