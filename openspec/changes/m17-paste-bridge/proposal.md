## Why

Karere v3 (`window.rs:1640-1891`) reads the GDK clipboard on Ctrl+V, builds a synthetic `DataTransfer`, and dispatches a `paste` event on `document.activeElement`. WebKit had no native shortcut handler beyond `key_controller`; CEF eats keyboard input inside the browser, so we must intercept BEFORE the key reaches CEF. The same plumbing also covers file drag-drop and middle-click primary-clipboard paste — three closely related GDK→page transfer paths that all use the M13 `DispatchPasteEvent` envelope.

## What Changes

- Extend `src/cef_gtk_area.rs::install_input_controllers`:
  - In the existing `EventControllerKey::connect_key_pressed` callback (from M3), intercept Ctrl+V BEFORE forwarding to `send_key`. Read `gtk::gdk::Clipboard::content()`; if image or file content present, async-read, build `BrowserMessage::DispatchPasteEvent { mime, payload }`, send via M13 IPC, and return `Propagation::Stop`. Text-only payloads fall through to CEF's built-in handler.
  - Add a `gtk::DropTarget` with `gdk::FileList::static_type()`. On `connect_drop`, send the same `DispatchPasteEvent` envelope (renderer reuses `paste_bridge.js` to dispatch a `drop` event on the element under the cursor).
  - Extend the existing middle-button `GestureClick` (M3): on `connect_pressed`, read `gtk::gdk::Display::default().primary_clipboard()` text; if any, send `DispatchPasteEvent { mime: "text/plain", payload: Base64(...) }`. Do NOT swallow the click so Chromium still receives middle-button (preserving middle-click-to-open-link behavior).
- New `data/js/paste_bridge.js`: listens for `karere:dispatch-paste` custom events, constructs `DataTransfer` populated with `File` objects via `fetch("file://...")` or base64→Blob, then dispatches a `paste` (or `drop` for drag events) on `document.activeElement`.
- Large-payload path: for payloads >1 MB, write to `$XDG_RUNTIME_DIR/karere/paste-<uuid>` (mode 0600) and pass `PasteBlob::FilePath(path)`. Add `--allow-file-access-from-files` to the cef_runtime command-line so the renderer's `fetch("file://...")` succeeds. Scope via `RequestHandler::on_before_resource_load` to allow only paths under `$XDG_RUNTIME_DIR/karere/`. Tempfile is removed after the renderer acknowledges (or on a 30 s timer).
- `src/handlers/render_process.rs` (existing from M13): route `BrowserMessage::DispatchPasteEvent` by `frame.execute_java_script("window.dispatchEvent(new CustomEvent('karere:dispatch-paste', {detail: <json>}))", "karere://paste", 0)`.

## Capabilities

### New Capabilities
- `clipboard-paste-bridge`: GDK→page paste pipeline that intercepts Ctrl+V before CEF, marshals image/file/text clipboard content over the M13 IPC envelope, and synthesizes a DOM `paste` event on `document.activeElement`. Includes a tempfile path for payloads >1 MB scoped under `$XDG_RUNTIME_DIR/karere/`.
- `drag-drop-files`: `gtk::DropTarget` wired to the same `DispatchPasteEvent` envelope so dragged files are surfaced to the page as a `drop` event on the element under the cursor.
- `primary-clipboard-paste`: Middle-click handler that reads `gdk::Display::primary_clipboard()` and dispatches the text as a `paste` event without swallowing the click (preserves middle-click link semantics).

### Modified Capabilities
<!-- None: M17 is purely additive on top of M3 input plumbing and M13 IPC. -->

## Impact

- New files: `data/js/paste_bridge.js`.
- Modified files: `src/cef_gtk_area.rs` (Ctrl+V intercept, DropTarget, middle-click extension), `src/handlers/render_process.rs` (route `DispatchPasteEvent`), `src/cef_runtime.rs` (add `--allow-file-access-from-files` switch), `src/handlers/request.rs` (scope file:// access to `$XDG_RUNTIME_DIR/karere/`).
- Runtime: clipboard reads are async; payloads >1 MB hit disk under `$XDG_RUNTIME_DIR/karere/paste-<uuid>` (mode 0600) and are cleaned up post-dispatch.
- Depends on: M3 (input controllers), M13 (IPC envelope, JS injection pipeline).
- Non-goals: animated GIF paste (treated as `image/gif` blob), rich-text/HTML paste (text-only fallback).
- Risk: `--allow-file-access-from-files` widens file:// access in the renderer; mitigated by `on_before_resource_load` scoping to the tempfile directory only.
