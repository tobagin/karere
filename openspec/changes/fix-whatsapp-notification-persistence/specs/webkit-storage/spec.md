# Spec: WebKit Storage Management

## Overview

This spec defines how Karere manages WebKit storage (localStorage, IndexedDB, cookies, cache) to ensure data persistence, privacy controls, and proper integration with the Flatpak sandbox.

## ADDED Requirements

### Requirement: Storage Location Specification

The application MUST define and use consistent storage locations for WebKit data within the Flatpak sandbox.

#### Scenario: Data directory structure

**GIVEN** the app is running in a Flatpak environment
**WHEN** WebKit storage is initialized
**THEN** the base data directory MUST be `~/.var/app/<app-id>/data/webkitgtk/websitedata/`
**AND** the base cache directory MUST be `~/.var/app/<app-id>/cache/webkitgtk/`
**AND** subdirectories for localStorage, IndexedDB, and cookies MUST be created under the data directory
**AND** cache files MUST be stored under the cache directory

#### Scenario: Development vs production storage isolation

**GIVEN** the user has both production and development builds installed
**WHEN** running the development build (io.github.tobagin.karere.Devel)
**THEN** storage MUST be isolated to `~/.var/app/io.github.tobagin.karere.Devel/`
**AND** production storage in `~/.var/app/io.github.tobagin.karere/` MUST remain untouched
**AND** both versions MUST maintain separate WhatsApp Web sessions

### Requirement: Storage Type Configuration

The application MUST configure all relevant WebKit storage types for WhatsApp Web functionality.

#### Scenario: Enable persistent storage types

**GIVEN** the WebsiteDataManager is being configured
**WHEN** storage types are enabled
**THEN** localStorage MUST be enabled and persistent
**AND** IndexedDB MUST be enabled and persistent
**AND** cookies MUST be enabled and persistent
**AND** offline web application cache MUST be configurable via settings
**AND** service workers MAY be enabled for progressive web app features

#### Scenario: Verify storage capabilities

**GIVEN** the WebView has loaded WhatsApp Web
**WHEN** WhatsApp Web queries available storage APIs
**THEN** `window.localStorage` MUST be available and functional
**AND** `window.indexedDB` MUST be available and functional
**AND** cookie storage MUST be available and functional
**AND** storage quota MUST be sufficient for typical WhatsApp Web usage

### Requirement: Storage Persistence Guarantees

The application MUST ensure stored data survives across sessions and system events.

#### Scenario: Data persists across app restarts

**GIVEN** WhatsApp Web has saved data to localStorage/IndexedDB
**WHEN** the user closes Karere completely
**AND** the user reopens Karere
**THEN** all localStorage data MUST be restored
**AND** all IndexedDB data MUST be restored
**AND** all cookies MUST be restored
**AND** WhatsApp Web MUST load with previous state intact

#### Scenario: Data persists across system reboots

**GIVEN** WhatsApp Web has saved data to storage
**WHEN** the system is rebooted
**AND** the user launches Karere
**THEN** all storage data MUST still be present
**AND** WhatsApp Web MUST load with previous state intact

#### Scenario: Data integrity after crashes

**GIVEN** WhatsApp Web has saved data to storage
**WHEN** Karere crashes or is force-killed
**AND** the user relaunches Karere
**THEN** storage data MUST remain intact
**AND** no corruption MUST occur
**AND** WhatsApp Web MUST function normally (WebKit handles transaction safety)

### Requirement: Storage Privacy Controls

The application MUST provide privacy controls for managing stored web data.

#### Scenario: Clear all data

**GIVEN** the user wants to reset their WhatsApp Web state
**WHEN** a "clear all data" operation is triggered
**THEN** all localStorage data MUST be removed
**AND** all IndexedDB data MUST be removed
**AND** all cookies MUST be removed
**AND** all cache files MUST be removed
**AND** the user MUST be logged out of WhatsApp Web

#### Scenario: Selective data clearing

**GIVEN** the user wants to clear only cache
**WHEN** a "clear cache" operation is triggered
**THEN** only cache files MUST be removed
**AND** localStorage MUST remain intact
**AND** IndexedDB MUST remain intact
**AND** cookies MUST remain intact
**AND** the user MUST remain logged in to WhatsApp Web

### Requirement: Storage Monitoring and Limits

The application MUST respect quota limits and MAY provide storage monitoring capabilities.

#### Scenario: Log storage usage on startup

**GIVEN** Karere is starting up
**WHEN** the WebView is initialized
**THEN** a debug message MAY log the current storage usage
**AND** the log MAY include size of localStorage, IndexedDB, and cache

#### Scenario: Handle quota exceeded gracefully

**GIVEN** WhatsApp Web attempts to store more data than the quota allows
**WHEN** the quota is exceeded
**THEN** WebKit MUST throw a quota exceeded error to WhatsApp Web
**AND** Karere MUST NOT crash
**AND** a warning MAY be logged about quota limits
**AND** WhatsApp Web MUST handle the error gracefully (truncate or notify user)

### Requirement: Storage Security

The application MUST ensure storage data is protected and isolated.

#### Scenario: Storage isolation from other apps

**GIVEN** multiple Flatpak apps are installed
**WHEN** Karere stores data
**THEN** storage MUST be isolated within Karere's Flatpak sandbox
**AND** other apps MUST NOT have access to Karere's WebKit storage
**AND** Karere MUST NOT have access to other apps' storage

#### Scenario: HTTPS-only storage access

**GIVEN** WhatsApp Web is loaded via HTTPS
**WHEN** WhatsApp Web accesses localStorage/IndexedDB
**THEN** storage MUST be scoped to the HTTPS origin
**AND** HTTP sites (if any) MUST NOT access the HTTPS storage
**AND** WebKit MUST enforce same-origin policy

### Requirement: Storage Cleanup on Uninstall

The application MUST follow Flatpak conventions for data cleanup.

#### Scenario: Data remains after uninstall (Flatpak default)

**GIVEN** the user uninstalls Karere via Flatpak
**WHEN** the uninstallation completes
**THEN** WebKit storage data MAY remain in `~/.var/app/<app-id>/`
**AND** the user MAY manually delete the directory to remove all data
**AND** this is standard Flatpak behavior

#### Scenario: Fresh install uses clean storage

**GIVEN** the user previously uninstalled Karere and deleted `~/.var/app/<app-id>/`
**WHEN** the user reinstalls Karere
**THEN** WebKit storage MUST start fresh
**AND** the user MUST log in to WhatsApp Web again
**AND** no old data MUST be present

### Requirement: Storage Migration and Upgrades

The application MUST handle storage format changes gracefully.

#### Scenario: WebKit storage format upgrade

**GIVEN** WebKitGTK is updated to a new version with storage format changes
**WHEN** Karere runs with the updated WebKitGTK
**THEN** WebKit MUST handle format migration automatically
**AND** user data MUST be preserved
**AND** no manual migration code MUST be required in Karere

#### Scenario: Corrupted storage recovery

**GIVEN** WebKit storage files become corrupted
**WHEN** Karere attempts to load the data
**THEN** WebKit MAY detect corruption and log warnings
**AND** WebKit MAY reset corrupted databases
**AND** Karere MUST continue to function
**AND** the user MAY need to log in again (acceptable for corruption recovery)

### Requirement: Developer and Debug Support

The application MUST provide storage inspection capabilities for development.

#### Scenario: Enable storage inspection with developer tools

**GIVEN** developer tools are enabled in preferences
**WHEN** the user opens WebKit Inspector
**THEN** the Storage tab MUST show localStorage data
**AND** the Storage tab MUST show IndexedDB data
**AND** the Storage tab MUST show cookies
**AND** the developer can inspect and modify storage for debugging

#### Scenario: Log storage paths for troubleshooting

**GIVEN** Karere is running with debug logging enabled
**WHEN** storage is initialized
**THEN** debug messages MUST log the full paths to storage directories
**AND** debug messages MUST indicate whether storage is working correctly
**AND** users can provide these logs for troubleshooting

### Requirement: Performance Optimization

The application MUST ensure storage operations do not block the UI and MAY implement additional performance optimizations.

#### Scenario: Async storage operations

**GIVEN** WhatsApp Web accesses localStorage or IndexedDB
**WHEN** storage operations are performed
**THEN** operations MUST be asynchronous where possible
**AND** the UI MUST NOT block during storage I/O
**AND** WebKit MUST handle storage I/O in background threads

#### Scenario: Storage caching

**GIVEN** WhatsApp Web frequently accesses the same storage keys
**WHEN** storage is accessed multiple times
**THEN** WebKit MAY cache frequently accessed data in memory
**AND** write operations MUST still be persisted to disk
**AND** cache MUST remain consistent with disk storage
