# download-manager Specification

## Purpose
Provides download detection, directory management, file opening, and download completion notifications for WhatsApp Web downloads within Karere.

## ADDED Requirements

### Requirement: Download Manager Class Creation
The application SHALL provide a `DownloadManager` class in `src/managers/DownloadManager.vala` that handles download detection, custom directory management, and file opening operations.

#### Scenario: Download manager instantiation
**GIVEN** the application is starting up
**WHEN** Application.vala creates the DownloadManager instance
**THEN** the manager is initialized as a singleton
**AND** the manager loads settings for custom download directory
**AND** the manager prepares to handle download signals

#### Scenario: Download manager dependency injection
**GIVEN** the Application has created a DownloadManager
**WHEN** Window is instantiated
**THEN** the DownloadManager reference is passed to the Window
**AND** the Window passes it to WebViewManager for integration

### Requirement: Download Detection via Policy Decisions
The DownloadManager SHALL detect downloads by monitoring WebKit policy decisions for response types that indicate file downloads.

#### Scenario: Attachment download detection
**GIVEN** a WebView is displaying WhatsApp Web
**WHEN** a user clicks a download link for an attachment
**AND** WebKit fires a decide_policy signal with RESPONSE type
**AND** the response has is_attachment flag set to true
**THEN** the DownloadManager detects this as a download
**AND** emits a download_detected signal with the URI

#### Scenario: Media file download detection
**GIVEN** a WebView is displaying WhatsApp Web
**WHEN** a user saves an image or video
**AND** WebKit fires a decide_policy signal with RESPONSE type
**AND** the response MIME type indicates downloadable content
**THEN** the DownloadManager detects this as a download
**AND** resolves the target download directory

#### Scenario: Normal navigation not treated as download
**GIVEN** a WebView navigation occurs
**WHEN** the policy decision is not a RESPONSE type
**OR** the response is not an attachment
**AND** the MIME type is text/html or application/json
**THEN** the DownloadManager does NOT treat this as a download
**AND** navigation proceeds normally

### Requirement: Download Directory Resolution
The DownloadManager SHALL resolve the target download directory based on user settings with fallback handling.

#### Scenario: Custom directory resolution when set and accessible
**GIVEN** the user has set a custom download directory in settings
**AND** the custom directory path is "/home/user/Documents/WhatsApp"
**AND** the directory exists and is writable
**WHEN** a download is detected
**THEN** the DownloadManager resolves to the custom directory
**AND** WebKit downloads the file to that location

#### Scenario: Fallback to xdg-download when custom directory unavailable
**GIVEN** the user has set a custom download directory
**AND** the custom directory no longer exists or is not accessible
**WHEN** a download is detected
**THEN** the DownloadManager falls back to xdg-download directory
**AND** emits a directory_fallback signal
**AND** downloads proceed to the fallback location

#### Scenario: Default behavior when no custom directory set
**GIVEN** the custom-download-directory setting is empty string
**WHEN** a download is detected
**THEN** the DownloadManager uses the xdg-download directory
**AND** no fallback signal is emitted

#### Scenario: Ultimate fallback to ~/Downloads
**GIVEN** custom directory is not set or unavailable
**AND** xdg-download directory cannot be determined
**WHEN** a download is detected
**THEN** the DownloadManager falls back to ~/Downloads
**AND** creates the directory if it doesn't exist

### Requirement: Download Completion Notification
The DownloadManager SHALL emit signals when downloads complete to enable UI notifications.

#### Scenario: Download completion signal emission
**GIVEN** a download has been initiated via WebKit
**WHEN** the download completes successfully
**THEN** the DownloadManager emits a download_completed signal
**AND** the signal includes the filename
**AND** the signal includes the full file path
**AND** Window can display a toast notification

#### Scenario: Download completion with notifications disabled
**GIVEN** download-notifications-enabled setting is false
**WHEN** a download completes
**THEN** the DownloadManager still emits the download_completed signal
**AND** Window checks the setting before displaying toast

### Requirement: Downloaded File Opening
The DownloadManager SHALL provide functionality to open downloaded files with the system default application using Flatpak portals.

#### Scenario: Opening downloaded file via portal
**GIVEN** a download has completed with path "/path/to/file.pdf"
**WHEN** open_file() is called with the file path
**THEN** the DownloadManager constructs a file:// URI
**AND** calls AppInfo.launch_default_for_uri_async() with the URI
**AND** the file opens in the default PDF viewer via portal

#### Scenario: File opening failure handling
**GIVEN** a file path points to a non-existent file
**WHEN** open_file() is called
**THEN** the operation fails gracefully
**AND** an error_opening_file signal is emitted with error message
**AND** Window can display an error toast

#### Scenario: Opening image file
**GIVEN** a downloaded image file "photo.jpg"
**WHEN** the user clicks "Open" in the download toast
**THEN** the DownloadManager opens the file
**AND** the system default image viewer launches via portal

### Requirement: Download Manager Signals
The DownloadManager SHALL provide signals for integration with Window and other components.

#### Scenario: Signal availability for Window connection
**GIVEN** a Window is being initialized
**WHEN** the Window receives the DownloadManager reference
**THEN** the Window can connect to download_detected signal
**AND** the Window can connect to download_completed signal
**AND** the Window can connect to download_failed signal
**AND** the Window can connect to directory_fallback signal
**AND** the Window can connect to error_opening_file signal

### Requirement: Settings Integration
The DownloadManager SHALL read and respect settings for custom download directory and notification preferences.

#### Scenario: Reading custom directory setting
**GIVEN** GSettings contains custom-download-directory key
**WHEN** the DownloadManager initializes
**THEN** it reads the current value
**AND** monitors the key for changes
**AND** updates internal state when the setting changes

#### Scenario: Dynamic settings update
**GIVEN** the DownloadManager is running
**WHEN** the user changes the custom download directory in preferences
**THEN** the DownloadManager receives the settings change notification
**AND** subsequent downloads use the new directory
**AND** no application restart is required
