## ADDED Requirements

### Requirement: Page selection mirrors to the PRIMARY selection
The renderer SHALL report page text selections to the host, which writes them to
the GDK PRIMARY selection, so middle-click paste of the selection works in other
applications. CEF's offscreen (windowless) mode never owns the system clipboard,
so the page must report its selection for the host to publish it.

#### Scenario: Selecting text in a chat
- **WHEN** the user selects text in the embedded page
- **THEN** `data/js/50-copy-bridge.js` reads `window.getSelection().toString()` on the debounced `selectionchange` event
- **AND** sends `RendererMessage::SetClipboard { text, primary: true }`
- **AND** the browser process writes the text to `gdk::Display::default().primary_clipboard()`
- **AND** middle-clicking in another application pastes the selected text

#### Scenario: Empty selection leaves PRIMARY intact
- **WHEN** the selection becomes empty (deselect)
- **THEN** no `SetClipboard` message is sent
- **AND** the existing PRIMARY selection is left unchanged (matching X11 behavior)

### Requirement: Ctrl+C mirrors the selection to the CLIPBOARD
The host SHALL promote the PRIMARY selection to the regular CLIPBOARD on Ctrl+C.
CEF's offscreen copy never reaches the system clipboard, and the DOM `copy` event
does not fire under windowless rendering, so the regular clipboard cannot be
driven from the page; the GTK key controller handles it instead.

#### Scenario: Ctrl+C on selected text
- **WHEN** the user presses Ctrl+C with text selected in the page
- **THEN** the GTK `EventControllerKey` handler reads `gdk::Display::default().primary_clipboard()` text
- **AND** writes it to `gdk::Display::default().clipboard()`
- **AND** Ctrl+V in another application pastes the text
- **AND** the keystroke is still forwarded to CEF so in-page behavior is unaffected

#### Scenario: Ctrl+C with no selection
- **WHEN** Ctrl+C is pressed with nothing selected
- **THEN** the PRIMARY read yields empty and the CLIPBOARD is left unchanged

### Requirement: Outbound mirror does not depend on the DOM copy event
The outbound path SHALL NOT rely on the page `copy` event, which does not fire
under CEF windowless rendering. The PRIMARY mirror is driven by `selectionchange`
and the CLIPBOARD by the GTK-level Ctrl+C handler reading PRIMARY.

#### Scenario: Copy works without a firing copy event
- **WHEN** the user selects text and presses Ctrl+C, and the page `copy` event never fires
- **THEN** the selection is still on PRIMARY (from `selectionchange`)
- **AND** the GTK Ctrl+C handler still promotes it to the CLIPBOARD
