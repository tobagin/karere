## ADDED Requirements

### Requirement: Store hook is the first-class identity source
The system SHALL inject `data/js/store_hook.js` into every WhatsApp Web main frame via the M13 bundle pipeline and SHALL use the Webpack-internals technique (matching `@wppconnect/wa-js`) to access `window.Store`.

#### Scenario: Store is reached via __webpack_require__
- **WHEN** `data/js/store_hook.js` runs at `on_context_created`
- **THEN** it overrides `__webpack_require__` to capture every loaded module
- **AND** it does NOT rely on DOM selectors to locate `Store`

#### Scenario: Identity events emit ProfileIdentity
- **GIVEN** the Store hook is attached
- **WHEN** `Store.Conn.wid` and `Store.Conn.pushname` become available
- **THEN** the script posts `RendererMessage::ProfileIdentity { wid, pushname }` to the browser process

#### Scenario: Avatar fetch resolves to base64 PNG
- **GIVEN** `Store.Conn.wid` is known
- **WHEN** `Store.ProfilePicThumb.find(wid)` resolves with a thumbnail descriptor
- **THEN** the script fetches `descriptor.eurl`, reads the blob via `FileReader.readAsDataURL`, and posts `RendererMessage::ProfileAvatar { base64_png }`

#### Scenario: Avatar re-fetch on pic-changed
- **GIVEN** the Store hook has subscribed to `Store.Contact(self-wid)`
- **WHEN** a `change:profilePicThumb` (or equivalent) event fires
- **THEN** the avatar is re-fetched and `RendererMessage::ProfileAvatar` is re-emitted

### Requirement: Pairing state from Store.AppState
The system SHALL derive pairing state from `Store.AppState.state` and SHALL NOT rely on URL inspection.

#### Scenario: AwaitingPairing emitted while not connected
- **GIVEN** the Store hook is attached
- **WHEN** `Store.AppState.state !== 'CONNECTED'`
- **THEN** the script posts `RendererMessage::AwaitingPairing` (debounced)

#### Scenario: AwaitingPairing clears on CONNECTED
- **WHEN** `Store.AppState.state` transitions to `'CONNECTED'`
- **THEN** no further `AwaitingPairing` messages are emitted until the state leaves `CONNECTED`

### Requirement: StoreUnavailable degraded mode
The system SHALL emit `RendererMessage::StoreUnavailable { reason }` when the Webpack hook fails and SHALL activate a degraded DOM-only fallback path.

#### Scenario: Hook failure emits StoreUnavailable
- **WHEN** the Store hook throws, finds no Store namespace, or otherwise cannot reach `Store.Conn`
- **THEN** the script posts `RendererMessage::StoreUnavailable { reason }` with a human-readable reason

#### Scenario: DOM fallback loads only after StoreUnavailable
- **WHEN** the browser process receives `StoreUnavailable` for an account
- **THEN** `data/js/profile_dom_fallback.js` is injected into that account's main frame
- **AND** before that message, the DOM fallback is NOT injected

#### Scenario: DOM fallback identifies its source
- **WHEN** the DOM fallback reports identity or avatar
- **THEN** the IPC payload carries `source: "dom-fallback"`

#### Scenario: Degraded badge persists even on fallback success
- **GIVEN** the switcher row is in degraded mode for an account
- **WHEN** the DOM fallback successfully reports identity and avatar
- **THEN** the "degraded mode" yellow badge remains visible on that account's row
- **AND** the badge is only cleared when a subsequent successful Store hook attachment occurs

### Requirement: DOM fallback scrape targets
The system SHALL, when in degraded mode, derive identity and avatar exclusively from the documented DOM landmarks.

#### Scenario: Fallback reads name from header span
- **WHEN** `data/js/profile_dom_fallback.js` reads the pushname
- **THEN** it reads `#side header span[dir="auto"][title]`

#### Scenario: Fallback reads avatar from header img
- **WHEN** the fallback reads the avatar
- **THEN** it polls `#side header img` for a `blob:` `src` and converts it to a base64 PNG
