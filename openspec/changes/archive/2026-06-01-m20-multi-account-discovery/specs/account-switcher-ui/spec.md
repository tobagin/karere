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
- **THEN** the row's `Adw.Avatar` `custom-image` is a `gdk::Texture` decoded from the bytes via a `gdk_pixbuf::PixbufLoader` (works for JPEG/PNG regardless of the GTK `Texture::from_bytes` availability)

#### Scenario: Initials path when avatar_png absent
- **WHEN** an account has `avatar_png: None`
- **THEN** the row's `Adw.Avatar` has `show-initials = true`
- **AND** `text` is the account name (pushname), else the user label, else `"Account"`
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

### Requirement: Row shows name as title and label as subtitle
Each row SHALL show the account's WhatsApp name (pushname) as the title and the user label as the subtitle, falling back sensibly before pairing.

#### Scenario: Paired row
- **WHEN** the account has a pushname
- **THEN** the row title is the pushname and the subtitle is the user label

#### Scenario: Unpaired row
- **WHEN** the account has no pushname yet
- **THEN** the row title is the user label (e.g. the auto-assigned "Account N")
- **AND** the subtitle reads "Waiting for QR scan…" with a spinner, until pairing completes

### Requirement: Add/edit dialog
The system SHALL provide an add/edit dialog ported from `karere/src/window.rs:2597-2829`, with emoji and color fields removed.

#### Scenario: Only user_label is editable
- **WHEN** the dialog is opened for an existing account
- **THEN** the `user_label` entry is editable
- **AND** `wid`, `pushname`, `avatar_url` are displayed read-only (greyed)
- **AND** no emoji picker or color picker is present

#### Scenario: Add account is prompt-free and goes straight to the QR
- **WHEN** the user invokes "Add account"
- **THEN** a new `Account` is created with a unique default label ("Account N", user-editable later)
- **AND** its CEF browser is spawned as the foreground pointing at `https://web.whatsapp.com` so the QR is visible immediately
- **AND** no dialog is shown upfront (the real name fills in from identity discovery on pairing)

### Requirement: Account count is capped and the last account is protected
The system SHALL cap linked accounts at 9 (matching the Alt+1..9 switch shortcuts) and SHALL NOT allow removing the only account.

#### Scenario: Add disabled at the cap
- **WHEN** 9 accounts exist
- **THEN** the "Add account" action is disabled and invoking it shows a "Maximum of 9 accounts reached" toast

#### Scenario: Last account cannot be removed
- **WHEN** only one account exists
- **THEN** its row's remove control is disabled and removal is refused

### Requirement: Keyboard shortcuts switch accounts
The system SHALL provide shortcuts to switch accounts in MRU order.

#### Scenario: Jump and cycle
- **WHEN** the user presses `Alt+1` … `Alt+9`
- **THEN** the switcher activates the account at that 1-based MRU position
- **AND** `Ctrl+Tab` / `Ctrl+Shift+Tab` cycle to the next / previous account
