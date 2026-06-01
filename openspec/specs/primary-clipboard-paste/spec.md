# primary-clipboard-paste Specification

## Purpose

Supports X11-style middle-click paste of the PRIMARY selection into the embedded
page. The middle-button gesture reads the GDK primary clipboard text and
dispatches it as a synthetic paste, while still allowing the middle click to
propagate to CEF so middle-click-to-open-link semantics remain intact.

## Requirements

### Requirement: Middle-click reads primary clipboard text
The middle-button `GestureClick::connect_pressed` handler SHALL read `gdk::Display::default().primary_clipboard()` text content and dispatch it as a synthetic paste.

#### Scenario: Text on primary clipboard
- **WHEN** the user selects text in another application and middle-clicks inside the embedded page
- **THEN** the host reads the primary clipboard text asynchronously
- **AND** sends `BrowserMessage::DispatchPasteEvent { mime: "text/plain", kind: "paste", payload: PasteBlob::Base64(<b64>) }`
- **AND** `paste_bridge.js` dispatches a `paste` event whose `clipboardData.getData('text/plain')` returns the original text

#### Scenario: Empty primary clipboard
- **WHEN** the user middle-clicks and the primary clipboard has no text content
- **THEN** the host does not send any IPC message

### Requirement: Middle-click is not swallowed
The middle-button handler SHALL allow the click to propagate to CEF so that middle-click-to-open-link semantics remain intact.

#### Scenario: Middle-click on a link
- **WHEN** the user middle-clicks a link with primary-clipboard text present
- **THEN** the host dispatches the `DispatchPasteEvent` AND forwards the middle button down/up to CEF
- **AND** CEF's link handler opens the link in the system browser (per M7)

#### Scenario: Middle-click in a text input
- **WHEN** the user middle-clicks inside a focused text input
- **THEN** the synthesized paste populates the input with the primary-clipboard text
- **AND** any duplicate middle-click action from Chromium MUST NOT result in a duplicate paste under default Chromium configuration
