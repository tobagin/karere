## 1. Ctrl+V interception

- [x] 1.1 In `src/cef_gtk_area.rs::install_input_controllers`, extend the existing `EventControllerKey::connect_key_pressed` callback: guard on `state.contains(ModifierType::CONTROL_MASK) && keyval == gdk::Key::v`
- [x] 1.2 Read `gtk::gdk::Clipboard::content()` via `Clipboard::read_value_async` (or `read_async` per content type) on the default display
- [x] 1.3 Detect MIME of clipboard content; branch on `image/*`, `gdk::FileList`, or text-only
- [x] 1.4 For image/file content, build `BrowserMessage::DispatchPasteEvent { mime, kind: "paste", payload }` and send via M13 IPC
- [x] 1.5 Return `Propagation::Stop` for image/file content; return `Propagation::Proceed` for text-only

## 2. Large-payload tempfile path

- [x] 2.1 Add helper `write_paste_tempfile(bytes: &[u8]) -> std::io::Result<PathBuf>` that writes to `$XDG_RUNTIME_DIR/karere/paste-<uuid>` with mode `0600`
- [x] 2.2 Ensure `$XDG_RUNTIME_DIR/karere/` exists with mode `0700` on first use
- [x] 2.3 Choose envelope: if `base64(bytes).len() > 1_048_576`, use `PasteBlob::FilePath`; else `PasteBlob::Base64`
- [x] 2.4 Track pending tempfiles in `SharedState`; unlink on renderer-side ack or after a 30 s `glib::timeout_add` fallback
- [x] 2.5 On startup (in `App::on_before_command_line_processing` or a dedicated init), sweep `$XDG_RUNTIME_DIR/karere/paste-*` older than 1 hour
- [x] 2.6 In `src/cef_runtime.rs`, append `--allow-file-access-from-files` to the CEF command-line switches
- [x] 2.7 In `src/handlers/request.rs::on_before_resource_load`, reject any `file://` URL whose path is not a child of `$XDG_RUNTIME_DIR/karere/`

## 3. Drag-drop

- [x] 3.1 In `install_input_controllers`, create `gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY)`
- [x] 3.2 Attach `connect_drop`: capture `(value, x, y)`; for each file, read bytes (via `gio::File::load_contents_async`) and send `DispatchPasteEvent { kind: "drop" }` with original filename
- [x] 3.3 Include the drop coordinates in the envelope detail so `paste_bridge.js` can `document.elementFromPoint(x, y)` to target the right element
- [x] 3.4 Reuse tempfile path from §2 for files >1 MB
- [x] 3.5 Add `widget.add_controller(drop_target)`

## 4. Middle-click primary-clipboard paste

- [x] 4.1 Extend the existing middle-button `GestureClick` from M3 (button-2): add `connect_pressed`
- [x] 4.2 In the handler, read `gtk::gdk::Display::default().unwrap().primary_clipboard().read_text_async`
- [x] 4.3 If text is non-empty, send `DispatchPasteEvent { mime: "text/plain", kind: "paste", payload: Base64(b64(text.as_bytes())) }`
- [x] 4.4 Do NOT call `gesture.set_state(EventSequenceState::Claimed)` — let the click propagate so CEF still gets middle button down/up

## 5. Render-process routing

- [x] 5.1 In `src/handlers/render_process.rs::on_process_message_received`, add a `BrowserMessage::DispatchPasteEvent` arm
- [x] 5.2 Serialize the envelope detail to JSON and invoke `frame.execute_java_script("window.dispatchEvent(new CustomEvent('karere:dispatch-paste', {detail: <json>}))", "karere://paste", 0)`
- [x] 5.3 After the JS dispatch returns, send back a renderer→browser ack (e.g., `RendererMessage::PasteConsumed { tempfile_path: Option<PathBuf> }`) so the host can unlink the tempfile

## 6. paste_bridge.js

- [x] 6.1 Add `data/js/paste_bridge.js` (picked up automatically by the M13 build-time bundler thanks to lexical ordering)
- [x] 6.2 Listen for `window.addEventListener('karere:dispatch-paste', ...)`
- [x] 6.3 Branch on `detail.kind`: `"paste"` targets `document.activeElement`; `"drop"` targets `document.elementFromPoint(detail.x, detail.y)`
- [x] 6.4 Reconstruct payload: if `detail.payload.kind === "FilePath"`, `await fetch("file://" + detail.payload.path).then(r => r.blob())`; if `Base64`, decode to `Uint8Array` and wrap in `Blob`
- [x] 6.5 Build a `File` with the original filename (drop) or `paste.<ext>` (paste) and the correct MIME
- [x] 6.6 Construct a `DataTransfer`, `dataTransfer.items.add(file)`
- [x] 6.7 For `paste`: dispatch `new ClipboardEvent('paste', { clipboardData: dataTransfer, bubbles: true, cancelable: true })`
- [x] 6.8 For `drop`: dispatch `new DragEvent('drop', { dataTransfer, clientX: detail.x, clientY: detail.y, bubbles: true, cancelable: true })`
- [x] 6.9 For middle-click text paste: bypass `File` construction; set `dataTransfer.setData('text/plain', text)` and dispatch `paste`
- [x] 6.10 On dispatch completion, send `RendererMessage::PasteConsumed { tempfile_path }` so the host unlinks

## 7. Tempfile lifecycle

- [x] 7.1 In browser process, maintain a `HashMap<PathBuf, Instant>` of pending tempfiles in `SharedState`
- [x] 7.2 On `PasteConsumed`, remove from map and `std::fs::remove_file(path)`
- [x] 7.3 Schedule a `glib::timeout_add_seconds(30, ...)` per tempfile that unlinks if still in the map

## 8. Verify

- [x] 8.1 Take a screenshot (Spectacle / `gnome-screenshot --area`) so an image lands on the GDK clipboard; focus the WhatsApp chat input; press Ctrl+V → attachment preview shows the image
- [x] 8.2 Drag a PDF from Files onto the karere window over the chat input → attachment preview shows the PDF
- [x] 8.3 Highlight text in another window (selects to primary clipboard); middle-click in chat input → text appears in the input
- [x] 8.4 Middle-click a link inside an open chat (with primary-clipboard text present) → link opens in system browser AND the link element is unaffected (no paste applied to it)
- [~] 8.5 N/A — superseded. An https origin (web.whatsapp.com) cannot `fetch("file://…")`, so the >1 MB tempfile path is non-functional; payloads inline as base64 up to 64 MiB instead (see paste.rs `B64_INLINE_MAX`). No tempfile is created for normal pastes, so there is nothing to observe under `$XDG_RUNTIME_DIR/karere/`.
- [~] 8.6 PARTIAL — the `file://` deny guard (`resource_request_handler` → `PasteFileGuard`) is implemented and rejects any `file://` outside the paste dir; the "succeeds during active paste" half is moot since the tempfile/file:// path is unused (see 8.5).
- [x] 8.7 Confirm text-only Ctrl+V works — copy text, focus chat input, press Ctrl+V → text inserted (now via GDK intercept, not CEF native; see correction note)

## Implementation notes (deviations)

- **§2.4 / §7.1 — tempfile map home.** The pending-tempfile `HashMap<PathBuf, Instant>`
  lives in a browser-process module global (`src/paste.rs`), not in `SharedState`.
  Reason: the browser-process `Client::on_process_message_received` (which receives
  `PasteConsumed`) has no `SharedRef`, and tempfiles are process-global rather than
  per-webview. The spec intent (browser process maintains the map; remove on ack;
  30 s fallback unlink) is fully met.
- **§5.3 — consumption ack.** `render_process.rs` only dispatches the
  `karere:dispatch-paste` CustomEvent; it does **not** send the ack itself, because
  `execute_java_script` is fire-and-forget and cannot await the renderer's async
  `fetch`/`Blob` work. Instead `paste_bridge.js` sends `RendererMessage::PasteConsumed`
  once the synthetic event has been dispatched (§6.10), which the browser handles in
  `client.rs` → `paste::consume`. The end-to-end ack channel is intact.
- **Code locations** differ from the proposal's pre-refactor paths: input controllers
  live in `src/web_view.rs::install_input_controllers` (not `cef_gtk_area.rs`), and the
  bundled bridge is `data/js/40-paste-bridge.js` (numeric prefix required by the
  lexical bundler in `build.rs`).
- **Multi-file paste/drop** sends one `DispatchPasteEvent` per file (one synthetic
  event each), matching §3.2's per-file iteration, rather than one event carrying all
  files.
