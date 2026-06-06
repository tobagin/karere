//! In-process notification bridge. CEF exposes no notification API, so we drive
//! the Chrome DevTools Protocol over the browser host's *in-process* message
//! channel (`AddDevToolsMessageObserver` / `SendDevToolsMessage`) — NOT a network
//! `--remote-debugging-port`. One observer per account browser patches
//! `Notification`/`showNotification` in the page (and any service-worker)
//! realm and forwards payloads to branded `gio::Notification`s.
//!
//! This needs no TCP port, no `remote-allow-origins`, and no Private/Local
//! Network Access exception, so nothing outside the process (or the Flatpak
//! sandbox) can reach the session through it. The F12 inspector still uses a
//! debug-only port (see `crate::devtools` / `cef_runtime::debug_enabled`).

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use cef::{
    Browser, BrowserHost, DevToolsMessageObserver, ImplBrowser, ImplBrowserHost,
    ImplDevToolsMessageObserver, Registration, WrapDevToolsMessageObserver,
    wrap_dev_tools_message_observer,
};
// Brings the cef refcount trait's `add_ref`/`release` into scope for the macro,
// without shadowing `std::rc::Rc` used for the observer's shared state.
use cef::rc::Rc as _;

// Transport is console.log (not Runtime.addBinding): bindingCalled only reaches the registering session, but consoleAPICalled reaches any session with Runtime.enable.
const NOTIF_PREFIX: &str = "__KARERE_NOTIF__:";

const SW_PATCH: &str = r#"
(function () {
  var PREFIX = "__KARERE_NOTIF__:";

  function override(title, opts) {
    opts = opts || {};
    var payload = {
      title: title == null ? "" : String(title),
      body: opts.body == null ? "" : String(opts.body),
      tag: opts.tag == null ? "" : String(opts.tag),
      icon: "",
    };
    function emit() {
      try { console.log(PREFIX + JSON.stringify(payload)); } catch (e) {}
    }
    function iconAllowed(u) {
      if (/^data:/i.test(u)) return true;
      try { return /(^|\.)whatsapp\.(net|com)$/i.test(new URL(u, self.location.href).hostname); }
      catch (e) { return false; }
    }
    var iconUrl = opts.icon;
    if (iconUrl && iconAllowed(iconUrl)) {
      fetch(iconUrl)
        .then(function (r) { return r.ok ? r.blob() : null; })
        .then(function (b) {
          if (!b) { emit(); return; }
          return new Promise(function (res) {
            var fr = new FileReader();
            fr.onloadend = function () {
              payload.icon = typeof fr.result === "string" ? fr.result : "";
              res();
            };
            fr.onerror = function () { res(); };
            fr.readAsDataURL(b);
          }).then(emit);
        })
        .catch(emit);
    } else {
      emit();
    }
    return Promise.resolve();
  }
  override.__karere = true;

  var did = [];

  // Patch the prototype: showNotification resolves via it, so an own-property patch alone is bypassed.
  try {
    var P = (self.ServiceWorkerRegistration && self.ServiceWorkerRegistration.prototype) || null;
    if (P && typeof P.showNotification === "function" && !P.showNotification.__karere) {
      P.showNotification = override;
      did.push("proto");
    }
  } catch (e) {}

  try {
    var reg = self.registration;
    if (reg && typeof reg.showNotification === "function" && !reg.showNotification.__karere) {
      reg.showNotification = override;
      did.push("instance");
    }
  } catch (e) {}

  return "patched:[" + did.join(",") + "]";
})();
"#;

/// Page-realm patch: WhatsApp raises message notifications via `new Notification()` in the page (not the SW). Replaces `window.Notification` with a Proxy whose `construct` trap suppresses the banner, forwards via `console.log`, and returns a Notification-shaped stub.
const PAGE_PATCH: &str = r#"
(function () {
  var PREFIX = "__KARERE_NOTIF__:";
  try {
    var Orig = window.Notification;
    if (typeof Orig !== "function") return "no-notification-api";
    if (window.__karereNotifPatched) return "already";
    window.__karereNotifPatched = true;

    var liveByTag = (window.__karereLive = window.__karereLive || new Map());
    var seq = 0;

    function Stub(title, opts) {
      opts = opts || {};
      this.title = title == null ? "" : String(title);
      this.body = opts.body == null ? "" : String(opts.body);
      this.icon = opts.icon == null ? "" : String(opts.icon);
      this.tag = opts.tag == null ? ("__k" + (++seq)) : String(opts.tag);
      this.data = opts.data;
      this.onclick = null; this.onclose = null; this.onshow = null; this.onerror = null;
      this._l = { click: [], close: [], show: [], error: [] };
    }
    Stub.prototype.addEventListener = function (t, c) { if (this._l[t] && typeof c === "function") this._l[t].push(c); };
    Stub.prototype.removeEventListener = function (t, c) { var a = this._l[t]; if (!a) return; var i = a.indexOf(c); if (i >= 0) a.splice(i, 1); };
    Stub.prototype.dispatchEvent = function (e) {
      var t = e && e.type, h = this["on" + t];
      try { if (typeof h === "function") h.call(this, e); } catch (x) {}
      var a = this._l[t] || []; for (var i = 0; i < a.length; i++) { try { a[i].call(this, e); } catch (x) {} }
      return true;
    };
    Stub.prototype._fire = function (t) { var e; try { e = new Event(t); } catch (x) { e = { type: t, target: this }; } this.dispatchEvent(e); };
    Stub.prototype.close = function () { liveByTag.delete(this.tag); this._fire("close"); };

    function iconAllowed(url) {
      if (/^blob:/i.test(url)) return true;
      try { return /(^|\.)whatsapp\.(net|com)$/i.test(new URL(url, location.href).hostname); }
      catch (e) { return false; }
    }
    function resolveIcon(url) {
      if (!url) return Promise.resolve("");
      if (/^data:/i.test(url)) return Promise.resolve(url);
      if (!iconAllowed(url)) return Promise.resolve("");
      return fetch(url).then(function (r) { return r.ok ? r.blob() : null; }).then(function (b) {
        if (!b) return "";
        return new Promise(function (res) {
          var fr = new FileReader();
          fr.onloadend = function () { res(typeof fr.result === "string" ? fr.result : ""); };
          fr.onerror = function () { res(""); };
          fr.readAsDataURL(b);
        });
      }).catch(function () { return ""; });
    }

    var bodyTags = (window.__karereBodyTags = window.__karereBodyTags || {});
    function construct(title, opts) {
      var stub = new Stub(title, opts);
      liveByTag.set(stub.tag, stub);
      // WhatsApp fires twice per tag (real body, then empty); skip an empty body only if a real one already fired.
      if (!stub.body && bodyTags[stub.tag]) return stub;
      if (stub.body) bodyTags[stub.tag] = true;
      resolveIcon(stub.icon).then(function (icon) {
        try {
          console.log(PREFIX + JSON.stringify({
            title: stub.title, body: stub.body, tag: stub.tag, icon: icon,
          }));
        } catch (e) {}
      });
      return stub;
    }

    var P = new Proxy(Orig, {
      construct: function (_t, a) { return construct(a[0], a[1]); },
      get: function (t, p, r) { return Reflect.get(t, p, r); },
      set: function (t, p, v, r) { return Reflect.set(t, p, v, r); },
      has: function (t, p) { return Reflect.has(t, p); },
    });
    try {
      Object.defineProperty(window, "Notification", { configurable: true, writable: true, value: P });
    } catch (e) { window.Notification = P; }

    window.__karereCloseNotif = function (tag) { var s = liveByTag.get(String(tag)); if (s) s.close(); };
    window.__karereActivateNotif = function (tag) { var s = liveByTag.get(String(tag)); if (s) s._fire("click"); };

    return "patched-page";
  } catch (e) {
    return "error:" + (e && e.message ? e.message : e);
  }
})();
"#;

/// Per-browser CDP state. Lives on the CEF UI thread (single-threaded), so a
/// plain `RefCell` suffices — no locking.
struct State {
    /// Trusted account id for this browser (notifications are attributed to it).
    account_id: String,
    next_id: u32,
    /// Flattened child sessions known to be service workers (patched with SW_PATCH).
    sw_sessions: HashSet<String>,
}

impl State {
    fn new(account_id: String) -> Self {
        Self {
            account_id,
            next_id: 100,
            sw_sessions: HashSet::new(),
        }
    }

    fn bump(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

wrap_dev_tools_message_observer! {
    struct NotifObserver {
        state: Rc<RefCell<State>>,
    }

    impl DevToolsMessageObserver {
        // The agent attaches lazily on the first message. Arm here too so a
        // re-attach after navigation/crash re-applies the patches.
        fn on_dev_tools_agent_attached(&self, browser: Option<&mut Browser>) {
            if let Some(b) = browser
                && let Some(host) = b.host()
            {
                arm(&host, &self.state);
            }
        }

        // Raw CDP frame (retains the flattened-session `sessionId`); mirrors the
        // former WebSocket recv loop.
        fn on_dev_tools_message(
            &self,
            browser: Option<&mut Browser>,
            message: Option<&[u8]>,
        ) -> ::std::os::raw::c_int {
            let (Some(b), Some(bytes)) = (browser, message) else {
                return 0;
            };
            let Some(host) = b.host() else { return 0 };
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes) {
                handle(&self.state, &host, &v);
            }
            0
        }
    }
}

/// Attach the in-process notification bridge to `browser` for `account_id`.
/// The returned [`Registration`] must be held alive for as long as the bridge
/// should run; dropping it detaches the observer.
pub fn attach(browser: &Browser, account_id: &str) -> Option<Registration> {
    let host = browser.host()?;
    let state = Rc::new(RefCell::new(State::new(account_id.to_owned())));
    let mut observer = NotifObserver::new(state.clone());
    let reg = host.add_dev_tools_message_observer(Some(&mut observer));
    // Sending the first message forces the agent to attach; arm immediately so
    // an already-loaded page is patched without waiting for a navigation.
    arm(&host, &state);
    log::info!("cdp: in-process bridge attached for account {account_id}");
    reg
}

/// Enable Runtime + patch the page realm, and arm auto-attach so service-worker
/// child sessions get patched as they appear. Idempotent (patches self-guard).
///
/// `send_dev_tools_message` delivers responses/events SYNCHRONOUSLY and can
/// re-enter the observer (e.g. agent-attached, executionContextCreated), so the
/// state borrow must be released before each send — never held across one.
fn arm(host: &BrowserHost, state: &Rc<RefCell<State>>) {
    let id = state.borrow_mut().bump();
    send(host, &json_msg(id, "Runtime.enable", "{}"));
    let id = state.borrow_mut().bump();
    send(host, &json_msg(id, "Runtime.evaluate", &eval_params(PAGE_PATCH)));
    let id = state.borrow_mut().bump();
    send(
        host,
        &json_msg(
            id,
            "Target.setAutoAttach",
            "{\"autoAttach\":true,\"waitForDebuggerOnStart\":false,\"flatten\":true}",
        ),
    );
}

/// Process one CDP frame. `session` is `None` for the page (root) realm and
/// `Some` for a flattened child session (e.g. a service worker).
// NOTE: `send` re-enters this observer synchronously, so the `state` borrow is
// taken in short scopes and always released before any `send` (see `arm`).
fn handle(state: &Rc<RefCell<State>>, host: &BrowserHost, v: &serde_json::Value) {
    let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let session = v.get("sessionId").and_then(|s| s.as_str());

    match method {
        "Target.attachedToTarget" => {
            let params = v.get("params");
            let ttype = params
                .and_then(|p| p.get("targetInfo"))
                .and_then(|t| t.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("");
            let child = params
                .and_then(|p| p.get("sessionId"))
                .and_then(|s| s.as_str())
                .unwrap_or("");
            if ttype == "service_worker" && !child.is_empty() {
                let child = child.to_owned();
                // Record before Runtime.enable: it emits executionContextCreated
                // synchronously, and that handler only re-patches known sessions.
                state.borrow_mut().sw_sessions.insert(child.clone());
                log::info!("cdp: service_worker attached; arming SW patch");
                let id = state.borrow_mut().bump();
                send(host, &json_msg_sess(id, "Runtime.enable", "{}", &child));
                let id = state.borrow_mut().bump();
                send(
                    host,
                    &json_msg_sess(id, "Runtime.evaluate", &eval_params(SW_PATCH), &child),
                );
            }
        }
        // A fresh execution context (page reload, or SW restart) means the realm
        // lost our patch; re-apply it.
        "Runtime.executionContextCreated" => match session {
            None => {
                let id = state.borrow_mut().bump();
                send(host, &json_msg(id, "Runtime.evaluate", &eval_params(PAGE_PATCH)));
            }
            Some(s) if state.borrow().sw_sessions.contains(s) => {
                let s = s.to_owned();
                let id = state.borrow_mut().bump();
                send(
                    host,
                    &json_msg_sess(id, "Runtime.evaluate", &eval_params(SW_PATCH), &s),
                );
            }
            _ => {}
        },
        "Runtime.consoleAPICalled" => {
            if let Some(payload) = console_payload(v) {
                let account = state.borrow().account_id.clone();
                dispatch(payload, account);
            }
        }
        _ => {
            if let Some(err) = v.get("error") {
                log::debug!("cdp: cmd error -> {err}");
            }
        }
    }
}

fn send(host: &BrowserHost, text: &str) {
    host.send_dev_tools_message(Some(text.as_bytes()));
}

/// Hand a notification payload to the GTK main thread, attributed to `account_id`.
fn dispatch(payload_json: String, account_id: String) {
    glib::MainContext::default().invoke(move || {
        let p: NotifPayload = match serde_json::from_str(&payload_json) {
            Ok(p) => p,
            Err(e) => {
                log::warn!("cdp: bad notif payload: {e}");
                return;
            }
        };
        let icon = if p.icon.is_empty() {
            None
        } else {
            Some(p.icon.as_str())
        };
        log::info!(
            "cdp: notification tag={:?} title={:?} account={:?}",
            p.tag,
            p.title,
            account_id
        );
        crate::notifications::tracker().on_seen(&p.tag, &p.title, &p.body, icon, &account_id);
    });
}

#[derive(serde::Deserialize)]
struct NotifPayload {
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    tag: String,
    #[serde(default)]
    icon: String,
}

/// Extract our notification payload (a `NOTIF_PREFIX`-tagged string arg) from a `consoleAPICalled` event.
fn console_payload(v: &serde_json::Value) -> Option<String> {
    let args = v.get("params")?.get("args")?.as_array()?;
    for a in args {
        if a.get("type").and_then(|t| t.as_str()) == Some("string")
            && let Some(s) = a.get("value").and_then(|s| s.as_str())
            && let Some(rest) = s.strip_prefix(NOTIF_PREFIX)
        {
            return Some(rest.to_owned());
        }
    }
    None
}

/// Build the `Runtime.evaluate` params for a patch expression.
fn eval_params(expr: &str) -> String {
    format!(
        "{{\"expression\":{},\"returnByValue\":true}}",
        json_string(expr)
    )
}

/// Build a CDP request frame; `params` is a raw JSON object string.
fn json_msg(id: u32, method: &str, params: &str) -> String {
    format!("{{\"id\":{id},\"method\":\"{method}\",\"params\":{params}}}")
}

/// Like [`json_msg`] but addressed to a flattened child session.
fn json_msg_sess(id: u32, method: &str, params: &str, session: &str) -> String {
    format!(
        "{{\"id\":{id},\"method\":\"{method}\",\"params\":{params},\"sessionId\":{}}}",
        json_string(session)
    )
}

fn json_string(s: &str) -> String {
    serde_json::Value::String(s.to_owned()).to_string()
}
