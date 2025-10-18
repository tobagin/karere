# Clipboard Manager

## ADDED Requirements

### Requirement: Clipboard Manager Class Creation
The application SHALL provide a `ClipboardManager` class in `src/managers/ClipboardManager.vala` that handles clipboard image paste operations and WhatsApp injection.

#### Scenario: Clipboard manager instantiation
**GIVEN** the application creates a new Window with a clipboard and WebView
**WHEN** ClipboardManager is instantiated
**THEN** the manager receives Gdk.Clipboard and WebKit.WebView references
**AND** the manager is ready to detect paste events

### Requirement: Paste Event Detection
The clipboard manager SHALL detect Ctrl+V keypress events on the WebView and intercept them for processing.

#### Scenario: Setup paste detection
**GIVEN** ClipboardManager is instantiated
**WHEN** setup_paste_detection() is called with the WebView widget
**THEN** a Gtk.EventControllerKey is created
**AND** the controller is added to the WebView
**AND** key_pressed signal is connected

#### Scenario: Ctrl+V keypress detected
**GIVEN** paste detection is set up
**WHEN** user presses Ctrl+V while focused on WebView
**THEN** the key event is intercepted
**AND** handle_paste_request() is called
**AND** the key event is consumed (returns true)

#### Scenario: Other keypress passthrough
**GIVEN** paste detection is set up
**WHEN** user presses any key other than Ctrl+V
**THEN** the key event is not consumed (returns false)
**AND** handle_paste_request() is not called

### Requirement: Clipboard Image Format Detection
The clipboard manager SHALL detect various image formats in the clipboard.

#### Scenario: Texture format detection
**GIVEN** clipboard contains a Gdk.Texture
**WHEN** handle_paste_request() is called
**THEN** the clipboard is queried for Gdk.Texture type
**AND** the texture is read asynchronously
**AND** paste_started signal is emitted

#### Scenario: PNG MIME type detection
**GIVEN** clipboard contains "image/png" MIME type but not Texture
**WHEN** handle_paste_request() is called
**THEN** the clipboard is read for image/png
**AND** the image stream is processed
**AND** paste_started signal is emitted

#### Scenario: JPEG MIME type detection
**GIVEN** clipboard contains "image/jpeg" MIME type
**WHEN** handle_paste_request() is called
**THEN** the clipboard is read for image/jpeg
**AND** the image stream is processed

#### Scenario: GIF MIME type detection
**GIVEN** clipboard contains "image/gif" MIME type
**WHEN** handle_paste_request() is called
**THEN** the clipboard is read for image/gif
**AND** the image stream is processed

#### Scenario: No image format detection
**GIVEN** clipboard contains only text
**WHEN** handle_paste_request() is called
**THEN** no image processing occurs
**AND** default paste JavaScript is injected

### Requirement: Image Processing and Conversion
The clipboard manager SHALL convert clipboard images to base64 data URLs for JavaScript injection.

#### Scenario: Texture to PNG conversion
**GIVEN** clipboard contains a Gdk.Texture
**WHEN** the texture is read successfully
**THEN** the texture is converted to PNG bytes
**AND** the bytes are base64 encoded
**AND** a data URL with "image/png" MIME type is created

#### Scenario: Image stream to bytes conversion
**GIVEN** clipboard contains an image stream with MIME type
**WHEN** the stream is read successfully
**THEN** the stream is read into a MemoryOutputStream
**AND** the bytes are extracted and base64 encoded
**AND** a data URL with the original MIME type is created

#### Scenario: Image conversion failure
**GIVEN** clipboard image processing fails
**WHEN** an error occurs during conversion
**THEN** a critical log message is recorded
**AND** paste_failed signal is emitted with error message
**AND** no JavaScript is injected

### Requirement: WhatsApp Image Injection
The clipboard manager SHALL inject images into WhatsApp Web using JavaScript.

#### Scenario: Image injection via clipboard event
**GIVEN** an image is converted to a data URL
**WHEN** inject_image_into_whatsapp() is called
**THEN** JavaScript finds the WhatsApp message box
**AND** a File object is created from the data URL
**AND** a ClipboardEvent with the file is created
**AND** the event is dispatched to the message box
**AND** paste_started signal is emitted

#### Scenario: Image injection success
**GIVEN** image injection JavaScript is executed
**WHEN** the JavaScript completes successfully
**THEN** paste_succeeded signal is emitted with image type
**AND** an info log message is recorded

#### Scenario: Image injection failure
**GIVEN** image injection JavaScript is executed
**WHEN** the JavaScript fails with an error
**THEN** paste_failed signal is emitted with error message
**AND** a critical log message is recorded

#### Scenario: WhatsApp message box not found
**GIVEN** image injection JavaScript runs
**WHEN** the WhatsApp message box selector doesn't match
**THEN** the JavaScript logs "message box not found"
**AND** no error is thrown (graceful degradation)

### Requirement: Default Paste Fallback
The clipboard manager SHALL fall back to default paste behavior for non-image content.

#### Scenario: Text content paste
**GIVEN** clipboard contains only text
**WHEN** handle_paste_request() determines no image is present
**THEN** inject_default_paste() is called
**AND** JavaScript finds the WhatsApp message box
**AND** document.execCommand('paste') is executed
**AND** text content is pasted normally

#### Scenario: Default paste JavaScript execution
**GIVEN** default paste is triggered
**WHEN** the JavaScript executes successfully
**THEN** a debug log message is recorded
**AND** no paste signals are emitted (transparent passthrough)

### Requirement: Error Handling and Logging
The clipboard manager SHALL provide comprehensive error handling and logging.

#### Scenario: Null clipboard handling
**GIVEN** clipboard reference is null
**WHEN** handle_paste_request() is called
**THEN** a warning is logged
**AND** no clipboard operations are attempted

#### Scenario: Null WebView handling
**GIVEN** WebView reference is null
**WHEN** JavaScript injection is attempted
**THEN** a warning is logged
**AND** no JavaScript is executed

#### Scenario: Clipboard read timeout
**GIVEN** clipboard read operation takes too long
**WHEN** the async operation times out
**THEN** an error is caught
**AND** paste_failed signal is emitted
**AND** a critical log message is recorded
