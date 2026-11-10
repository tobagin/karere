use std::sync::OnceLock;
use std::time::{Duration, Instant};

use cef::{
    self, Browser, CefString, Frame, ImplBrowser, ImplFrame, ImplRequest, ImplRequestHandler,
    Request, RequestHandler, TerminationStatus, WindowOpenDisposition, WrapRequestHandler,
    rc::Rc, wrap_request_handler,
};

use super::{CrashDialog, SharedRef};

const CRASH_WINDOW: Duration = Duration::from_secs(60);
const CRASH_THRESHOLD: usize = 5;
const RELOAD_DELAY: Duration = Duration::from_millis(1500);

#[derive(Clone)]
pub struct ShellRequestHandler {
    shared: SharedRef,
    /// When set, never route navigations out — used by the embedded DevTools
    /// view (trusted Chromium chrome).
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
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            user_gesture: ::std::os::raw::c_int,
            _is_redirect: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            // DevTools view: load in-view, never route out.
            if self.handler.permissive {
                return 0;
            }
            let Some(request) = request else { return 0 };
            let url_uf = request.url();
            let url = CefString::from(&url_uf).to_string();
            if url.is_empty() {
                return 0;
            }
            let is_main = frame.is_none_or(|f| f.is_main() == 1);
            if !is_main {
                // Subframes never leave the shell; disallowed ones are cancelled.
                return if is_subframe_allowed(&url) { 0 } else { 1 };
            }
            if is_in_shell(&url) || is_dev_url_override(&url) {
                return 0;
            }
            open_external(&url, user_gesture != 0);
            1
        }

        fn on_open_urlfrom_tab(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            target_url: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            user_gesture: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            let url = target_url.map(CefString::to_string).unwrap_or_default();
            let from_main = frame.is_none_or(|f| f.is_main() == 1);
            route_target(browser, &url, from_main && user_gesture != 0);
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

/// Route a popup/new-tab target: WhatsApp Web navigates the opener's main
/// frame (single window), allowed external links open in the default browser
/// when `allow_external` (main frame + user gesture), everything else is
/// dropped. Used by the popup and open-from-tab paths (never reach
/// `on_before_browse`). Only `https://web.whatsapp.com` is ever loaded here,
/// so a `data:`/`file:` target can never become a browser-initiated load.
pub(crate) fn route_target(browser: Option<&mut Browser>, url: &str, allow_external: bool) {
    if url.is_empty() {
        return;
    }
    if is_whatsapp_web(url) {
        if let Some(browser) = browser
            && let Some(frame) = browser.main_frame()
        {
            frame.load_url(Some(&CefString::from(url)));
        }
    } else {
        open_external(url, allow_external);
    }
}

/// Hand `url` to the desktop's default handler when the scheme is allowlisted
/// and the navigation came from a user gesture in the main frame.
fn open_external(url: &str, allow_external: bool) {
    if !allow_external || !is_external_allowed(url) {
        log::debug!(
            "blocked navigation (gesture/main frame: {allow_external}, scheme: {:?})",
            scheme_of(url)
        );
        return;
    }
    if let Err(err) = gio::AppInfo::launch_default_for_uri(url, None::<&gio::AppLaunchContext>) {
        log::warn!("launch_default_for_uri failed: {err}");
    }
}

/// Lowercased scheme of an absolute URL.
fn scheme_of(url: &str) -> Option<String> {
    let (scheme, _) = url.split_once(':')?;
    let valid = !scheme.is_empty()
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid.then(|| scheme.to_ascii_lowercase())
}

/// Schemes the system may open on the page's behalf.
fn is_external_allowed(url: &str) -> bool {
    matches!(
        scheme_of(url).as_deref(),
        Some("http" | "https" | "mailto" | "tel")
    )
}

/// Whether `url` is exactly the WhatsApp Web origin: `https`, host
/// `web.whatsapp.com`, default port, no userinfo. This is the only origin that
/// may stay in the top frame and the only one that gets the native bridge.
pub(crate) fn is_whatsapp_web(url: &str) -> bool {
    if scheme_of(url).as_deref() != Some("https") {
        return false;
    }
    let Some((_, rest)) = url.split_once(':') else {
        return false;
    };
    let Some(authority) = rest.strip_prefix("//") else {
        return false;
    };
    let end = authority.find(['/', '?', '#']).unwrap_or(authority.len());
    let authority = authority[..end].to_ascii_lowercase();
    authority == WHATSAPP_WEB_HOST || authority == format!("{WHATSAPP_WEB_HOST}:443")
}

const WHATSAPP_WEB_HOST: &str = "web.whatsapp.com";

/// Top-frame policy: WhatsApp Web, plus the internal pages CEF itself puts in
/// the top frame (`about:blank`, error pages) and same-origin blobs.
fn is_in_shell(url: &str) -> bool {
    if is_whatsapp_web(url) || url.eq_ignore_ascii_case("about:blank") {
        return true;
    }
    match scheme_of(url).as_deref() {
        Some("chrome-error") => true,
        Some("blob") => is_whatsapp_web(&url["blob:".len()..]),
        _ => false,
    }
}

/// Subframe policy: inert schemes and https WhatsApp hosts load; everything
/// else is cancelled (never opened externally).
fn is_subframe_allowed(url: &str) -> bool {
    match scheme_of(url).as_deref() {
        Some("about" | "blob" | "data" | "chrome-error") => true,
        Some("https") => url
            .split_once(':')
            .and_then(|(_, rest)| host_of(rest))
            .is_some_and(|host| is_whatsapp_host(&host.to_ascii_lowercase())),
        _ => false,
    }
}

/// The `--url` development override is loaded as given. It never gets the
/// native bridge (that is gated on [`is_whatsapp_web`] separately).
fn is_dev_url_override(url: &str) -> bool {
    static OVERRIDE: OnceLock<Option<String>> = OnceLock::new();
    OVERRIDE
        .get_or_init(crate::parse_url_flag)
        .as_deref()
        .is_some_and(|dev| dev.trim_end_matches('/') == url.trim_end_matches('/'))
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
    fn only_https_web_whatsapp_stays_in_shell() {
        assert!(is_in_shell("https://web.whatsapp.com/"));
        assert!(is_in_shell("https://web.whatsapp.com"));
        assert!(is_in_shell("HTTPS://Web.WhatsApp.com/send?phone=1"));
        assert!(is_in_shell("https://web.whatsapp.com:443/"));
        assert!(!is_in_shell("http://web.whatsapp.com/"));
        assert!(!is_in_shell("https://web.whatsapp.com:8443/"));
        assert!(!is_in_shell("https://user@web.whatsapp.com/"));
        assert!(!is_in_shell("https://web.whatsapp.com.evil.example/"));
        assert!(!is_in_shell("wss://web.whatsapp.com/"));
    }

    #[test]
    fn other_whatsapp_hosts_leave_the_shell() {
        assert!(!is_in_shell("https://faq.whatsapp.com/article"));
        assert!(!is_in_shell("https://whatsapp.com/foo"));
        assert!(!is_in_shell("https://chat.whatsapp.com/invite"));
        assert!(!is_in_shell("https://static.whatsapp.net/img.png"));
        assert!(is_external_allowed("https://faq.whatsapp.com/article"));
    }

    #[test]
    fn top_frame_internal_pages() {
        assert!(is_in_shell("about:blank"));
        assert!(is_in_shell("chrome-error://chromewebdata/"));
        assert!(is_in_shell("blob:https://web.whatsapp.com/1234-abcd"));
        assert!(!is_in_shell("blob:https://evil.example/1234"));
        assert!(!is_in_shell("about:srcdoc"));
    }

    #[test]
    fn dangerous_schemes_rejected_in_top_frame() {
        assert!(!is_in_shell("data:text/html,<script>1</script>"));
        assert!(!is_in_shell("file:///etc/passwd"));
        assert!(!is_in_shell("javascript:alert(1)"));
        assert!(!is_in_shell("relative/path"));
        assert!(!is_in_shell(""));
    }

    #[test]
    fn subframe_policy() {
        assert!(is_subframe_allowed("about:blank"));
        assert!(is_subframe_allowed("about:srcdoc"));
        assert!(is_subframe_allowed("blob:https://web.whatsapp.com/x"));
        assert!(is_subframe_allowed("data:text/plain,hi"));
        assert!(is_subframe_allowed("https://static.whatsapp.net/x"));
        assert!(!is_subframe_allowed("http://static.whatsapp.net/x"));
        assert!(!is_subframe_allowed("https://google.com/"));
        assert!(!is_subframe_allowed("file:///etc/passwd"));
        assert!(!is_subframe_allowed("javascript:alert(1)"));
    }

    #[test]
    fn external_scheme_allowlist() {
        assert!(is_external_allowed("https://google.com/"));
        assert!(is_external_allowed("http://example.com/"));
        assert!(is_external_allowed("mailto:a@example.com"));
        assert!(is_external_allowed("TEL:+10000000000"));
        for url in [
            "javascript:alert(1)",
            "data:text/html,x",
            "file:///etc/passwd",
            "smb://host/share",
            "ssh://host",
            "vscode://file/x",
            "steam://run/1",
            "blob:https://web.whatsapp.com/x",
            "",
        ] {
            assert!(!is_external_allowed(url), "{url}");
        }
    }

    #[test]
    fn external_hosts_route_out() {
        assert!(!is_in_shell("https://google.com/"));
        assert!(!is_in_shell("https://evil-whatsapp.com.example/"));
        assert!(!is_in_shell("https://notwhatsapp.com/"));
    }

    #[test]
    fn host_parsing_strips_port_and_userinfo() {
        assert_eq!(
            host_of("//user:pw@web.whatsapp.com:443/x"),
            Some("web.whatsapp.com".to_string())
        );
        assert_eq!(host_of("//google.com"), Some("google.com".to_string()));
    }
}
