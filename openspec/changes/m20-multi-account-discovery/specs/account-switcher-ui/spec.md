## ADDED Requirements

### Requirement: Account switcher Blueprint surface
The system SHALL provide an account switcher defined in `data/ui/account_switcher.blp`, ported from karere v3's `data/ui/window.blp:85-122` bottom-sheet + popover structure.

#### Scenario: Blueprint file exists and is registered
- **WHEN** the build runs
- **THEN** `data/ui/account_switcher.blp` is compiled and embedded in the GResource bundle

#### Scenario: Bottom-sheet on mobile, popover on desktop
- **WHEN** the switcher is shown at narrow widths (`Adw.Breakpoint` mobile)
- **THEN** it presents as an `Adw.BottomSheet`
- **AND** at wide widths it presents as a `Gtk.Popover`

### Requirement: Adw.Avatar rendering
The system SHALL render account avatars with `Adw.Avatar`, never with a custom Cairo/Pango pipeline.

#### Scenario: Custom-image path when avatar_png present
- **WHEN** an account has `avatar_png: Some(bytes)`
- **THEN** the row's `Adw.Avatar` has `custom-image` set to `gdk::Texture::from_bytes(&Bytes::from(&bytes))`

#### Scenario: Initials path when avatar_png absent
- **WHEN** an account has `avatar_png: None`
- **THEN** the row's `Adw.Avatar` has `show-initials = true`
- **AND** `text = user_label.or(pushname).unwrap_or("?")`
- **AND** Adw.Avatar's built-in color hash provides the tint

### Requirement: MRU-ordered row layout
The switcher SHALL render rows in the order returned by `AccountManager::get_accounts_sorted` and SHALL NOT expose any reorder UI.

#### Scenario: Rows match MRU order
- **WHEN** the switcher is shown
- **THEN** rows appear top-to-bottom in `last_used_at` descending order

#### Scenario: No drag handles or reorder controls
- **WHEN** the switcher is inspected
- **THEN** no row exposes a drag handle, up/down arrow, or reorder context-menu action

#### Scenario: Activation reorders on next show
- **GIVEN** order is C, A, B
- **WHEN** the user activates B
- **THEN** the next time the switcher opens, rows are B, C, A

### Requirement: Degraded-mode badge
The switcher SHALL display a persistent yellow "degraded mode" badge on any row whose account is in `StoreUnavailable` state.

#### Scenario: Badge appears on StoreUnavailable
- **WHEN** an account has received `RendererMessage::StoreUnavailable` since the last successful Store hook
- **THEN** its row shows a yellow "degraded mode" badge

#### Scenario: Badge persists through DOM-fallback success
- **GIVEN** the badge is shown for an account
- **WHEN** the DOM fallback successfully reports identity and avatar
- **THEN** the badge remains visible

### Requirement: Awaiting-pairing row state
The switcher SHALL display a "Waiting for QR scan…" subtitle and a spinner on rows whose account has not yet reached `Store.AppState.state === 'CONNECTED'`.

#### Scenario: AwaitingPairing renders spinner
- **WHEN** `RendererMessage::AwaitingPairing` has been received for an account and no subsequent CONNECTED event
- **THEN** the row subtitle reads "Waiting for QR scan…"
- **AND** a spinner is shown in the row

#### Scenario: Connected clears spinner
- **WHEN** the account transitions to CONNECTED
- **THEN** the subtitle is replaced with the account's `pushname` (or `user_label`)
- **AND** the spinner is removed

### Requirement: Add/edit dialog
The system SHALL provide an add/edit dialog ported from `karere/src/window.rs:2597-2829`, with emoji and color fields removed.

#### Scenario: Only user_label is editable
- **WHEN** the dialog is opened for an existing account
- **THEN** the `user_label` entry is editable
- **AND** `wid`, `pushname`, `avatar_url` are displayed read-only (greyed)
- **AND** no emoji picker or color picker is present

#### Scenario: New account opens dialog and spawns hidden browser
- **WHEN** the user invokes "Add account"
- **THEN** a new `Account` row is created
- **AND** a hidden CEF browser is spawned for it pointing at `https://web.whatsapp.com`
- **AND** the dialog closes when `RendererMessage::ProfileIdentity` arrives or the user cancels
