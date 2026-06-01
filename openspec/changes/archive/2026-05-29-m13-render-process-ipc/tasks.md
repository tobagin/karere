## 1. Build pipeline for JS bundle

- [x] 1.1 Create `data/js/` directory and add a placeholder `00-bootstrap.js` so the directory is non-empty before `build.rs` runs.
- [x] 1.2 Write/extend `build.rs` to enumerate `data/js/*.js` in lexical order, concatenate them into `$OUT_DIR/injected_bundle.js`, and emit `cargo:rerun-if-changed=data/js` plus per-file `cargo:rerun-if-changed` entries.
- [x] 1.3 Add a `build.rs` warning when a file is missing the recommended `NN-name.js` prefix.
- [x] 1.4 Verify with `cargo build` that `$OUT_DIR/injected_bundle.js` exists and matches expectations.

## 2. IPC envelope

- [x] 2.1 Create `src/ipc.rs` with `BrowserMessage`, `RendererMessage`, and `PasteBlob` types (derive `Serialize`, `Deserialize`).
- [x] 2.2 Implement `to_cef_message(name: &str, args_json: String) -> CefProcessMessage` producing single-string base64-JSON payload.
- [x] 2.3 Implement `BrowserMessage::try_from_cef_message(&msg)` and `RendererMessage::try_from_cef_message(&msg)` returning `Result<Self, IpcError>`.
- [x] 2.4 Implement helpers `BrowserMessage::to_cef_message(&self)` and `RendererMessage::to_cef_message(&self)` that derive variant tag from the enum and call `to_cef_message`.
- [x] 2.5 Gate `Ping` / `Pong` variants behind `cfg(debug_assertions)` (or a `debug-ipc` feature flag) per design.
- [x] 2.6 Add unit tests covering encoding roundtrips for every variant, unknown-name rejection, and malformed-payload rejection.

## 3. Render-process handler

- [x] 3.1 Create `src/handlers/render_process.rs` with `ShellRenderProcessHandlerBuilder` using `wrap_render_process_handler!`.
- [x] 3.2 Define `const EMBED_BUNDLE: &str = include_str!(concat!(env!("OUT_DIR"), "/injected_bundle.js"));`.
- [x] 3.3 Implement `on_context_created`: when `frame.is_main()`, call `frame.execute_java_script(EMBED_BUNDLE, "karere://bootstrap", 0)`.
- [x] 3.4 Implement `on_process_message_received`: parse via `BrowserMessage::try_from_cef_message`, dispatch known variants (most are stubs that log; `Ping` replies with `Pong`), and log unknown-message warnings.
- [x] 3.5 Add a tracing/log helper so console output from `ConsoleLog` (received on the browser side) appears in the project's existing log facade — defer browser-side receiver wiring to step 4.2.

## 4. App trait wiring

- [x] 4.1 Create or extend `src/handlers/app.rs` (or `cef_runtime.rs`) to override `App::render_process_handler` returning a `ShellRenderProcessHandlerBuilder` instance.
- [x] 4.2 In the browser-process `Client::on_process_message_received` (or equivalent), parse incoming `RendererMessage` and route `ConsoleLog` to the host logger and `Pong` to a debug verifier.
- [x] 4.3 If `App` was previously a unit struct, ensure both browser and renderer entry points construct the same `App` instance per cef-rs idiom.

## 5. Bootstrap JS

- [x] 5.1 Implement `data/js/00-bootstrap.js`: wrap body in `try/catch`; on catch, fall back to the native console plus a `send_process_message`-equivalent error report.
- [x] 5.2 Replace `console.log/warn/error` with shims that forward each call (level + stringified message) to the browser process as `RendererMessage::ConsoleLog`, while still calling the original console method.
- [x] 5.3 Register `document` event listeners for `karere:dispatch-paste` and `karere:close-notif`. Listener bodies are no-op stubs in M13 (real handling lands in M17/M20).
- [x] 5.4 If `CefRegisterExtension` proves unavailable in cef-rs, add a small JS shim that defines `window.cefQuery(...)` and routes through the renderer-side dispatcher per the design's fallback.

## 6. Verification

- [x] 6.1 Load any page (e.g. `https://example.com`), open DevTools, run `console.log("hi")`, and confirm a `ConsoleLog { level: "log", msg: "hi" }` appears in the browser-process log.
- [x] 6.2 In a debug build, send `BrowserMessage::Ping` from a test harness and assert a `RendererMessage::Pong` arrives within 50 ms.
- [x] 6.3 Move the compiled binary to a temp directory away from the source tree, launch it, and confirm injection still works (bundle is embedded).
- [x] 6.4 Confirm `cargo build` triggers a rebuild after editing any file under `data/js/`.

## 7. Documentation

- [x] 7.1 Add a short module-level doc comment to `src/ipc.rs` describing the envelope contract, base64-JSON encoding, and how to add a new variant.
- [x] 7.2 Add a short module-level doc comment to `src/handlers/render_process.rs` noting that it runs in the renderer subprocess only.
