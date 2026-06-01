## ADDED Requirements

### Requirement: Ctrl+V intercepts GDK clipboard image and file content
The system SHALL intercept Ctrl+V at the GTK `EventControllerKey` callback before forwarding the key event to CEF, and translate non-text GDK clipboard content into a synthetic page paste.

#### Scenario: Image on GDK clipboard
- **WHEN** the user focuses an embedded page element and presses Ctrl+V
- **AND** `gtk::gdk::Clipboard::content()` reports an image MIME (e.g., `image/png`)
- **THEN** the host reads the image asynchronously
- **AND** sends `BrowserMessage::DispatchPasteEvent { mime: "image/png", payload: <PasteBlob> }` over the M13 IPC channel
- **AND** the `connect_key_pressed` callback returns `Propagation::Stop` so CEF does not also process the keystroke

#### Scenario: File list on GDK clipboard
- **WHEN** the user focuses an embedded page element and presses Ctrl+V
- **AND** `gtk::gdk::Clipboard::content()` reports `gdk::FileList`
- **THEN** the host iterates each file, builds one `DispatchPasteEvent` per file (or a single envelope with multiple blobs in implementation order), and sends it via M13 IPC
- **AND** the callback returns `Propagation::Stop`

#### Scenario: Text-only clipboard is read from GDK and synthesized
- **WHEN** the user presses Ctrl+V and the GDK clipboard contains only text
- **THEN** the host reads the GDK clipboard text asynchronously
- **AND** sends `BrowserMessage::DispatchPasteEvent { mime: "text/plain", kind: "paste", payload: PasteBlob::Base64(<b64>) }`
- **AND** the callback returns `Propagation::Stop`

> Note (implementation): CEF's offscreen (windowless) clipboard does not consult
> GDK, so its native Ctrl+V pastes nothing. The original design deferred text to
> CEF's built-in handler; that was found non-functional in OSR, so text is now
> intercepted and synthesized via the same bridge as image/file content. An
> empty clipboard still returns `Propagation::Proceed`.

### Requirement: Renderer dispatches synthetic paste event on focused element
The renderer-side `paste_bridge.js` SHALL listen for `karere:dispatch-paste` CustomEvents and dispatch a DOM `paste` event on `document.activeElement` whose `clipboardData` is a populated `DataTransfer`.

#### Scenario: Image blob arrives
- **WHEN** `paste_bridge.js` receives a `karere:dispatch-paste` event whose `detail.mime` starts with `image/` and `detail.kind === "paste"`
- **THEN** it builds a `File` with name `paste.<ext>` where `<ext>` is inferred from the MIME
- **AND** populates a new `DataTransfer` with that file
- **AND** dispatches a `paste` event with `clipboardData` set to the `DataTransfer` on `document.activeElement`

#### Scenario: No focused element
- **WHEN** `paste_bridge.js` receives the event and `document.activeElement` is `null` or `document.body`
- **THEN** the script logs a `ConsoleLog` warning via the M13 envelope
- **AND** does not dispatch a `paste` event

### Requirement: Large payloads round-trip via tempfile under XDG_RUNTIME_DIR
The host SHALL avoid base64-over-IPC for blobs whose base64-encoded size would exceed 1 MB and instead pass a tempfile path.

#### Scenario: 5 MB image paste
- **WHEN** the user pastes a 5 MB PNG
- **THEN** the host writes the raw bytes to `$XDG_RUNTIME_DIR/karere/paste-<uuid>` with permissions `0600`
- **AND** sends `BrowserMessage::DispatchPasteEvent { payload: PasteBlob::FilePath(path) }`
- **AND** the renderer fetches `file://<path>` to construct the `File`
- **AND** the host unlinks `<path>` after the renderer acknowledges consumption or after a 30 s fallback timer, whichever comes first

#### Scenario: Tempfile sweep on startup
- **WHEN** the host process starts
- **THEN** any file under `$XDG_RUNTIME_DIR/karere/` matching `paste-*` whose mtime is older than 1 hour SHALL be removed

### Requirement: File access scoped to paste tempfile directory
The system SHALL pass `--allow-file-access-from-files` to the CEF command line AND scope `file://` resource loads to `$XDG_RUNTIME_DIR/karere/` via `RequestHandler::on_before_resource_load`.

#### Scenario: Permitted tempfile fetch
- **WHEN** the renderer issues `fetch("file://<XDG_RUNTIME_DIR>/karere/paste-<uuid>")`
- **THEN** `on_before_resource_load` returns continue and the fetch succeeds

#### Scenario: Denied arbitrary file fetch
- **WHEN** the renderer issues `fetch("file:///etc/passwd")` or any `file://` URL outside `$XDG_RUNTIME_DIR/karere/`
- **THEN** `on_before_resource_load` cancels the load
- **AND** the renderer observes a network error

### Requirement: Render-process routes DispatchPasteEvent to the page
The render-process handler SHALL convert inbound `BrowserMessage::DispatchPasteEvent` into a `karere:dispatch-paste` CustomEvent on `window`.

#### Scenario: Envelope received
- **WHEN** `on_process_message_received` parses a `BrowserMessage::DispatchPasteEvent`
- **THEN** the handler invokes `frame.execute_java_script("window.dispatchEvent(new CustomEvent('karere:dispatch-paste', {detail: <json>}))", "karere://paste", 0)` on the main frame
- **AND** the JSON `detail` contains `mime`, `kind` (`"paste"` or `"drop"`), and the blob (base64 string or file URL)
