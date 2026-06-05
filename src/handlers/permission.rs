use cef::{
    self, Browser, CefString, Frame, ImplMediaAccessCallback, ImplPermissionHandler,
    ImplPermissionPromptCallback, MediaAccessCallback, PermissionHandler, PermissionPromptCallback,
    PermissionRequestResult, WrapPermissionHandler, rc::Rc, wrap_permission_handler,
};

use crate::permissions_store::{self, Decision};

#[derive(Clone, Default)]
pub struct ShellPermissionHandler;

wrap_permission_handler! {
    pub struct ShellPermissionHandlerBuilder {
        handler: ShellPermissionHandler,
    }

    impl PermissionHandler {
        fn on_request_media_access_permission(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            requesting_origin: Option<&CefString>,
            requested_permissions: u32,
            callback: Option<&mut MediaAccessCallback>,
        ) -> ::std::os::raw::c_int {
            let origin = requesting_origin
                .map(|s| s.to_string())
                .unwrap_or_default();
            let mask = requested_permissions;
            let Some(cb) = callback else { return 0 };

            // A remembered decision short-circuits the dialog.
            match permissions_store::get(&origin, mask) {
                Decision::Allow => {
                    cb.cont(mask);
                    return 1;
                }
                Decision::Deny => {
                    cb.cont(0);
                    return 1;
                }
                Decision::AskAll | Decision::AskMixed => {}
            }

            // Trampoline the prompt onto the glib main loop. The CEF UI thread
            // is the glib main thread under external_message_pump but we still
            // want a fresh dispatch so AdwAlertDialog runs after the callback
            // returns.
            let cb_clone = cb.clone();
            glib::MainContext::default().spawn_local(async move {
                let allow = prompt_user(&origin, mask).await;
                persist(&origin, mask, allow);
                cb_clone.cont(if allow { mask } else { 0 });
            });
            1
        }

        fn on_show_permission_prompt(
            &self,
            _browser: Option<&mut Browser>,
            _prompt_id: u64,
            requesting_origin: Option<&CefString>,
            requested_permissions: u32,
            callback: Option<&mut PermissionPromptCallback>,
        ) -> ::std::os::raw::c_int {
            let origin = requesting_origin
                .map(|s| s.to_string())
                .unwrap_or_default();
            let mask = requested_permissions;
            let Some(cb) = callback else { return 0 };

            // M14 kill-switch (6.3): a notification request is denied outright
            // when notifications are globally off or message notifications are
            // disabled, regardless of any stored per-origin decision.
            if requests_notifications(mask) && !notifications_allowed() {
                cb.cont(PermissionRequestResult::DENY);
                return 1;
            }

            // The CEF_PERMISSION_TYPE_NOTIFICATIONS branch reaches here with no
            // preselected outcome: an unstored origin resolves to AskAll, which
            // falls through to `prompt_user` (default response = deny). We never
            // auto-allow notifications on first visit (M14 1.1).
            match permissions_store::get(&origin, mask) {
                Decision::Allow => {
                    cb.cont(PermissionRequestResult::ACCEPT);
                    return 1;
                }
                Decision::Deny => {
                    cb.cont(PermissionRequestResult::DENY);
                    return 1;
                }
                Decision::AskAll | Decision::AskMixed => {}
            }

            let cb_clone = cb.clone();
            glib::MainContext::default().spawn_local(async move {
                let allow = prompt_user(&origin, mask).await;
                persist(&origin, mask, allow);
                cb_clone.cont(if allow {
                    PermissionRequestResult::ACCEPT
                } else {
                    PermissionRequestResult::DENY
                });
            });
            1
        }
    }
}

impl ShellPermissionHandlerBuilder {
    pub fn build() -> PermissionHandler {
        Self::new(ShellPermissionHandler)
    }
}

/// Record the user's choice. Decisions are always remembered (browser-style),
/// so the prompt never re-fires for that origin + permission.
fn persist(origin: &str, mask: u32, allow: bool) {
    let decision = if allow { Decision::Allow } else { Decision::Deny };
    permissions_store::set(origin, mask, decision);
}

/// Show the permission dialog. Returns whether the user allowed.
async fn prompt_user(origin: &str, requested: u32) -> bool {
    use libadwaita as adw;
    use libadwaita::prelude::*;

    let perms = describe_permissions(requested);
    let body = format!("{origin} is requesting access to {perms}.");

    let dialog = adw::AlertDialog::new(Some("Permission request"), Some(&body));
    dialog.add_response("deny", "Deny");
    dialog.add_response("allow", "Allow");
    dialog.set_response_appearance("allow", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("deny"));
    dialog.set_close_response("deny");

    let parent = active_window();
    dialog.choose_future(parent.as_ref()).await == "allow"
}

/// True when `mask` includes the notifications permission bit.
fn requests_notifications(mask: u32) -> bool {
    use cef::sys::cef_permission_request_types_t as P;
    mask & (P::CEF_PERMISSION_TYPE_NOTIFICATIONS as u32) != 0
}

/// Whether notification permission may be granted at all, per the M14 gschema
/// toggles: the global kill-switch (`notifications-enabled`) and the message
/// notification toggle (`notify-messages`). Either being false denies the
/// request (6.3).
fn notifications_allowed() -> bool {
    use gtk::prelude::SettingsExt;
    let settings = gio::Settings::new(crate::application::APP_ID);
    settings.boolean("notifications-enabled") && settings.boolean("notify-messages")
}

fn active_window() -> Option<gtk::Window> {
    use gtk::prelude::*;
    let app = gio::Application::default()?.downcast::<gtk::Application>().ok()?;
    app.active_window()
}

/// True when `mask` requests microphone access. Mic/cam arrive via
/// `on_request_media_access_permission` (the media-access bitset) — a DIFFERENT
/// enum from the permission-prompt types — so check both.
fn requests_microphone(mask: u32) -> bool {
    use cef::sys::{cef_media_access_permission_types_t as M, cef_permission_request_types_t as P};
    mask & (M::CEF_MEDIA_PERMISSION_DEVICE_AUDIO_CAPTURE as u32) != 0
        || mask & (P::CEF_PERMISSION_TYPE_MIC_STREAM as u32) != 0
}

/// True when `mask` requests camera access (media-access or prompt enum).
fn requests_camera(mask: u32) -> bool {
    use cef::sys::{cef_media_access_permission_types_t as M, cef_permission_request_types_t as P};
    mask & (M::CEF_MEDIA_PERMISSION_DEVICE_VIDEO_CAPTURE as u32) != 0
        || mask & (P::CEF_PERMISSION_TYPE_CAMERA_STREAM as u32) != 0
}

pub(crate) fn describe_permissions(mask: u32) -> String {
    use cef::sys::cef_permission_request_types_t as P;
    let mut parts = Vec::new();
    if requests_microphone(mask) { parts.push("microphone"); }
    if requests_camera(mask) { parts.push("camera"); }
    if mask & (P::CEF_PERMISSION_TYPE_GEOLOCATION as u32) != 0 { parts.push("location"); }
    if mask & (P::CEF_PERMISSION_TYPE_NOTIFICATIONS as u32) != 0 { parts.push("notifications"); }
    if mask & (P::CEF_PERMISSION_TYPE_MIDI_SYSEX as u32) != 0 { parts.push("MIDI devices"); }
    if mask & (P::CEF_PERMISSION_TYPE_CLIPBOARD as u32) != 0 { parts.push("clipboard"); }
    if parts.is_empty() { return "device access".into(); }
    parts.join(", ")
}

/// Friendly single-permission label for the Privacy preferences list (one row
/// per stored bit). Distinguishes microphone vs camera (the media-access bits)
/// instead of the generic "device access".
pub(crate) fn permission_label(mask: u32) -> String {
    use cef::sys::cef_permission_request_types_t as P;
    if requests_microphone(mask) {
        return "Microphone access".into();
    }
    if requests_camera(mask) {
        return "Camera access".into();
    }
    if mask & (P::CEF_PERMISSION_TYPE_NOTIFICATIONS as u32) != 0 {
        return "Notifications".into();
    }
    if mask & (P::CEF_PERMISSION_TYPE_GEOLOCATION as u32) != 0 {
        return "Location access".into();
    }
    if mask & (P::CEF_PERMISSION_TYPE_CLIPBOARD as u32) != 0 {
        return "Clipboard access".into();
    }
    if mask & (P::CEF_PERMISSION_TYPE_MIDI_SYSEX as u32) != 0 {
        return "MIDI device access".into();
    }
    "Site access".into()
}
