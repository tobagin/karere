## ADDED Requirements

### Requirement: Browser-to-renderer message envelope
The system SHALL define a `BrowserMessage` enum (`src/ipc.rs`) representing all messages sent from the browser process to the renderer subprocess.

#### Scenario: Required variants are present
- **WHEN** the `BrowserMessage` enum is compiled
- **THEN** it contains at minimum: `DispatchPasteEvent { mime: String, payload: PasteBlob }`, `SetViewportSize { w: i32, h: i32 }`, and `CloseNotifByTag { tag: String }`
- **AND** `PasteBlob` is an enum with `Base64(String)` and `FilePath(PathBuf)` variants

#### Scenario: Debug Ping variant exists in debug builds
- **WHEN** the crate is compiled with `cfg(debug_assertions)` or the `debug-ipc` feature enabled
- **THEN** `BrowserMessage::Ping` is available
- **AND** is omitted in release builds without the feature

### Requirement: Renderer-to-browser message envelope
The system SHALL define a `RendererMessage` enum representing all messages sent from the renderer subprocess to the browser process.

#### Scenario: Required variants are present
- **WHEN** the `RendererMessage` enum is compiled
- **THEN** it contains at minimum: `ProfileIdentity { wid, pushname }`, `ProfileAvatar { base64_png }`, `AwaitingPairing`, `StoreUnavailable { reason }`, `NotificationSeen { account_id, title, tag, has_icon }`, `NotificationClosed { tag }`, and `ConsoleLog { level, msg }`

#### Scenario: Debug Pong variant exists in debug builds
- **WHEN** the crate is compiled with `cfg(debug_assertions)` or the `debug-ipc` feature enabled
- **THEN** `RendererMessage::Pong` is available

### Requirement: CefProcessMessage serialization
The IPC module SHALL provide bidirectional conversion between typed envelope values and `CefProcessMessage`.

#### Scenario: Encoding to CefProcessMessage
- **WHEN** `to_cef_message(name, args_json)` is called
- **THEN** it returns a `CefProcessMessage` whose name is the variant tag and whose single string argument is `base64(json(payload))`

#### Scenario: Decoding from CefProcessMessage
- **WHEN** `BrowserMessage::try_from_cef_message(&msg)` (or the `RendererMessage` equivalent) is called on a well-formed message
- **THEN** it returns `Ok(value)` with the deserialized typed variant

#### Scenario: Decoding rejects unknown name
- **WHEN** the conversion is called on a message whose name does not match any variant tag
- **THEN** it returns an `Err` indicating an unknown variant

#### Scenario: Decoding rejects malformed payload
- **WHEN** the conversion is called on a message whose payload fails base64 decode or JSON deserialization
- **THEN** it returns an `Err` describing the failure without panicking

### Requirement: Debug Ping/Pong roundtrip
When debug IPC is enabled the system SHALL respond to a `BrowserMessage::Ping` with a `RendererMessage::Pong` sent on the same browser's IPC channel.

#### Scenario: Ping receives Pong
- **WHEN** the browser process sends `BrowserMessage::Ping` to the main frame
- **THEN** the renderer receives, parses, and replies with `RendererMessage::Pong` within 50 ms under normal load
