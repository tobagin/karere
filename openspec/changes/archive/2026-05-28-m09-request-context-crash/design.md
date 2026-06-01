## Context

After M07 retargeted the shell to Karere and M08 added window-state persistence and `GAction` plumbing, the embedded WhatsApp Web surface still differs from Karere v3 in four user-visible ways: external links navigate inside the shell, renderer crashes leave a frozen `GLArea`, the right-click menu offers "Open in New Window/Tab/Incognito" entries that never go anywhere, and a transient network failure during the initial load is fatal. Karere v3 solves each in `window.rs` (lines 2047-2188) by hooking WebKitGTK's `decide-policy`, `web-process-terminated`, `context-menu`, and `load-failed` signals. CEF exposes the same hooks via dedicated handler interfaces: `RequestHandler`, `ContextMenuHandler`, and the already-wired `LoadHandler`. M09 stands those handlers up and feeds their results into `SharedState` so the GTK side can react on its own thread.

## Goals / Non-Goals

**Goals:**
- Top-level navigations to non-WhatsApp origins open via `gio::AppInfo::launch_default_for_uri` and cancel inside the shell.
- Renderer terminations show a toast within ~1 s and self-reload up to a 5-per-60-s threshold, after which an `AdwAlertDialog` offers an "Open logs" action.
- The link context menu omits `MENU_ID_OPEN_LINK_NEW_WINDOW`, `MENU_ID_OPEN_LINK_NEW_TAB`, and `MENU_ID_OPEN_LINK_IN_INCOGNITO_WINDOW`, including the separators that wrap them.
- Load failures (excluding `ERR_ABORTED`) retry with `min(60_000, 500 * 2.pow(count))` ms backoff and reset on `on_load_end`.

**Non-Goals:**
- Cookie or cache management (deferred to M20 multi-account).
- Permission persistence (M11).
- Find-in-page integration (M10).
- Per-account crash telemetry beyond the in-process counter.

## Decisions

- **Allowlist by host + scheme.** `on_before_browse` parses the URL once and matches against `whatsapp.com`, `whatsapp.net`, `web.whatsapp.com`, plus the inert schemes `data:`, `blob:`, `about:`, `file:`, `chrome-error:`. Anything else returns 1 (cancel) after handing the URI to `gio::AppInfo::launch_default_for_uri(url, None::<&gio::AppLaunchContext>)`. We treat `user_gesture == false` the same as `true`; Karere v3 also routes both.
- **Crash recovery on the CEF UI thread.** `on_render_process_terminated` writes a `crash_toast` string and bumps a `(timestamp, count)` ring on `SharedState`. The handler schedules `glib::timeout_add_local(Duration::from_millis(1500), …)` to invoke `browser.reload()`. The 100 ms poller introduced in M8 drains `crash_toast` and posts it through the toast overlay. When the ring shows 5+ crashes in 60 s, the poller raises an `AdwAlertDialog` instead of another reload.
- **Context menu walk.** CEF's `MenuModel` exposes ordered access via `count()` and `get_command_id_at(idx)`. We iterate from the top, collecting indices whose `command_id` matches the three forbidden IDs, expand the range to include adjacent separators (`MENU_ID_USER_FIRST` separators only when sandwiched between removed items), then call `model.remove(idx)` in reverse order to keep indices stable. (Exact bindings — `count_at`, `get_command_id_at`, `remove_at` — confirmed against `cef-rs 148` at implementation time.)
- **Load-error backoff uses glib's main loop.** Because CEF dispatches handler callbacks on the main thread (we run with `external_message_pump = false`), we can call `glib::timeout_add_local` directly. The closure captures a clone of the `Browser` handle and calls `browser.reload_ignore_cache()`. We reset `load_error_count` to 0 inside `on_load_end` so a successful load clears the backoff state.
- **Handler ownership stays in `ShellClient`.** `client.rs` gains two new `Arc<…>` fields and exposes them via the `request_handler(&self)` / `context_menu_handler(&self)` overrides. This mirrors how `load_handler` is wired today and keeps the lifetime story uniform.

## Risks / Trade-offs

- **cef-rs 148 menu-model API drift.** The exact method names on `MenuModel` may differ; we will pin them at implementation time and fall back to `set_label_at("")` + `set_enabled_at(false)` if `remove_at` is unavailable.
- **Reload storms.** A pathological page that crashes immediately after reload could spin the 1500 ms reload up to 5 times per minute. The dialog escape hatch caps user-visible churn, but the renderer subprocess will still respawn five times. Acceptable for parity with v3.
- **Timer leakage on close.** `glib::timeout_add_local` IDs need to be tracked or guarded by a weak `Browser` reference so a closing window does not reload a dead browser. We will store the `SourceId` on `SharedState` and remove it from `on_load_end` and from window dispose.
- **External-link UX on Flatpak.** `launch_default_for_uri` goes through xdg-open / portal; on hosts without a default browser, the call silently fails. We log via `tracing::warn!` but do not pop a dialog (matches v3).
- **Scheme allowlist gaps.** WhatsApp Web occasionally navigates to `https://faq.whatsapp.com`; that host matches `whatsapp.com` and stays in-shell — intentional, consistent with v3.
