use std::time::{Duration, Instant};

use cef::{
    self, Browser, Callback, CefString, Frame, ImplBrowser, ImplFrame, ImplRequest,
    ImplRequestHandler, ImplResourceRequestHandler, Request, RequestHandler, ResourceRequestHandler,
    ReturnValue, TerminationStatus, WindowOpenDisposition, WrapRequestHandler,
    WrapResourceRequestHandler, rc::Rc, wrap_request_handler, wrap_resource_request_handler,
};

use super::{CrashDialog, SharedRef};

const CRASH_WINDOW: Duration = Duration::from_secs(60);
const CRASH_THRESHOLD: usize = 5;
const RELOAD_DELAY: Duration = Duration::from_millis(1500);

#[derive(Clone)]
pub struct ShellRequestHandler {
    shared: SharedRef,
    /// When set, never route navigations to the external browser — used by the
    /// embedded DevTools view, whose frontend is trusted Chromium chrome.
    permissive: bool,
}

impl ShellRequestHandler {
    pub fn new(shared: SharedRef) -> Self {
        Self {
            shared,
            permissive: false,
        }
    }

    /// Variant that keeps every navigation in-view (DevTools frontend).
    pub fn new_permissive(shared: SharedRef) -> Self {
        Self {
            shared,
            permissive: true,
        }
    }
}

wrap_request_handler! {
    pub struct ShellRequestHandlerBuilder {
        handler: ShellRequestHandler,
    }

    impl RequestHandler {
        fn on_before_browse(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _user_gesture: ::std::os::raw::c_int,
            _is_redirect: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            // DevTools view: load everything in-view, never route out.
            if self.handler.permissive {
                return 0;
            }
            let Some(request) = request else { return 0 };
            let url_uf = request.url();
            let url = CefString::from(&url_uf).to_string();
            if url.is_empty() || is_in_shell(&url) {
                return 0;
            }
            if let Err(err) =
                gio::AppInfo::launch_default_for_uri(&url, None::<&gio::AppLaunchContext>)
            {
                log::warn!("launch_default_for_uri({url}) failed: {err}");
            }
            1
        }

        // Scope renderer file:// access (enabled by --allow-file-access-from-files
        // for the M17 paste tempfile path) to the paste directory only. We return
        // a resource handler exclusively for file:// loads so non-file requests
        // keep CEF's default handling; that handler cancels any file:// URL not
        // inside `$XDG_RUNTIME_DIR/karere/`.
        fn resource_request_handler(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _is_navigation: ::std::os::raw::c_int,
            _is_download: ::std::os::raw::c_int,
            _request_initiator: Option<&CefString>,
            _disable_default_handling: Option<&mut ::std::os::raw::c_int>,
        ) -> Option<ResourceRequestHandler> {
            // DevTools (permissive) view: trusted Chromium chrome, do not scope.
            if self.handler.permissive {
                return None;
            }
            let url = request
                .map(|r| CefString::from(&r.url()).to_string())
                .unwrap_or_default();
            if url.starts_with("file://") {
                Some(PasteFileGuard::new())
            } else {
                None
            }
        }

        fn on_open_urlfrom_tab(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            target_url: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            let url = target_url.map(CefString::to_string).unwrap_or_default();
            route_target(browser, &url);
            1
        }

        fn on_render_process_terminated(
            &self,
            browser: Option<&mut Browser>,
            _status: TerminationStatus,
            _error_code: ::std::os::raw::c_int,
            _error_string: Option<&CefString>,
        ) {
            log::warn!("renderer process terminated");
            let now = Instant::now();
            let storm = {
                let mut s = self.handler.shared.lock();
                s.crash_history
                    .retain(|t| now.duration_since(*t) < CRASH_WINDOW);
                s.crash_history.push(now);
                s.crash_toast = Some("Web view crashed — reconnecting…".to_string());
                if s.crash_history.len() >= CRASH_THRESHOLD {
                    s.crash_dialog_request = Some(CrashDialog {
                        title: "Web view keeps crashing.".to_string(),
                        body: "The embedded WhatsApp view crashed repeatedly. \
                               Open the logs to investigate."
                            .to_string(),
                    });
                    true
                } else {
                    false
                }
            };
            if storm {
                return;
            }
            let Some(browser) = browser else { return };
            let browser = browser.clone();
            glib::timeout_add_local_once(RELOAD_DELAY, move || {
                browser.reload();
            });
        }
    }
}

impl ShellRequestHandlerBuilder {
    pub fn build(handler: ShellRequestHandler) -> RequestHandler {
        Self::new(handler)
    }
}

// Per-`file://`-request guard: cancels any load whose path is not inside the M17
// paste tempfile directory. Returned by `resource_request_handler` only for
// `file://` URLs, so it never sees other schemes.
wrap_resource_request_handler! {
    pub struct PasteFileGuard;

    impl ResourceRequestHandler {
        fn on_before_resource_load(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _callback: Option<&mut Callback>,
        ) -> ReturnValue {
            let url = request
                .map(|r| CefString::from(&r.url()).to_string())
                .unwrap_or_default();
            if crate::paste::is_allowed_file_url(&url) {
                ReturnValue::CONTINUE
            } else {
                log::warn!("blocked file:// load outside paste dir: {url}");
                ReturnValue::CANCEL
            }
        }
    }
}

/// Route a popup / new-tab target: WhatsApp + inert URLs navigate the opener's
/// main frame (keeping everything in the single window), everything else opens
/// in the host's default browser. Used by both the popup and open-from-tab
/// paths, which never reach `on_before_browse`.
pub(crate) fn route_target(browser: Option<&mut Browser>, url: &str) {
    if url.is_empty() {
        return;
    }
    if is_in_shell(url) {
        if let Some(browser) = browser
            && let Some(frame) = browser.main_frame()
        {
            frame.load_url(Some(&CefString::from(url)));
        }
    } else if let Err(err) =
        gio::AppInfo::launch_default_for_uri(url, None::<&gio::AppLaunchContext>)
    {
        log::warn!("launch_default_for_uri({url}) failed: {err}");
    }
}

/// Whether `url` should navigate inside the embedded view rather than the host
/// browser: inert schemes plus the WhatsApp host tree.
fn is_in_shell(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once(':') else {
        // Relative / fragment-only target — let CEF resolve it in-shell.
        return true;
    };
    let scheme = scheme.to_ascii_lowercase();
    if matches!(
        scheme.as_str(),
        "data" | "blob" | "about" | "file" | "chrome-error"
    ) {
        return true;
    }
    match host_of(rest) {
        Some(host) => {
            let host = host.to_ascii_lowercase();
            is_whatsapp_host(&host)
        }
        None => false,
    }
}

fn is_whatsapp_host(host: &str) -> bool {
    for domain in ["whatsapp.com", "whatsapp.net"] {
        if host == domain || host.ends_with(&format!(".{domain}")) {
            return true;
        }
    }
    false
}

/// Extract the host from the part of a URL following `scheme:`.
fn host_of(after_scheme: &str) -> Option<String> {
    let authority = after_scheme.strip_prefix("//")?;
    let end = authority.find(['/', '?', '#']).unwrap_or(authority.len());
    let authority = &authority[..end];
    // Drop any userinfo (`user:pass@`) and trailing port.
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let host = authority.split(':').next().unwrap_or(authority);
    (!host.is_empty()).then(|| host.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whatsapp_hosts_stay_in_shell() {
        assert!(is_in_shell("https://web.whatsapp.com/"));
        assert!(is_in_shell("https://whatsapp.com/foo"));
        assert!(is_in_shell("https://faq.whatsapp.com/article"));
        assert!(is_in_shell("https://static.whatsapp.net/img.png"));
    }

    #[test]
    fn inert_schemes_stay_in_shell() {
        assert!(is_in_shell("about:blank"));
        assert!(is_in_shell("blob:abc-123"));
        assert!(is_in_shell("data:text/plain,hi"));
        assert!(is_in_shell("chrome-error://chromewebdata/"));
    }

    #[test]
    fn external_hosts_route_out() {
        assert!(!is_in_shell("https://google.com/"));
        assert!(!is_in_shell("https://evil-whatsapp.com.example/"));
        assert!(!is_in_shell("https://notwhatsapp.com/"));
    }

    #[test]
    fn host_parsing_strips_port_and_userinfo() {
        assert_eq!(host_of("//user:pw@web.whatsapp.com:443/x"), Some("web.whatsapp.com".to_string()));
        assert_eq!(host_of("//google.com"), Some("google.com".to_string()));
    }
}
