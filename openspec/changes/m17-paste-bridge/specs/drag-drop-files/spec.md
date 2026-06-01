## ADDED Requirements

### Requirement: DropTarget accepts file lists on the embedded surface
The system SHALL register a `gtk::DropTarget` on the CEF embedding widget that accepts `gdk::FileList` payloads.

#### Scenario: DropTarget registered with FileList type
- **WHEN** `install_input_controllers` completes
- **THEN** the widget has a `DropTarget` whose accepted types include `gdk::FileList::static_type()`
- **AND** the target is configured for the COPY action

#### Scenario: Drop of a file fires connect_drop
- **WHEN** the user drags a file from a file manager and releases it on the widget
- **THEN** the `connect_drop` callback receives the `FileList` and the drop coordinates

### Requirement: Dropped files surface as a synthetic drop event
The host SHALL marshal each dropped file into a `BrowserMessage::DispatchPasteEvent { kind: "drop" }` envelope so the renderer can synthesize a DOM `drop` event on the element under the cursor.

#### Scenario: PDF drop
- **WHEN** the user drops a PDF onto the chat input area
- **THEN** the host sends `DispatchPasteEvent` with `mime` matching the file (`application/pdf`), `kind: "drop"`, and the original filename preserved in the envelope
- **AND** `paste_bridge.js` constructs a `File` with that name and MIME
- **AND** dispatches a `drop` event on the element at the recorded drop coordinates with `dataTransfer` populated

#### Scenario: Multi-file drop
- **WHEN** the user drops multiple files at once
- **THEN** the renderer receives all files in the dispatched `dataTransfer` of a single `drop` event in the order the host iterated them

### Requirement: Drop payloads share the tempfile path for large files
The drop path SHALL reuse the >1 MB tempfile branch from the paste bridge.

#### Scenario: 10 MB video drop
- **WHEN** the user drops a 10 MB video file
- **THEN** the host writes the bytes to `$XDG_RUNTIME_DIR/karere/paste-<uuid>` with permissions `0600`
- **AND** sends `PasteBlob::FilePath(path)` in the envelope
- **AND** the tempfile is unlinked after consumption or the 30 s fallback timer
