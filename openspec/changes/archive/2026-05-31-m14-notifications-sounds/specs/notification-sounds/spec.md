## ADDED Requirements

### Requirement: Optional Custom Sound Playback
The system SHALL play a custom notification sound when a `NotificationSeen` event arrives and `notify-sound-enabled` is `true`; otherwise it SHALL NOT emit any host-side audio.

#### Scenario: Sound enabled plays selected file
- **WHEN** `notify-sound-enabled` is `true` and `notify-sound-file` is `"whatsapp"`
- **THEN** the system SHALL play `<gresource>/sounds/whatsapp.oga` for each received `NotificationSeen`

#### Scenario: Sound disabled stays silent
- **WHEN** `notify-sound-enabled` is `false`
- **THEN** the system SHALL NOT spawn `paplay` or any fallback audio process for incoming notifications

### Requirement: Sound Asset Extraction
Sound assets SHALL be bundled inside the gresource and extracted lazily to `$XDG_RUNTIME_DIR/karere/sounds/` so external audio CLIs can read them as plain files.

#### Scenario: First playback extracts asset
- **WHEN** a sound file is requested for the first time in a session
- **THEN** the system SHALL extract the gresource entry to `$XDG_RUNTIME_DIR/karere/sounds/<name>.oga` and SHALL reuse that path for subsequent playback in the same session

### Requirement: Playback Backend with Fallback
The system SHALL prefer `paplay` via `gio::Subprocess::spawn` and SHALL fall back to `gst-launch-1.0 playbin uri=file://...` when `paplay` is unavailable.

#### Scenario: paplay available
- **WHEN** `paplay` resolves on `PATH`
- **THEN** the system SHALL invoke `paplay <path>` and SHALL NOT invoke the GStreamer fallback

#### Scenario: paplay missing falls back
- **WHEN** `paplay` is not found on `PATH`
- **THEN** the system SHALL spawn `gst-launch-1.0 playbin uri=file://<path>` instead

#### Scenario: Both backends missing degrade silently
- **WHEN** neither `paplay` nor `gst-launch-1.0` is available
- **THEN** the system SHALL log a warning once and SHALL NOT raise an error to the user

### Requirement: Chromium Sound Suppression Switch
If Chromium emits its own notification sound that overlaps with the custom sound, the system SHALL append `--disable-notification-sound` (exact switch verified at integration time) in `cef_runtime::on_before_command_line_processing` to suppress the duplicate.

#### Scenario: Double-audio suppression
- **WHEN** integration testing observes Chromium playing a notification sound in addition to the custom sound
- **THEN** the launcher SHALL append the verified Chromium switch to suppress the platform sound while the custom sound remains active

