# WebView Manager

## ADDED Requirements

### Requirement: WebView Manager Class Creation
The application SHALL provide a `WebViewManager` class in `src/managers/WebViewManager.vala` that encapsulates WebView lifecycle, navigation policy, and developer tools management.

#### Scenario: WebView manager instantiation
**GIVEN** the application creates a new Window
**WHEN** Window.vala instantiates WebViewManager
**THEN** the manager receives Settings and WebKitManager dependencies
**AND** the manager creates a WebKit.WebView instance
**AND** the WebView is not yet added to any container

#### Scenario: WebView setup and container integration
**GIVEN** WebViewManager is instantiated
**WHEN** Window.vala calls setup() with a Gtk.Box container
**THEN** the WebView is configured with WebKitManager settings
**AND** the WebView is added to the provided container
**AND** navigation policy handlers are connected
**AND** load event handlers are connected
**AND** the WebView loads https://web.whatsapp.com

### Requirement: Navigation Policy Management
The WebView manager SHALL handle navigation policy decisions to allow internal WhatsApp URLs and redirect external links to the system browser.

#### Scenario: Internal WhatsApp URL navigation
**GIVEN** a WebView is loaded with WhatsApp Web
**WHEN** a navigation to "https://web.whatsapp.com/some/path" is requested
**THEN** the navigation is allowed within the WebView
**AND** no external browser is opened

#### Scenario: External link navigation
**GIVEN** a WebView is loaded with WhatsApp Web
**WHEN** a navigation to "https://example.com" is requested
**THEN** the navigation is blocked in the WebView
**AND** the external_link_clicked signal is emitted with the URL
**AND** the URL is opened in the system browser via portal

#### Scenario: New window creation for external links
**GIVEN** a WebView is loaded with WhatsApp Web
**WHEN** WhatsApp Web requests a new window for "https://example.com"
**THEN** no new WebView is created
**AND** the URL is opened in the system browser via portal

### Requirement: Load Event Handling
The WebView manager SHALL emit signals for load lifecycle events that Window needs to react to.

#### Scenario: Load started event
**GIVEN** a WebView begins loading a page
**WHEN** the load starts
**THEN** the load_started signal is emitted
**AND** the signal can be connected by Window for UI updates

#### Scenario: Load finished event
**GIVEN** a WebView completes loading a page
**WHEN** the load finishes successfully
**THEN** the load_finished signal is emitted
**AND** WebKitManager injects user agent override
**AND** notification permission setup is triggered

#### Scenario: Load failed event
**GIVEN** a WebView fails to load a page
**WHEN** the load fails with an error
**THEN** the load_failed signal is emitted with URI and error message
**AND** Window can display an error toast

### Requirement: Developer Tools Management
The WebView manager SHALL provide methods to control WebKit Inspector (developer tools).

#### Scenario: Opening developer tools when enabled
**GIVEN** developer tools are enabled in settings
**WHEN** open_developer_tools() is called
**THEN** the WebKit Inspector is shown

#### Scenario: Opening developer tools when disabled
**GIVEN** developer tools are disabled in settings
**WHEN** open_developer_tools() is called
**THEN** the WebKit Inspector is not shown
**AND** the method returns early with a warning

#### Scenario: Checking developer tools state
**GIVEN** the WebKit Inspector is attached
**WHEN** is_developer_tools_open() is called
**THEN** the method returns true

#### Scenario: Closing developer tools
**GIVEN** the WebKit Inspector is attached
**WHEN** close_developer_tools() is called
**THEN** the WebKit Inspector is detached and closed

### Requirement: JavaScript Injection
The WebView manager SHALL provide a clean API for injecting JavaScript into the WebView.

#### Scenario: JavaScript injection with callback
**GIVEN** a WebView is loaded
**WHEN** inject_javascript() is called with a script and callback
**THEN** the script is executed in the WebView context
**AND** the callback is invoked with the result

#### Scenario: JavaScript injection without callback
**GIVEN** a WebView is loaded
**WHEN** inject_javascript() is called with a script and no callback
**THEN** the script is executed in the WebView context
**AND** no callback is invoked

### Requirement: WebView Reload Operations
The WebView manager SHALL provide methods to reload the WebView content.

#### Scenario: Normal reload
**GIVEN** a WebView is loaded
**WHEN** reload(false) is called
**THEN** the WebView reloads using cached content

#### Scenario: Force reload
**GIVEN** a WebView is loaded
**WHEN** reload(true) is called
**THEN** the WebView reloads bypassing the cache

### Requirement: URL Classification
The WebView manager SHALL classify URLs as internal WhatsApp URLs or external links.

#### Scenario: WhatsApp Web domain classification
**GIVEN** a URL "https://web.whatsapp.com/path"
**WHEN** is_whatsapp_internal_uri() is called
**THEN** the method returns true

#### Scenario: WhatsApp static resource classification
**GIVEN** a URL "https://static.whatsapp.net/resource"
**WHEN** is_whatsapp_internal_uri() is called
**THEN** the method returns true

#### Scenario: WhatsApp WebSocket classification
**GIVEN** a URL "wss://web.whatsapp.com/ws"
**WHEN** is_whatsapp_internal_uri() is called
**THEN** the method returns true

#### Scenario: Data URI classification
**GIVEN** a URL "data:image/png;base64,..."
**WHEN** is_whatsapp_internal_uri() is called
**THEN** the method returns true

#### Scenario: External HTTPS URL classification
**GIVEN** a URL "https://example.com"
**WHEN** is_external_link() is called
**THEN** the method returns true
