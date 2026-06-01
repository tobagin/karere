## Context

Karere v3 ports a WebKit-based GTK shell to CEF. Under WebKit, host↔page communication used `UserContentManager::add_script` (build-time injection) and `register_script_message_handler` (`window.webkit.messageHandlers.<name>.postMessage`) for the reverse direction. CEF has no equivalent on the browser-process side: scripts must be injected from the renderer subprocess via a `RenderProcessHandler::on_context_created` hook, and host messaging is performed through `CefProcessMessage` over the bidirectional process channel (`Frame::send_process_message` / `RenderProcessHandler::on_process_message_received`).

The renderer subprocess is a separate OS process from the browser process; both run the same binary. `cef-rs` exposes the `App` trait in both, and `App::render_process_handler()` is only invoked in the renderer process. Most other handlers (`Client`, `LifeSpanHandler`, etc.) live in the browser process. This milestone is the structural prerequisite for every feature that needs to observe DOM state or alter page behavior.

## Goals / Non-Goals

**Goals:**
- Inject a single concatenated JS bundle into the main frame on every main-frame `on_context_created` event.
- Define a typed, versionable IPC envelope (`BrowserMessage`, `RendererMessage`) with a single canonical encoding path to/from `CefProcessMessage`.
- Forward renderer console output to the browser process for unified logging.
- Provide a debug `Ping`/`Pong` roundtrip so the channel is end-to-end verifiable before downstream milestones depend on it.
- Bundle JS at build time so the shipped binary is self-contained (no `data/js/` lookup at runtime).

**Non-Goals:**
- Feature scripts (mobile responsive, paste bridge, notification observer, store hook). Those land in M14/M17/M20/M21 and only add files under `data/js/` plus new message variants — the envelope from M13 is sufficient.
- Subframe injection. Only the main frame receives the bundle.
- Per-browser script selection. The bundle is global; feature scripts gate themselves on `location.host`.

## Decisions

### Decision: Bundle JS at build time via `build.rs`
- **Choice**: `build.rs` enumerates `data/js/*.js` in lexical order, concatenates them into `$OUT_DIR/injected_bundle.js`, and a `const EMBED_BUNDLE: &str = include_str!(concat!(env!("OUT_DIR"), "/injected_bundle.js"));` exposes it to Rust.
- **Why**: Avoids runtime file lookup, which would fail under flatpak/relocated installs. Lexical order gives deterministic dependency: `00-bootstrap.js` runs before features.
- **Alternatives**: (a) `include_str!` each file individually — works but couples Rust source to file list. (b) Read from disk at startup — fragile across install layouts.

### Decision: Single-string base64-JSON payload over `CefProcessMessage`
- **Choice**: `to_cef_message(name, args_json)` produces a `CefProcessMessage` whose name is the variant tag (e.g., `"DispatchPasteEvent"`) and whose single string argument is `base64(json(payload))`. Inverse parser decodes and `serde_json::from_str` into the typed enum.
- **Why**: `CefListValue` arg lists are awkward to map from `serde`. Base64 sidesteps any UTF-8 / null-byte issues with binary payloads like `PasteBlob::Base64`. One encoding rule covers every variant.
- **Trade-off**: ~33% size overhead vs raw binary. Acceptable; messages are small (<100KB worst case for a clipboard image).

### Decision: native bridge bound in `on_context_created` (NOT `register_extension`)
- **Choice (implemented)**: Bind a native `karere_send(name, json)` V8 function onto the context global inside `RenderProcessHandler::on_context_created` (via `v8_value_create_function` + `V8Value::set_value_bykey`), then inject the bundle. Page JS reaches it as `window.karere_send`, with `window.karere.send` / `window.cefQuery` aliases set by the bootstrap.
- **Why**: The originally-planned `register_extension` path (in `on_web_kit_initialized`) is exposed by cef-rs, but using it **broke page rendering** — its JS runs in every V8 context *before* `on_context_created`, and it aborted WhatsApp's own context setup (blank SPA; our `on_context_created` never fired). Binding *after* the context exists cannot break context creation.
- **Trade-off**: The bridge is bound per-context instead of once globally — negligible cost (single main-frame context per page). The IPC envelope is unchanged.
- **Superseded**: the `register_extension` / `send_process_message`-fallback plan below. The `send_process_message` mechanism is still used (the native handler calls `Frame::send_process_message`); only the *registration* mechanism changed.

### Decision: `App` returns `RenderProcessHandler`, not `Client`
- **Choice**: Wire the handler at `App::render_process_handler`. `Client` (browser process) remains untouched.
- **Why**: cef-rs and upstream CEF both invoke `render_process_handler` only in the renderer subprocess. Putting it on `Client` would silently never fire.

### Decision: Debug-only `Ping`/`Pong`
- **Choice**: Compile `Ping`/`Pong` variants behind `cfg(debug_assertions)` (or a `debug-ipc` feature flag). Release builds omit them.
- **Why**: Keeps the production envelope minimal but gives the verification step a real channel exercise.

## Risks / Trade-offs

- **Resolved**: cef-rs `register_extension` *is* exposed but broke page rendering; the bridge is bound in `on_context_created` instead (see Decision above).
- **Risk**: Renderer crash if `on_context_created` injection throws. → Mitigation: wrap injected bundle in `try { ... } catch(e) { /* report via ConsoleLog */ }`.
- **Risk**: `console.log` flooding swamps the browser process. → Mitigation: bootstrap script truncates messages >4KB and drops bursts >100/sec (deferred to M13.1 if hit; not required for landing).
- **Risk**: Base64 overhead for large paste blobs (images up to ~10MB). → Trade-off accepted; revisit if M17 perf testing flags it. Could swap to `CefBinaryValue` later without changing the envelope's type surface.
- **Risk**: Lexical-order bundling makes implicit dependencies between scripts brittle. → Mitigation: enforce `NN-name.js` prefix convention in `build.rs` (warn if missing).
- **Trade-off**: Process messages are async fire-and-forget. No reply correlation built in. → Acceptable for v1; if needed, add a `request_id` field in a future minor revision of the envelope.
