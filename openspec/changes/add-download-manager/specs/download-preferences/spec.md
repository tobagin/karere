# download-preferences Specification

## Purpose
Provides user interface for configuring download directory and download notification settings in the Preferences dialog.

## ADDED Requirements

### Requirement: Download Preferences Page
The PreferencesDialog SHALL provide a dedicated page for download-related settings accessible from the preferences navigation.

#### Scenario: Download preferences page availability
**GIVEN** the user opens the Preferences dialog
**WHEN** the dialog displays the navigation sidebar
**THEN** a "Downloads" page is available in the list
**AND** clicking it displays download-related settings
**AND** the page follows LibAdwaita preferences page design patterns

#### Scenario: Download preferences page layout
**GIVEN** the user navigates to the Downloads preferences page
**WHEN** the page is displayed
**THEN** it shows a "Download Directory" section
**AND** it shows a "Notifications" section
**AND** all controls are properly labeled for accessibility

### Requirement: Custom Download Directory Setting
The download preferences SHALL provide UI to select and display a custom download directory using the File Chooser Portal.

#### Scenario: Custom directory selection button
**GIVEN** the Downloads preferences page is open
**WHEN** the page renders the Download Directory section
**THEN** an AdwActionRow displays the current directory or "Default (Downloads)"
**AND** a "Choose Folder" button is present
**AND** the button has an accessible label and tooltip

#### Scenario: Opening file chooser portal for directory selection
**GIVEN** the user is on the Downloads preferences page
**WHEN** the user clicks "Choose Folder" button
**THEN** a Gtk.FileDialog opens in folder selection mode
**AND** the dialog is modal to the preferences window
**AND** the dialog title is "Select Download Directory"
**AND** the dialog uses the portal automatically in Flatpak

#### Scenario: Successful directory selection
**GIVEN** the file chooser dialog is open
**WHEN** the user selects a directory and clicks "Select"
**THEN** the dialog closes
**AND** the custom-download-directory setting is updated with the path
**AND** the UI updates to display the selected path
**AND** a success message is shown (optional)

#### Scenario: Cancelling directory selection
**GIVEN** the file chooser dialog is open
**WHEN** the user clicks "Cancel"
**THEN** the dialog closes
**AND** no settings are changed
**AND** the previous directory selection remains active
**AND** no error message is shown

#### Scenario: Directory selection error handling
**GIVEN** the file chooser dialog operation fails
**WHEN** an error occurs (not cancellation)
**THEN** the error is caught
**AND** an error toast is displayed with the error message
**AND** the previous setting remains unchanged

#### Scenario: Displaying custom directory path
**GIVEN** a custom download directory is set to "/home/user/Documents/WhatsApp"
**WHEN** the Downloads preferences page is displayed
**THEN** the directory row shows the full path
**AND** the path is readable and not truncated unnecessarily
**AND** the row indicates this is a custom location

#### Scenario: Displaying default directory
**GIVEN** no custom download directory is set (empty string)
**WHEN** the Downloads preferences page is displayed
**THEN** the directory row shows "Default (Downloads)"
**AND** indicates the system download folder will be used

### Requirement: Reset to Default Directory
The download preferences SHALL provide a way to reset the download directory to the system default.

#### Scenario: Reset button availability
**GIVEN** a custom download directory is set
**WHEN** the Downloads preferences page is displayed
**THEN** a "Reset to Default" button is visible and enabled
**AND** the button has clear labeling

#### Scenario: Reset button hidden when default
**GIVEN** no custom download directory is set
**WHEN** the Downloads preferences page is displayed
**THEN** the "Reset to Default" button is hidden or disabled
**AND** the UI clearly indicates default is in use

#### Scenario: Resetting to default directory
**GIVEN** a custom directory is configured
**WHEN** the user clicks "Reset to Default"
**THEN** the custom-download-directory setting is set to empty string
**AND** the UI updates to show "Default (Downloads)"
**AND** the "Reset to Default" button becomes hidden/disabled

### Requirement: Download Notifications Setting
The download preferences SHALL provide a toggle for enabling/disabling download completion notifications.

#### Scenario: Download notifications toggle display
**GIVEN** the Downloads preferences page is open
**WHEN** the Notifications section is rendered
**THEN** an AdwActionRow with switch shows "Show Download Notifications"
**AND** the switch reflects the current download-notifications-enabled setting
**AND** a subtitle explains "Show a notification when downloads complete"

#### Scenario: Enabling download notifications
**GIVEN** download notifications are currently disabled
**WHEN** the user toggles the switch to ON
**THEN** the download-notifications-enabled setting is set to true
**AND** the switch UI updates immediately
**AND** future downloads will show toast notifications

#### Scenario: Disabling download notifications
**GIVEN** download notifications are currently enabled
**WHEN** the user toggles the switch to OFF
**THEN** the download-notifications-enabled setting is set to false
**AND** the switch UI updates immediately
**AND** future downloads will not show toast notifications

### Requirement: GSettings Schema Extensions
The application's GSettings schema SHALL include keys for download directory and notification preferences.

#### Scenario: Custom download directory schema key
**GIVEN** the gschema.xml.in file is processed
**WHEN** the schema is compiled
**THEN** a key "custom-download-directory" of type string exists
**AND** the default value is empty string
**AND** the summary is "Custom download directory"
**AND** the description explains empty means default xdg-download

#### Scenario: Download notifications schema key
**GIVEN** the gschema.xml.in file is processed
**WHEN** the schema is compiled
**THEN** a key "download-notifications-enabled" of type boolean exists
**AND** the default value is true
**AND** the summary is "Enable download notifications"
**AND** the description explains toast notification behavior

### Requirement: Preferences UI Layout
The download preferences UI SHALL follow LibAdwaita design patterns and GNOME HIG.

#### Scenario: AdwPreferencesPage structure
**GIVEN** the preferences.blp file is compiled
**WHEN** the Downloads page is rendered
**THEN** it uses AdwPreferencesPage as the container
**AND** contains AdwPreferencesGroup elements for sections
**AND** uses AdwActionRow for individual settings

#### Scenario: Responsive layout
**GIVEN** the preferences window is resized
**WHEN** the Downloads page is visible
**THEN** the layout adapts responsively
**AND** long paths are wrapped or ellipsized appropriately
**AND** controls remain accessible at minimum window size

#### Scenario: Accessibility labels
**GIVEN** a screen reader is active
**WHEN** the Downloads preferences page is focused
**THEN** all interactive elements have accessible labels
**AND** the directory path is announced correctly
**AND** button purposes are clear

### Requirement: Integration with PreferencesDialog
The download preferences page SHALL be integrated into the existing PreferencesDialog navigation structure.

#### Scenario: Navigation to downloads page
**GIVEN** the Preferences dialog is open on any page
**WHEN** the user clicks "Downloads" in the sidebar
**THEN** the Downloads page is displayed
**AND** the previous page is hidden
**AND** the Downloads item is highlighted in sidebar

#### Scenario: Preferences dialog initialization
**GIVEN** PreferencesDialog.vala is instantiated
**WHEN** setup_preferences_pages() is called
**THEN** the Downloads page is added to the navigation
**AND** UI elements are bound to GSettings
**AND** signal handlers for buttons are connected
