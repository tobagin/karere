# Spec: WebView Manager Storage Configuration

## Overview

This spec extends the WebView Manager to properly configure WebKit's website data storage, ensuring that WhatsApp Web's preferences and data (localStorage, IndexedDB, cookies) are persisted across application sessions.

## MODIFIED Requirements

### Requirement: WebView Manager Class Creation (EXISTING - Modified)

The application SHALL provide a `WebViewManager` class in `src/managers/WebViewManager.vala` that encapsulates WebView lifecycle, navigation policy, and developer tools management.

**Modification**: The WebViewManager MUST also configure persistent website data storage.

#### Scenario: WebView setup with persistent storage (MODIFIED)

**GIVEN** WebViewManager is instantiated
**WHEN** Window.vala calls setup() with a Gtk.Box container
**THEN** the WebView is configured with WebKitManager settings
**AND** the WebView is configured with a WebsiteDataManager for persistent storage
**AND** localStorage and IndexedDB are enabled for data persistence
**AND** the WebView is added to the provided container
**AND** navigation policy handlers are connected
**AND** load event handlers are connected
**AND** the WebView loads https://web.whatsapp.com

## ADDED Requirements

### Requirement: Website Data Manager Configuration

The WebViewManager MUST configure a `WebKit.WebsiteDataManager` with persistent storage paths to ensure web data survives across sessions.

#### Scenario: Configure persistent storage paths

**GIVEN** WebViewManager is being initialized
**WHEN** the website data manager is configured
**THEN** the base data directory MUST be set to a persistent location within the Flatpak sandbox
**AND** the base cache directory MUST be set to a cache location within the Flatpak sandbox
**AND** localStorage MUST be enabled and persisted
**AND** IndexedDB MUST be enabled and persisted
**AND** a debug message MUST log the storage paths

#### Scenario: Storage paths in Flatpak environment

**GIVEN** the app is running as a Flatpak (production or development)
**WHEN** storage paths are configured
**THEN** the base data directory MUST be `~/.var/app/<app-id>/data/webkitgtk/websitedata`
**AND** the base cache directory MUST be `~/.var/app/<app-id>/cache/webkitgtk`
**AND** the app-id MUST match the current build (io.github.tobagin.karere or io.github.tobagin.karere.Devel)

#### Scenario: WebView creation with data manager

**GIVEN** a website data manager is configured with persistent storage
**WHEN** the WebView is created
**THEN** the WebView MUST be created with the configured WebsiteDataManager
**AND** the WebView's network session MUST use the data manager's storage
**AND** the WebView MUST have access to persistent localStorage and IndexedDB

### Requirement: Storage Verification and Logging

The WebViewManager MUST provide logging to verify storage configuration for debugging purposes.

#### Scenario: Log storage configuration on setup

**GIVEN** the WebViewManager is setting up the WebView
**WHEN** the setup completes
**THEN** a debug message MUST log "WebKit storage configured at: <path>"
**AND** an info message MUST log "localStorage and IndexedDB persistence enabled"

#### Scenario: Verify storage directories exist

**GIVEN** storage paths are configured
**WHEN** the WebView is created
**THEN** the storage directories MUST be created automatically by WebKit if they don't exist
**AND** no manual directory creation MUST be required by the application

### Requirement: Cookie Storage Persistence

The WebViewManager MUST ensure cookies are properly persisted for WhatsApp Web session management.

#### Scenario: Cookie persistence configuration (MODIFIED)

**GIVEN** the WebView is being configured
**WHEN** configure_cookie_storage() is called
**THEN** the method MUST configure the WebsiteDataManager for persistent cookie storage
**AND** cookies MUST be saved to the persistent data directory
**AND** cookies MUST survive application restarts
**AND** an info message MUST log "Cookie persistence enabled via WebsiteDataManager"

#### Scenario: WhatsApp Web session persistence

**GIVEN** a user has logged into WhatsApp Web
**AND** the session cookies are saved
**WHEN** the user closes and reopens Karere
**THEN** the WebView MUST load the saved cookies
**AND** the user MUST remain logged in to WhatsApp Web
**AND** no QR code scan MUST be required (unless session expired on server side)

### Requirement: LocalStorage and IndexedDB Persistence

The WebViewManager MUST ensure localStorage and IndexedDB data are persisted to maintain WhatsApp Web preferences.

#### Scenario: WhatsApp notification preference persistence

**GIVEN** the user enables notifications within WhatsApp Web interface
**AND** WhatsApp Web saves this preference to localStorage or IndexedDB
**WHEN** the user closes and reopens Karere
**THEN** the WebView MUST restore the localStorage/IndexedDB data
**AND** WhatsApp Web MUST remember the notification preference
**AND** the "Message notifications are off" banner MUST NOT appear

#### Scenario: Other WhatsApp preferences persistence

**GIVEN** the user configures settings in WhatsApp Web (e.g., chat archive, mute settings, themes)
**AND** WhatsApp Web saves these to web storage
**WHEN** the user closes and reopens Karere
**THEN** all WhatsApp Web preferences MUST be restored
**AND** the user MUST NOT need to reconfigure settings

### Requirement: Storage Cleanup Support

The WebViewManager MUST support clearing website data for privacy and troubleshooting.

#### Scenario: Clear all website data

**GIVEN** the WebView has accumulated website data
**WHEN** clear_website_data() is called with all data types
**THEN** all localStorage data MUST be cleared
**AND** all IndexedDB data MUST be cleared
**AND** all cookies MUST be cleared
**AND** all cache data MUST be cleared
**AND** a debug message MUST log "All website data cleared"

#### Scenario: Clear specific data types

**GIVEN** the WebView has accumulated website data
**WHEN** clear_website_data() is called with specific types (e.g., cache only)
**THEN** only the specified data types MUST be cleared
**AND** other data types MUST remain intact
**AND** a debug message MUST log which data types were cleared

### Requirement: Backward Compatibility

The storage configuration MUST handle existing installations gracefully.

#### Scenario: First-time storage configuration

**GIVEN** a user is running Karere for the first time
**WHEN** the WebView is initialized
**THEN** the storage directories MUST be created automatically
**AND** no errors MUST occur due to missing directories
**AND** WhatsApp Web MUST function normally

#### Scenario: Migrating from unconfigured storage

**GIVEN** a user previously ran Karere without explicit storage configuration
**AND** WebKit used default storage paths
**WHEN** the updated version runs with explicit storage paths
**THEN** WebKit SHOULD use the new storage paths
**AND** the user MAY need to log in again (acceptable one-time migration cost)
**AND** no crashes or errors MUST occur

### Requirement: Error Handling

The WebViewManager MUST handle storage configuration errors gracefully.

#### Scenario: Storage path unavailable

**GIVEN** the configured storage path cannot be created or accessed
**WHEN** the WebView is initialized
**THEN** WebKit MUST fall back to default storage behavior
**AND** a warning MUST be logged about storage configuration failure
**AND** the application MUST NOT crash
**AND** WhatsApp Web MUST still load (though preferences may not persist)

#### Scenario: Storage quota exceeded

**GIVEN** WebKit storage has reached quota limits
**WHEN** WhatsApp Web attempts to save data
**THEN** WebKit MUST handle the quota error
**AND** a warning MAY be logged about storage limits
**AND** the application MUST continue to function
