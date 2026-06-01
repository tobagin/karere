## ADDED Requirements

### Requirement: External-link routing through the default browser
The shell SHALL install a CEF `RequestHandler` that intercepts top-level navigations in `on_before_browse` and routes any URL whose host is not in `{whatsapp.com, whatsapp.net, web.whatsapp.com}` and whose scheme is not `data:`, `blob:`, `about:`, `file:`, or `chrome-error:` to the system default handler via `gio::AppInfo::launch_default_for_uri`, then cancel the in-shell navigation by returning `1`.

#### Scenario: Click an external https link inside WhatsApp Web
- **WHEN** the user clicks a link to `https://google.com` rendered inside the embedded WhatsApp Web view
- **THEN** `on_before_browse` invokes `gio::AppInfo::launch_default_for_uri("https://google.com", None)` and returns `1`, the host's default browser opens the URL, and the embedded view stays on its current `web.whatsapp.com` page

#### Scenario: Click a WhatsApp internal link
- **WHEN** the user clicks a link whose host is `web.whatsapp.com` or `whatsapp.com`
- **THEN** `on_before_browse` returns `0` and the embedded view navigates without invoking the external handler

#### Scenario: Inline blob or data URL
- **WHEN** the embedded view navigates to a `blob:`, `data:`, `about:`, `file:`, or `chrome-error:` URL
- **THEN** `on_before_browse` returns `0` and no external handler is invoked

### Requirement: Renderer crash recovery with backoff and escalation
The shell SHALL handle `on_render_process_terminated` by publishing a `crash_toast` string into `SharedState`, scheduling `browser.reload()` via `glib::timeout_add_local` after 1500 ms, tracking crashes in a 60-second sliding window, and, when 5 or more crashes occur inside that window, replacing the auto-reload with an `AdwAlertDialog` titled "Web view keeps crashing." that offers an "Open logs" action and a "Cancel" action.

#### Scenario: Single renderer crash
- **WHEN** the renderer subprocess terminates once
- **THEN** within ~1 s the toast overlay displays "Web view crashed — reconnecting…" and `browser.reload()` runs 1500 ms after the termination

#### Scenario: Crash storm exceeds threshold
- **WHEN** the renderer terminates 5 times within a single 60-second window
- **THEN** the handler stops auto-reloading and surfaces an `AdwAlertDialog` whose primary action opens the application's log viewer

### Requirement: Route new-window and popup navigations like top-level links
External links frequently open via `target="_blank"`, `window.open`, or programmatic tab opens, which CEF delivers through `LifeSpanHandler::on_before_popup` and `RequestHandler::on_open_urlfrom_tab` rather than `on_before_browse`. The shell SHALL handle both callbacks by cancelling the popup (returning `1` from `on_before_popup`, `1` from `on_open_urlfrom_tab`) and routing the target URL the same way as `on_before_browse`: WhatsApp hosts and inert schemes load in the opener's main frame, every other URL opens in the host's default browser via `gio::AppInfo::launch_default_for_uri`. No second CEF window is ever created.

#### Scenario: Click an external link that targets a new window
- **WHEN** the user clicks a `target="_blank"` link to `https://youtube.com/...` inside the embedded view
- **THEN** `on_before_popup` cancels the popup, the host's default browser opens the URL, and no new CEF window appears

#### Scenario: WhatsApp link targets a new window
- **WHEN** a popup or open-from-tab targets a `web.whatsapp.com` URL
- **THEN** the handler loads that URL into the opener's existing main frame and suppresses the popup, keeping the shell a single window
