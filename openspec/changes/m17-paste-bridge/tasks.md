## 1. Ctrl+V interception

- [ ] 1.1 In `src/cef_gtk_area.rs::install_input_controllers`, extend the existing `EventControllerKey::connect_key_pressed` callback: guard on `state.contains(ModifierType::CONTROL_MASK) && keyval == gdk::Key::v`
- [ ] 1.2 Read `gtk::gdk::Clipboard::content()` via `Clipboard::read_value_async` (or `read_async` per content type) on the default display
- [ ] 1.3 Detect MIME of clipboard content; branch on `image/*`, `gdk::FileList`, or text-only
- [ ] 1.4 For image/file content, build `BrowserMessage::DispatchPasteEvent { mime, kind: "paste", payload }` and send via M13 IPC
- [ ] 1.5 Return `Propagation::Stop` for image/file content; return `Propagation::Proceed` for text-only

## 2. Large-payload tempfile path

- [ ] 2.1 Add helper `write_paste_tempfile(bytes: &[u8]) -> std::io::Result<PathBuf>` that writes to `$XDG_RUNTIME_DIR/karere/paste-<uuid>` with mode `0600`
- [ ] 2.2 Ensure `$XDG_RUNTIME_DIR/karere/` exists with mode `0700` on first use
- [ ] 2.3 Choose envelope: if `base64(bytes).len() > 1_048_576`, use `PasteBlob::FilePath`; else `PasteBlob::Base64`
- [ ] 2.4 Track pending tempfiles in `SharedState`; unlink on renderer-side ack or after a 30 s `glib::timeout_add` fallback
- [ ] 2.5 On startup (in `App::on_before_command_line_processing` or a dedicated init), sweep `$XDG_RUNTIME_DIR/karere/paste-*` older than 1 hour
- [ ] 2.6 In `src/cef_runtime.rs`, append `--allow-file-access-from-files` to the CEF command-line switches
- [ ] 2.7 In `src/handlers/request.rs::on_before_resource_load`, reject any `file://` URL whose path is not a child of `$XDG_RUNTIME_DIR/karere/`

## 3. Drag-drop

- [ ] 3.1 In `install_input_controllers`, create `gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY)`
- [ ] 3.2 Attach `connect_drop`: capture `(value, x, y)`; for each file, read bytes (via `gio::File::load_contents_async`) and send `DispatchPasteEvent { kind: "drop" }` with original filename
- [ ] 3.3 Include the drop coordinates in the envelope detail so `paste_bridge.js` can `document.elementFromPoint(x, y)` to target the right element
- [ ] 3.4 Reuse tempfile path from §2 for files >1 MB
- [ ] 3.5 Add `widget.add_controller(drop_target)`

## 4. Middle-click primary-clipboard paste

- [ ] 4.1 Extend the existing middle-button `GestureClick` from M3 (button-2): add `connect_pressed`
- [ ] 4.2 In the handler, read `gtk::gdk::Display::default().unwrap().primary_clipboard().read_text_async`
- [ ] 4.3 If text is non-empty, send `DispatchPasteEvent { mime: "text/plain", kind: "paste", payload: Base64(b64(text.as_bytes())) }`
- [ ] 4.4 Do NOT call `gesture.set_state(EventSequenceState::Claimed)` — let the click propagate so CEF still gets middle button down/up

## 5. Render-process routing

- [ ] 5.1 In `src/handlers/render_process.rs::on_process_message_received`, add a `BrowserMessage::DispatchPasteEvent` arm
- [ ] 5.2 Serialize the envelope detail to JSON and invoke `frame.execute_java_script("window.dispatchEvent(new CustomEvent('karere:dispatch-paste', {detail: <json>}))", "karere://paste", 0)`
- [ ] 5.3 After the JS dispatch returns, send back a renderer→browser ack (e.g., `RendererMessage::PasteConsumed { tempfile_path: Option<PathBuf> }`) so the host can unlink the tempfile

## 6. paste_bridge.js

- [ ] 6.1 Add `data/js/paste_bridge.js` (picked up automatically by the M13 build-time bundler thanks to lexical ordering)
- [ ] 6.2 Listen for `window.addEventListener('karere:dispatch-paste', ...)`
- [ ] 6.3 Branch on `detail.kind`: `"paste"` targets `document.activeElement`; `"drop"` targets `document.elementFromPoint(detail.x, detail.y)`
- [ ] 6.4 Reconstruct payload: if `detail.payload.kind === "FilePath"`, `await fetch("file://" + detail.payload.path).then(r => r.blob())`; if `Base64`, decode to `Uint8Array` and wrap in `Blob`
- [ ] 6.5 Build a `File` with the original filename (drop) or `paste.<ext>` (paste) and the correct MIME
- [ ] 6.6 Construct a `DataTransfer`, `dataTransfer.items.add(file)`
- [ ] 6.7 For `paste`: dispatch `new ClipboardEvent('paste', { clipboardData: dataTransfer, bubbles: true, cancelable: true })`
- [ ] 6.8 For `drop`: dispatch `new DragEvent('drop', { dataTransfer, clientX: detail.x, clientY: detail.y, bubbles: true, cancelable: true })`
- [ ] 6.9 For middle-click text paste: bypass `File` construction; set `dataTransfer.setData('text/plain', text)` and dispatch `paste`
- [ ] 6.10 On dispatch completion, send `RendererMessage::PasteConsumed { tempfile_path }` so the host unlinks

## 7. Tempfile lifecycle

- [ ] 7.1 In browser process, maintain a `HashMap<PathBuf, Instant>` of pending tempfiles in `SharedState`
- [ ] 7.2 On `PasteConsumed`, remove from map and `std::fs::remove_file(path)`
- [ ] 7.3 Schedule a `glib::timeout_add_seconds(30, ...)` per tempfile that unlinks if still in the map

## 8. Verify

- [ ] 8.1 Take a screenshot (Spectacle / `gnome-screenshot --area`) so an image lands on the GDK clipboard; focus the WhatsApp chat input; press Ctrl+V → attachment preview shows the image
- [ ] 8.2 Drag a PDF from Files onto the karere window over the chat input → attachment preview shows the PDF
- [ ] 8.3 Highlight text in another window (selects to primary clipboard); middle-click in chat input → text appears in the input
- [ ] 8.4 Middle-click a link inside an open chat (with primary-clipboard text present) → link opens in system browser AND the link element is unaffected (no paste applied to it)
- [ ] 8.5 Paste a >5 MB PNG → preview appears; verify `$XDG_RUNTIME_DIR/karere/paste-*` exists during dispatch and is gone within 30 s
- [ ] 8.6 Confirm `fetch("file:///etc/passwd")` from DevTools console is rejected (network error) while `fetch("file://$XDG_RUNTIME_DIR/karere/paste-<uuid>")` succeeds during an active paste
- [ ] 8.7 Confirm text-only Ctrl+V still works (CEF native path) — copy text from a terminal, focus chat input, press Ctrl+V → text inserted
