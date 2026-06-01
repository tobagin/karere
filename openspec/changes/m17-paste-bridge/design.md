## Context

CEF's offscreen browser receives keyboard input through host-driven `send_key_event` calls (M3); once forwarded, Chromium handles Ctrl+V internally by issuing a synthetic paste against its own clipboard model — which is decoupled from GDK. Karere v3 worked around this on WebKit by reading `gdk::Clipboard` itself and synthesizing a `paste` event on `document.activeElement`. The CEF port must do the same, but the interception point shifts: we must catch Ctrl+V at the `EventControllerKey` callback BEFORE forwarding to CEF.

The M13 IPC envelope already defines `BrowserMessage::DispatchPasteEvent { mime, payload: PasteBlob }` with `PasteBlob::{ Base64(String), FilePath(PathBuf) }`. This milestone is the consumer.

## Goals / Non-Goals

**Goals:**
- Ctrl+V with an image or file on the GDK clipboard pastes into the focused page element as a synthesized `paste` event carrying a `DataTransfer` with `File` entries.
- Drag-drop of files from a file manager onto the karere window surfaces them as a `drop` event on the element under the cursor.
- Middle-click in any focused element pastes primary-clipboard text while preserving middle-click-to-open-link behavior (the click is not swallowed).
- Large payloads (>1 MB) round-trip via a tempfile under `$XDG_RUNTIME_DIR/karere/` rather than base64-over-IPC.

**Non-Goals:**
- Rich-text / HTML paste. Text falls through to CEF's native Ctrl+V handler; HTML is downgraded to plain text.
- Animated-GIF preservation as more than an `image/gif` Blob — Chromium will inspect frames itself if the site cares.
- Outbound copy (page → GDK clipboard). M17 is one-way IN.
- Subframe paste targeting. Only the main frame is wired.

## Decisions

### Decision: Intercept Ctrl+V at the EventControllerKey BEFORE `send_key`
- **Choice**: The existing `connect_key_pressed` closure (M3) gets a guard: if `state.contains(CONTROL_MASK)` and `keyval == Key::v`, read the GDK clipboard. If image, file, **or text** content is present, dispatch via IPC and return `Propagation::Stop`. Only an empty clipboard falls through (`Propagation::Proceed`).
- **Correction (implementation)**: The original design let text-only Ctrl+V fall through to CEF's native handler. Testing showed CEF's offscreen/windowless clipboard does not consult GDK, so native text paste was a no-op. Text is therefore intercepted and synthesized through the same `DispatchPasteEvent` envelope (`mime: "text/plain"`, base64 payload) as image/file content — `paste_bridge.js` already had the text/plain branch (shared with middle-click).
- **Why**: CEF's offscreen mode does not consult GDK. The only place where the host owns the keystroke before it is committed to the renderer is the GTK controller callback. Returning `Stop` is what prevents the duplicate paste.
- **Alternatives**: (a) Override CEF's `RequestContextHandler::on_request_context_initialized` to swap clipboard provider — not supported in cef-rs. (b) Always synthesize, never let CEF paste — loses native text fast-path and breaks paste inside DevTools where we do not control the surface.

### Decision: DataTransfer synthesis in `paste_bridge.js`, dispatched from a `karere:dispatch-paste` CustomEvent
- **Choice**: `render_process.rs` receives `DispatchPasteEvent` and runs `window.dispatchEvent(new CustomEvent('karere:dispatch-paste', {detail: <json>}))`. `paste_bridge.js` listens, builds a `DataTransfer`, populates with `File` (via `fetch(file://...)` for `FilePath` blobs, or base64→`Blob` for `Base64` blobs), and dispatches a `paste` or `drop` event on `document.activeElement` (or the element under the cursor for drops).
- **Why**: Keeps Rust side mime-agnostic — it just ferries a blob. JS side knows DOM event surfaces. Identical to the karere-webkit pattern, easing future cross-checks.
- **Alternatives**: Build the `File` in Rust via `CefBinaryValue` and pipe directly — cef-rs does not expose `File` construction from the renderer side without round-tripping through JS anyway.

### Decision: Tempfile path for payloads >1 MB, scoped via `on_before_resource_load`
- **Choice**: When the encoded payload would exceed 1 MB after base64, write the raw bytes to `$XDG_RUNTIME_DIR/karere/paste-<uuid>` with mode `0600`. Pass `PasteBlob::FilePath(path)`. The renderer's `paste_bridge.js` does `await fetch("file://" + path)`, which requires `--allow-file-access-from-files`. To avoid broadening file:// reach to arbitrary user files, `RequestHandler::on_before_resource_load` rejects any `file://` request whose path is not a child of `$XDG_RUNTIME_DIR/karere/`. After the renderer signals consumption (or on a 30 s fallback timer), the tempfile is unlinked.
- **Why**: Base64 over `CefProcessMessage` for a 5 MB PNG would be ~6.7 MB and stress the message bus. A capability-scoped tempfile is the cheapest escape valve.
- **Alternatives**: (a) Chunked base64 across multiple `CefProcessMessage` envelopes — adds reassembly complexity and ordering guarantees we do not get for free. (b) Use `CefBinaryValue` directly — bypasses the JSON envelope from M13 and forces a second encoding rule.

### Decision: Drag-drop reuses `DispatchPasteEvent` envelope (but dispatches `drop` in JS)
- **Choice**: `gtk::DropTarget` with `gdk::FileList::static_type()`; `connect_drop` reads each file's content (or its path for the >1 MB path) and sends `DispatchPasteEvent`. The renderer-side `paste_bridge.js` distinguishes paste-vs-drop via a `kind` field in the detail (`"paste"` or `"drop"`).
- **Why**: Same MIME-typed blobs, same `DataTransfer` shape; only the DOM event name and target element differ. Sharing the envelope avoids a second IPC variant.
- **Alternatives**: Introduce `BrowserMessage::DispatchDropEvent` — duplicative; rejected.

### Decision: Middle-click does NOT swallow the click
- **Choice**: The middle-button `GestureClick::connect_pressed` reads `gdk::Display::primary_clipboard()` text and sends `DispatchPasteEvent { mime: "text/plain" }`, but RETURNS without claiming the gesture — Chromium still gets the middle button down/up.
- **Why**: Middle-click on a link opens it in the system browser (delegated to M7's URL handler). Swallowing the click for paste would break that.
- **Trade-off**: A middle-click in a text input fires both our synthesized paste AND any Chromium-native middle-click action. Empirically Chromium does nothing on middle-click in inputs, so the net effect is a single paste. Accepted.

### Decision: Mime-typed `paste_bridge.js` distinguishes image vs file vs text
- **Choice**: The JS handler branches on `detail.mime`:
  - `image/*`: builds a single `File` with the image extension inferred from MIME (`paste.png`, etc.).
  - `application/*` or `text/*` with `kind === "drop"`: builds files preserving the original filename if provided.
  - `text/plain` with `kind === "paste"` (middle-click only): dispatches a `paste` event whose `clipboardData.getData('text/plain')` returns the string; no `File`.
- **Why**: Mirrors how Chromium's native paste populates `DataTransfer` for each path.

## Risks / Trade-offs

- **Risk**: `--allow-file-access-from-files` widens renderer file:// reach. → Mitigated by `on_before_resource_load` scoping to `$XDG_RUNTIME_DIR/karere/` exactly; any other file:// URL is denied. Audited by the spec scenario.
- **Risk**: Tempfile leak if the renderer crashes between fetch and consume-ack. → Mitigated by a 30 s fallback unlink timer on the browser side, plus a startup sweep of `$XDG_RUNTIME_DIR/karere/paste-*` older than 1 hour.
- **Risk**: A page with no focused element drops the paste silently. → Acceptable; matches karere v3 behavior. Optionally log a `ConsoleLog` warning.
- **Risk**: DropTarget intercepts file drops the page itself wanted (e.g., a built-in drop zone). → WhatsApp's drop zone is what we are targeting; we dispatch a real `drop` event, so the page's listener still runs. Accepted.
- **Trade-off**: Ctrl+V handling diverges between text (CEF-native) and image/file (synthetic). Mental model is slightly more complex but matches what Chromium itself does (clipboard providers are mime-typed).
- **Risk**: Base64 1 MB cutoff may be too aggressive for screenshot-heavy users on slow disks. → Tunable; defer to perf testing.
