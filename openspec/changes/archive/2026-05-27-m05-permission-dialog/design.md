## Context

The shell embeds Chromium content via CEF in offscreen-rendering mode and pumps CEF on the glib main loop (`external_message_pump`). Until this change, no `PermissionHandler` was wired into `ClientBuilder`, so CEF's default behaviour denied every `getUserMedia` and related call before the page could surface a UI. The shell already uses libadwaita for application chrome, so the natural prompt mechanism is `adw::AlertDialog`.

CEF's `on_request_media_access_permission` is invoked on the CEF UI thread with a `MediaAccessCallback`. The callback may be retained and resolved later as long as the handler returns `1` (the C int signalling asynchronous handling).

## Goals / Non-Goals

**Goals:**
- Surface every page-initiated camera / microphone / clipboard / notifications / location request as a libadwaita modal.
- Respect the exact `requested_permissions` bitmask when granting (so we never silently widen a request).
- Keep the handler free of business state — no caching, no persistence, no allowlist.
- Avoid re-entering CEF from inside the handler callback by deferring the dialog to a fresh glib dispatch.

**Non-Goals:**
- Persisting user decisions across requests or sessions.
- Per-origin allowlists or settings UI.
- Implementing `on_show_permission_prompt` (Chromium's separate code path for notifications / midi / geolocation). That belongs to M11.
- Localising the dialog body or permission labels (English-only for now).

## Decisions

- **Return 1 and resolve asynchronously.** The dialog is `async`; returning `0` would force a synchronous decision and lose the user prompt. Returning `1` tells CEF to wait for `MediaAccessCallback::cont`.
- **Trampoline through `glib::MainContext::default().spawn_local`.** Even though the CEF UI thread is the glib main thread under `external_message_pump`, we still hop through `spawn_local` so the AlertDialog runs after the C++ callback returns. This avoids running an async modal inside CEF's stack frame and keeps the dialog modal-friendly.
- **Grant the full requested mask on Allow.** `cb.cont(requested_permissions)` — we do not filter bits we do not recognise. Rationale: CEF asked for exactly that set; selectively dropping bits would risk producing an inconsistent state the page cannot handle, and the user already saw the human description (with fallback `device access` for unknown bits). On Deny we pass `0`.
- **`describe_permissions` returns a joined human string.** Walks the documented `cef_permission_request_types_t` bits (`CAMERA_STREAM`, `MIC_STREAM`, `GEOLOCATION`, `NOTIFICATIONS`, `CLIPBOARD`). If no known bit is set the body reads "device access" so the user is never shown an empty noun phrase.
- **`active_window()` looks up the parent via `gio::Application::default()`.** Keeps `permission.rs` decoupled from `MainWindow`; downcasts to `gtk::Application` and returns `active_window()`. If no GTK application is alive (shouldn't happen while CEF is alive), the dialog parents to `None` and still works as a transient.
- **Wire through `wrap_client!` rather than a separate handler registration.** A new `permission_handler: PermissionHandler` field on the builder struct plus the matching `Client::permission_handler` getter mirrors the existing `RenderHandler` / `LifeSpanHandler` / `DisplayHandler` / `LoadHandler` pattern; `ClientBuilder::build_for` constructs it via `ShellPermissionHandlerBuilder::build()`.

## Risks / Trade-offs

- **Prompt fatigue.** Every request prompts; pages that re-request after a refresh will replay the dialog. Mitigation: M11 will introduce remembered decisions.
- **Unknown permission bits.** New bits added to `cef_permission_request_types_t` upstream will be displayed as "device access" until `describe_permissions` is extended. Allow still grants the full mask; risk is that the user does not realise what they accepted. Mitigation: revisit `describe_permissions` on each CEF bump.
- **Modal parent missing.** If `gio::Application::default()` is not a `gtk::Application` (e.g. during very early startup or late teardown) the dialog parents to `None`. In practice CEF will not issue permission requests in those windows, so the impact is theoretical.
- **No filtering of granted bits.** Granting the full mask is convenient but means we trust CEF's request shape; if Chromium ever batched unrelated permissions into one call we would over-grant. Mitigation: revisit if CEF starts merging requests; document the behaviour here.
