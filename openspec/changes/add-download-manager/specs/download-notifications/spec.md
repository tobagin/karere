# download-notifications Specification

## Purpose
Provides toast notification display for download completion with action buttons to quickly open downloaded files.

## ADDED Requirements

### Requirement: Download Toast Notification Display
The Window SHALL display toast notifications when downloads complete, following the existing toast pattern used for clipboard operations.

#### Scenario: Download completion toast display
**GIVEN** a download has completed successfully
**AND** download-notifications-enabled setting is true
**WHEN** the DownloadManager emits download_completed signal
**THEN** Window displays an AdwToast notification
**AND** the toast message shows "Downloaded: {filename}"
**AND** the toast timeout is 5 seconds
**AND** the toast appears in the toast_overlay

#### Scenario: Download notification suppressed when disabled
**GIVEN** download-notifications-enabled setting is false
**WHEN** the DownloadManager emits download_completed signal
**THEN** Window receives the signal but does NOT display a toast
**AND** the download still completes normally

#### Scenario: Multiple downloads show separate toasts
**GIVEN** two files download in quick succession
**WHEN** the first download completes
**AND** the second download completes shortly after
**THEN** two separate toast notifications are shown
**AND** they queue appropriately (LibAdwaita handles queuing)

### Requirement: Toast Action Button for Opening Files
Download toast notifications SHALL include an action button that opens the downloaded file with the default application.

#### Scenario: Toast with "Open" action button
**GIVEN** a download completion toast is displayed
**WHEN** the toast is rendered
**THEN** an action button labeled "Open" is visible
**AND** the button is keyboard accessible
**AND** the button triggers the open-downloaded-file action

#### Scenario: Opening file via toast button
**GIVEN** a download toast is showing for "photo.jpg"
**WHEN** the user clicks the "Open" button
**THEN** the open-downloaded-file action is triggered
**AND** the file path is passed to DownloadManager.open_file()
**AND** the file opens with the default application
**AND** the toast is dismissed

#### Scenario: Opening file via keyboard
**GIVEN** a download toast is showing
**AND** keyboard focus is on the toast action button
**WHEN** the user presses Enter or Space
**THEN** the file opening action is triggered
**AND** the file opens with the default application

### Requirement: Download Action Registration
The Window SHALL register actions for handling download-related operations triggered from toasts.

#### Scenario: Open downloaded file action registration
**GIVEN** Window is being initialized
**WHEN** setup_actions() is called
**THEN** an "open-downloaded-file" SimpleAction is created
**AND** the action accepts a string parameter (file path)
**AND** the action handler calls DownloadManager.open_file()
**AND** the action is added to the window action group

#### Scenario: Action parameter handling
**GIVEN** the open-downloaded-file action is registered
**WHEN** the action is activated with a file path parameter
**THEN** the parameter is extracted as a string
**AND** passed to DownloadManager.open_file()
**AND** errors are handled gracefully

### Requirement: Download Error Toast Display
The Window SHALL display error toasts when download operations fail.

#### Scenario: Directory fallback notification
**GIVEN** a custom download directory becomes unavailable
**WHEN** DownloadManager emits directory_fallback signal
**THEN** Window displays a warning toast
**AND** the message explains "Download directory unavailable, using default"
**AND** the toast timeout is 5 seconds
**AND** no action button is present

#### Scenario: File opening error notification
**GIVEN** the user clicks "Open" on a download toast
**AND** the file no longer exists or cannot be opened
**WHEN** DownloadManager emits error_opening_file signal
**THEN** Window displays an error toast
**AND** the message shows "Failed to open file: {error}"
**AND** the toast timeout is 5 seconds

### Requirement: Toast State Management
The Window SHALL manage toast display state to handle file path references for action buttons.

#### Scenario: Storing file path for action callback
**GIVEN** a download completes with path "/path/to/file.pdf"
**WHEN** the download toast is created
**THEN** the file path is captured in the action closure
**AND** clicking the button uses the correct file path
**AND** the closure properly manages the path lifecycle

#### Scenario: Toast dismissed before action clicked
**GIVEN** a download toast is displayed
**WHEN** the toast timeout expires and it's dismissed
**THEN** the action closure is cleaned up properly
**AND** no memory leaks occur
**AND** the file path reference is released

### Requirement: Filename Extraction
The Window SHALL extract and display readable filenames from download paths in toast notifications.

#### Scenario: Displaying filename from full path
**GIVEN** a download completes with path "/home/user/Downloads/document.pdf"
**WHEN** the toast is constructed
**THEN** the filename "document.pdf" is extracted
**AND** displayed in the toast message "Downloaded: document.pdf"
**AND** the full path is used for the open action

#### Scenario: Handling special characters in filename
**GIVEN** a filename contains special characters "My Photo (1).jpg"
**WHEN** the toast is displayed
**THEN** the filename is shown correctly without escaping issues
**AND** the file opens correctly when the button is clicked

### Requirement: Toast Accessibility
Download toast notifications SHALL be accessible to screen readers and keyboard users.

#### Scenario: Screen reader announcement
**GIVEN** a screen reader is active
**WHEN** a download toast is displayed
**THEN** the toast message is announced automatically
**AND** the presence of the "Open" button is announced
**AND** the user can navigate to the button via keyboard

#### Scenario: Keyboard navigation to toast action
**GIVEN** a download toast is visible
**WHEN** the user presses Tab
**THEN** focus can move to the "Open" button
**AND** pressing Enter activates the button
**AND** the action is triggered appropriately

## MODIFIED Requirements

### Requirement: Navigation Policy Management
The WebViewManager SHALL be enhanced to handle download policy decisions in addition to existing navigation policy handling.

#### Scenario: Download response policy handling
**GIVEN** a WebView is loaded with WhatsApp Web
**WHEN** a navigation policy decision fires with RESPONSE type
**AND** the response is an attachment or downloadable content
**THEN** the WebViewManager identifies this as a download
**AND** calls decision.download() to initiate the download
**AND** emits the download_detected signal
**AND** returns true to indicate the policy was handled

### Requirement: WebViewManager Signal Emissions
The WebViewManager SHALL emit a new download_detected signal to notify other components of downloads.

#### Scenario: Download detected signal emission
**GIVEN** WebViewManager handles a policy decision
**AND** the decision type is RESPONSE
**AND** the response indicates a download
**WHEN** WebViewManager calls decision.download()
**THEN** a download_detected signal is emitted
**AND** the signal includes the download URI
**AND** the signal includes the suggested filename

#### Scenario: WebViewManager signal definition
**GIVEN** WebViewManager.vala class definition
**WHEN** the class signals are declared
**THEN** a download_detected signal is defined
**AND** the signal signature is: (string uri, string suggested_filename)
**AND** Window or DownloadManager can connect to this signal
