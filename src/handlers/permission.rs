use cef::{
    self, Browser, CefString, Frame, ImplMediaAccessCallback, ImplPermissionHandler,
    ImplPermissionPromptCallback, MediaAccessCallback, PermissionHandler, PermissionPromptCallback,
    PermissionRequestResult, WrapPermissionHandler, rc::Rc, wrap_permission_handler,
};

use gettextrs::gettext;

use crate::permissions_store::{self, Decision, Kind};

#[derive(Clone, Default)]
pub struct ShellPermissionHandler;

wrap_permission_handler! {
    pub struct ShellPermissionHandlerBuilder {
        handler: ShellPermissionHandler,
    }

    impl PermissionHandler {
        fn on_request_media_access_permission(
            &self,
            browser: Option<&mut Browser>,
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

            let browser = browser.map(|b| b.clone());

            // Remembered decision short-circuits the dialog.
            match permissions_store::get(&origin, Kind::Media, mask) {
                Decision::Allow => {
                    mirror_media_content_setting(browser.as_ref(), &origin, mask, true);
                    cb.cont(mask);
                    return 1;
                }
                Decision::Deny => {
                    mirror_media_content_setting(browser.as_ref(), &origin, mask, false);
                    cb.cont(0);
                    return 1;
                }
                Decision::AskAll | Decision::AskMixed => {}
            }

            // Trampoline the prompt onto the glib main loop so AdwAlertDialog
            // runs after the callback returns (CEF UI thread = glib main thread
            // under external_message_pump).
            let cb_clone = cb.clone();
            glib::MainContext::default().spawn_local(async move {
                let choice = prompt_user(&origin, Kind::Media, mask).await;
                // Dismissed: deny this one request, remember nothing.
                let Some(allow) = choice else {
                    cb_clone.cont(0);
                    return;
                };
                persist(&origin, Kind::Media, mask, allow);
                mirror_media_content_setting(browser.as_ref(), &origin, mask, allow);
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

            // M14 kill-switch (6.3): deny a notification request outright when
            // notifications/messages are globally off, ignoring any stored
            // per-origin decision.
            if requests_notifications(mask) && !notifications_allowed() {
                // DISMISS, not DENY: a deny is persisted in the profile and would
                // outlive Do Not Disturb.
                cb.cont(PermissionRequestResult::DISMISS);
                return 1;
            }

            // Unstored origin → AskAll → falls through to `prompt_user` (default
            // = deny). Never auto-allow notifications on first visit (M14 1.1).
            match permissions_store::get(&origin, Kind::Prompt, mask) {
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
                let choice = prompt_user(&origin, Kind::Prompt, mask).await;
                // Dismissed: deny this one request, remember nothing.
                if let Some(allow) = choice {
                    persist(&origin, Kind::Prompt, mask, allow);
                }
                cb_clone.cont(match choice {
                    Some(true) => PermissionRequestResult::ACCEPT,
                    Some(false) => PermissionRequestResult::DENY,
                    // DENY would persist a block in the profile; DISMISS does not.
                    None => PermissionRequestResult::DISMISS,
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

/// Mirror a mic/camera decision into Chromium's content settings for `origin`.
///
/// CEF answers media access through this handler, so Chromium's own
/// HostContentSettingsMap never learns the outcome and
/// `navigator.permissions.query({name:"microphone"})` stays "prompt" forever.
/// WhatsApp's in-call unmute preflight (WAWebVoipRecoverMicrophone) re-queries
/// that state after a successful getUserMedia and bails with "permission
/// remains prompt" — the mic can never be unmuted in a call (#182). Writing the
/// setting makes the query report granted/denied like a normal browser.
fn mirror_media_content_setting(browser: Option<&Browser>, origin: &str, mask: u32, allow: bool) {
    use cef::{
        ContentSettingTypes, ContentSettingValues, ImplBrowser, ImplBrowserHost, ImplRequestContext,
    };

    let Some(ctx) = browser
        .and_then(|b| b.host())
        .and_then(|h| h.request_context())
    else {
        log::warn!("permission: no request context to mirror media setting for {origin}");
        return;
    };
    let value = if allow {
        ContentSettingValues::ALLOW
    } else {
        ContentSettingValues::BLOCK
    };
    let url = CefString::from(origin);
    if requests_microphone(mask) {
        ctx.set_content_setting(
            Some(&url),
            Some(&url),
            ContentSettingTypes::MEDIASTREAM_MIC,
            value,
        );
    }
    if requests_camera(mask) {
        ctx.set_content_setting(
            Some(&url),
            Some(&url),
            ContentSettingTypes::MEDIASTREAM_CAMERA,
            value,
        );
    }
    log::info!(
        "permission: mirrored media content setting for {origin} mask={mask:#x} allow={allow}"
    );
}

/// Record an explicit Allow/Deny so the prompt never re-fires for that origin +
/// permission.
fn persist(origin: &str, kind: Kind, mask: u32, allow: bool) {
    let decision = if allow {
        Decision::Allow
    } else {
        Decision::Deny
    };
    permissions_store::set(origin, kind, mask, decision);
}

/// Show the permission dialog. `Some(allow)` for an explicit choice, `None` when
/// the dialog was dismissed (Escape / close) without choosing.
async fn prompt_user(origin: &str, kind: Kind, requested: u32) -> Option<bool> {
    use libadwaita as adw;
    use libadwaita::prelude::*;

    let perms = describe_permissions(kind, requested);
    let body = gettext("{origin} is requesting access to {perms}.")
        .replace("{origin}", origin)
        .replace("{perms}", &perms);

    let dialog = adw::AlertDialog::new(Some(&gettext("Permission request")), Some(&body));
    dialog.add_response("deny", &gettext("Deny"));
    dialog.add_response("allow", &gettext("Allow"));
    dialog.set_response_appearance("allow", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("deny"));
    // No explicit close response: dismissal yields the default "close" id.

    let parent = active_window();
    match dialog.choose_future(parent.as_ref()).await.as_str() {
        "allow" => Some(true),
        "deny" => Some(false),
        _ => None,
    }
}

/// True when `mask` includes the notifications permission bit.
fn requests_notifications(mask: u32) -> bool {
    use cef::sys::cef_permission_request_types_t as P;
    mask & (P::CEF_PERMISSION_TYPE_NOTIFICATIONS as u32) != 0
}

/// Whether notification permission may be granted at all: false when either M14
/// toggle (`notifications-enabled`, `notify-messages`) is off (6.3).
fn notifications_allowed() -> bool {
    use gtk::prelude::SettingsExt;
    let settings = gio::Settings::new(crate::application::APP_ID);
    settings.boolean("notifications-enabled") && settings.boolean("notify-messages")
}

fn active_window() -> Option<gtk::Window> {
    use gtk::prelude::*;
    let app = gio::Application::default()?
        .downcast::<gtk::Application>()
        .ok()?;
    app.active_window()
}

/// Media-access bits (`cef_media_access_permission_types_t`).
const MEDIA_MIC: u32 = 1;
const MEDIA_CAMERA: u32 = 2;
const MEDIA_DESKTOP_AUDIO: u32 = 4;
const MEDIA_DESKTOP_VIDEO: u32 = 8;

/// True when a media-access `mask` requests the microphone.
fn requests_microphone(mask: u32) -> bool {
    use cef::sys::cef_media_access_permission_types_t as M;
    mask & (M::CEF_MEDIA_PERMISSION_DEVICE_AUDIO_CAPTURE as u32) != 0
}

/// True when a media-access `mask` requests the camera.
fn requests_camera(mask: u32) -> bool {
    use cef::sys::cef_media_access_permission_types_t as M;
    mask & (M::CEF_MEDIA_PERMISSION_DEVICE_VIDEO_CAPTURE as u32) != 0
}

/// Human-readable list of what `mask` (of the given request `kind`) asks for.
pub(crate) fn describe_permissions(kind: Kind, mask: u32) -> String {
    use cef::sys::cef_permission_request_types_t as P;
    let mut parts: Vec<String> = Vec::new();
    match kind {
        Kind::Media => {
            if mask & MEDIA_MIC != 0 {
                parts.push(gettext("microphone"));
            }
            if mask & MEDIA_CAMERA != 0 {
                parts.push(gettext("camera"));
            }
            if mask & MEDIA_DESKTOP_AUDIO != 0 {
                parts.push(gettext("screen audio"));
            }
            if mask & MEDIA_DESKTOP_VIDEO != 0 {
                parts.push(gettext("screen sharing"));
            }
        }
        Kind::Prompt => {
            if mask & (P::CEF_PERMISSION_TYPE_MIC_STREAM as u32) != 0 {
                parts.push(gettext("microphone"));
            }
            if mask & (P::CEF_PERMISSION_TYPE_CAMERA_STREAM as u32) != 0 {
                parts.push(gettext("camera"));
            }
            if mask & (P::CEF_PERMISSION_TYPE_GEOLOCATION as u32) != 0 {
                parts.push(gettext("location"));
            }
            if mask & (P::CEF_PERMISSION_TYPE_NOTIFICATIONS as u32) != 0 {
                parts.push(gettext("notifications"));
            }
            if mask & (P::CEF_PERMISSION_TYPE_MIDI_SYSEX as u32) != 0 {
                parts.push(gettext("MIDI devices"));
            }
            if mask & (P::CEF_PERMISSION_TYPE_CLIPBOARD as u32) != 0 {
                parts.push(gettext("clipboard"));
            }
        }
    }
    if parts.is_empty() {
        return gettext("device access");
    }
    parts.join(", ")
}

/// Friendly single-permission label for the Privacy preferences list. `key` is
/// a stored key as returned by `permissions_store::entries` (kind flag + bit).
pub(crate) fn permission_label(key: u32) -> String {
    use cef::sys::cef_permission_request_types_t as P;
    let bit = key & !(permissions_store::MEDIA_FLAG | permissions_store::PROMPT_FLAG);
    if key & permissions_store::MEDIA_FLAG != 0 {
        return match bit {
            MEDIA_MIC => gettext("Microphone access"),
            MEDIA_CAMERA => gettext("Camera access"),
            MEDIA_DESKTOP_AUDIO => gettext("Screen audio access"),
            MEDIA_DESKTOP_VIDEO => gettext("Screen sharing"),
            _ => gettext("Site access"),
        };
    }
    if bit == P::CEF_PERMISSION_TYPE_MIC_STREAM as u32 {
        return gettext("Microphone access");
    }
    if bit == P::CEF_PERMISSION_TYPE_CAMERA_STREAM as u32 {
        return gettext("Camera access");
    }
    if bit == P::CEF_PERMISSION_TYPE_NOTIFICATIONS as u32 {
        return gettext("Notifications");
    }
    if bit == P::CEF_PERMISSION_TYPE_GEOLOCATION as u32 {
        return gettext("Location access");
    }
    if bit == P::CEF_PERMISSION_TYPE_CLIPBOARD as u32 {
        return gettext("Clipboard access");
    }
    if bit == P::CEF_PERMISSION_TYPE_MIDI_SYSEX as u32 {
        return gettext("MIDI device access");
    }
    gettext("Site access")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions_store::{MEDIA_FLAG, PROMPT_FLAG};

    #[test]
    fn media_bits_match_cef() {
        use cef::sys::cef_media_access_permission_types_t as M;
        assert_eq!(M::CEF_MEDIA_PERMISSION_DEVICE_AUDIO_CAPTURE as u32, MEDIA_MIC);
        assert_eq!(M::CEF_MEDIA_PERMISSION_DEVICE_VIDEO_CAPTURE as u32, MEDIA_CAMERA);
        assert_eq!(M::CEF_MEDIA_PERMISSION_DESKTOP_AUDIO_CAPTURE as u32, MEDIA_DESKTOP_AUDIO);
        assert_eq!(M::CEF_MEDIA_PERMISSION_DESKTOP_VIDEO_CAPTURE as u32, MEDIA_DESKTOP_VIDEO);
    }

    #[test]
    fn describe_media() {
        assert_eq!(describe_permissions(Kind::Media, 1), "microphone");
        assert_eq!(describe_permissions(Kind::Media, 3), "microphone, camera");
        assert_eq!(describe_permissions(Kind::Media, 12), "screen audio, screen sharing");
    }

    #[test]
    fn describe_prompt_does_not_reuse_media_meaning() {
        // Prompt bit 1 is AR_SESSION, not the microphone.
        assert_eq!(describe_permissions(Kind::Prompt, 1), "device access");
        assert_eq!(describe_permissions(Kind::Prompt, 32768), "notifications");
        assert_eq!(describe_permissions(Kind::Prompt, 4), "camera");
        assert_eq!(describe_permissions(Kind::Media, 0), "device access");
    }

    #[test]
    fn labels_respect_kind() {
        assert_eq!(permission_label(MEDIA_FLAG | 1), "Microphone access");
        assert_eq!(permission_label(MEDIA_FLAG | 2), "Camera access");
        assert_eq!(permission_label(MEDIA_FLAG | 4), "Screen audio access");
        assert_eq!(permission_label(MEDIA_FLAG | 8), "Screen sharing");
        assert_eq!(permission_label(PROMPT_FLAG | 1), "Site access");
        assert_eq!(permission_label(PROMPT_FLAG | 32768), "Notifications");
    }

    #[test]
    fn mirror_only_for_real_mic_camera() {
        assert!(requests_microphone(1) && !requests_camera(1));
        assert!(requests_camera(2) && !requests_microphone(2));
        assert!(!requests_microphone(12) && !requests_camera(12));
    }
}
