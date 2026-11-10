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
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

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

/// Desired web-content color scheme, mirrored from the Karere theme (#160).
/// WhatsApp Web (on its default "System" theme) follows `prefers-color-scheme`;
/// CEF never reads the app's libadwaita scheme, so we emulate the media feature
/// over CDP. Set by [`set_dark_preference`] (theme apply); read by [`arm`] and
/// [`apply_color_scheme`].
static PREFERS_DARK: AtomicBool = AtomicBool::new(false);

/// Update the desired web-content color scheme. Live browsers are re-emulated by
/// the caller (the GTK layer iterates its web views); newly-loaded pages pick it
/// up via [`arm`]. (#160)
pub fn set_dark_preference(dark: bool) {
    PREFERS_DARK.store(dark, Ordering::Relaxed);
}

/// Emulate `prefers-color-scheme` on `host`'s page realm to match the current
/// [`PREFERS_DARK`]. Sent on attach/load and on live theme changes. (#160)
pub fn apply_color_scheme(host: &BrowserHost) {
    // Out-of-band id range so it can't collide with a session's bumped ids.
    static CTR: AtomicU32 = AtomicU32::new(2_000_000);
    let id = CTR.fetch_add(1, Ordering::Relaxed);
    let scheme = if PREFERS_DARK.load(Ordering::Relaxed) {
        "dark"
    } else {
        "light"
    };
    send(
        host,
        &json_msg(
            id,
            "Emulation.setEmulatedMedia",
            &format!(
                "{{\"features\":[{{\"name\":\"prefers-color-scheme\",\"value\":\"{scheme}\"}}]}}"
            ),
        ),
    );
}

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
///
/// Installed both as a new-document script (runs before page scripts) and by
/// `Runtime.evaluate` (already-loaded documents), so it is idempotent: it only
/// (re)installs when `window.Notification` is not already its own proxy, which
/// also lets it win over the renderer-injected fallback observer whichever of
/// the two runs first. Page-initiated `close()` reports `{event:"close"}`.
const PAGE_PATCH: &str = r#"
(function () {
  var PREFIX = "__KARERE_NOTIF__:";
  try {
    var Orig = window.Notification;
    if (typeof Orig !== "function") return "no-notification-api";
    if (window.__karereNotifProxy && Orig === window.__karereNotifProxy) return "already";

    var liveByTag = (window.__karereLive = window.__karereLive || new Map());
    // tag -> time a real-body notification fired (see construct).
    var bodyTags = (window.__karereBodyTags = window.__karereBodyTags || new Map());
    var BODY_TTL_MS = 5000;
    function emit(o) { try { console.log(PREFIX + JSON.stringify(o)); } catch (e) {} }
    function nextSeq() { window.__karereSeq = (window.__karereSeq || 0) + 1; return window.__karereSeq; }

    function Stub(title, opts) {
      opts = opts || {};
      this.title = title == null ? "" : String(title);
      this.body = opts.body == null ? "" : String(opts.body);
      this.icon = opts.icon == null ? "" : String(opts.icon);
      this.tag = opts.tag == null ? ("__k" + nextSeq()) : String(opts.tag);
      this.data = opts.data;
      this.onclick = null; this.onclose = null; this.onshow = null; this.onerror = null;
      this._l = { click: [], close: [], show: [], error: [] };
      this._closed = false;
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
    // `report`: page-initiated close -> tell the host to withdraw the banner.
    // Only the current stub for a tag counts, and the report is deferred so a
    // close immediately followed by a same-tag replacement keeps the banner.
    Stub.prototype._close = function (report) {
      if (this._closed) return;
      this._closed = true;
      if (liveByTag.get(this.tag) === this) {
        liveByTag.delete(this.tag);
        if (report) {
          var tag = this.tag;
          setTimeout(function () { if (!liveByTag.has(tag)) emit({ event: "close", tag: tag }); }, 1000);
        }
      }
      this._fire("close");
    };
    Stub.prototype.close = function () { this._close(true); };

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

    function construct(title, opts) {
      var stub = new Stub(title, opts);
      liveByTag.set(stub.tag, stub);
      // WhatsApp fires twice per tag (real body, then empty); skip an empty body
      // only shortly after a real one for the same tag.
      var now = Date.now();
      bodyTags.forEach(function (t, k) { if (now - t > BODY_TTL_MS) bodyTags.delete(k); });
      if (!stub.body && bodyTags.has(stub.tag)) return stub;
      if (stub.body) bodyTags.set(stub.tag, now);
      resolveIcon(stub.icon).then(function (icon) {
        emit({ title: stub.title, body: stub.body, tag: stub.tag, icon: icon });
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
    window.__karereNotifProxy = P;
    window.__karereNotifPatched = true;

    window.__karereCloseNotif = function (tag) { var s = liveByTag.get(String(tag)); if (s) s._close(false); };
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
    /// WhatsApp Web locale (`WALangUserPref`) to enforce, from the UI-language
    /// override; `None` follows the linked phone. WhatsApp reads this only at
    /// boot, so a mismatch needs a cache-bypassing reload (the SW pins the
    /// language otherwise) — done at most once per browser via `lang_reloaded`.
    desired_lang: Option<String>,
    lang_query_id: Option<u32>,
    lang_reloaded: bool,
    /// PAGE_PATCH registered via `Page.addScriptToEvaluateOnNewDocument` for
    /// the current DevTools session (reset on agent detach).
    new_doc_script_added: bool,
    /// Pending `Page.getFrameTree` request id (main-frame discovery).
    frame_tree_query_id: Option<u32>,
    /// Main frame id of the page; notification markers are accepted only from
    /// its default execution context.
    main_frame_id: Option<String>,
    /// Page-session default (main-world) execution contexts: id -> frame id.
    default_contexts: HashMap<i64, String>,
}

impl State {
    fn new(account_id: String) -> Self {
        Self {
            account_id,
            next_id: 100,
            sw_sessions: HashSet::new(),
            // Raw gettext override code (e.g. `pt_BR`); reconcile derives the
            // cookie (`pt_BR`) and `WALangUserPref` (`pt-BR`) forms from it.
            desired_lang: crate::i18n::override_locale(),
            lang_query_id: None,
            lang_reloaded: false,
            new_doc_script_added: false,
            frame_tree_query_id: None,
            main_frame_id: None,
            default_contexts: HashMap::new(),
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

        // A new session starts with no new-document scripts and no contexts.
        fn on_dev_tools_agent_detached(&self, _browser: Option<&mut Browser>) {
            let mut st = self.state.borrow_mut();
            st.new_doc_script_added = false;
            st.frame_tree_query_id = None;
            st.main_frame_id = None;
            st.default_contexts.clear();
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

/// Enable Runtime + Page, register the page patch for every new document and
/// apply it to the current one, and arm auto-attach so service-worker child
/// sessions get patched as they appear. Idempotent (patches self-guard; the
/// new-document script is registered once per DevTools session).
///
/// `send_dev_tools_message` delivers responses/events SYNCHRONOUSLY and can
/// re-enter the observer (e.g. agent-attached, executionContextCreated), so the
/// state borrow must be released before each send — never held across one.
fn arm(host: &BrowserHost, state: &Rc<RefCell<State>>) {
    let id = state.borrow_mut().bump();
    send(host, &json_msg(id, "Runtime.enable", "{}"));
    // Page.enable: needed for addScriptToEvaluateOnNewDocument, frameNavigated
    // (main-frame id) and loadEventFired — the reliable point to read the
    // rendered language (document.documentElement.lang is unset this early).
    let id = state.borrow_mut().bump();
    send(host, &json_msg(id, "Page.enable", "{}"));
    // Run the patch in every new document before any page script.
    let register = {
        let mut st = state.borrow_mut();
        !std::mem::replace(&mut st.new_doc_script_added, true)
    };
    if register {
        let id = state.borrow_mut().bump();
        send(
            host,
            &json_msg(
                id,
                "Page.addScriptToEvaluateOnNewDocument",
                &format!("{{\"source\":{}}}", json_string(PAGE_PATCH)),
            ),
        );
    }
    // And in the already-loaded document, if any.
    let id = state.borrow_mut().bump();
    send(
        host,
        &json_msg(id, "Runtime.evaluate", &eval_params(PAGE_PATCH)),
    );
    // Learn the main frame id (notification markers are main-frame only).
    let id = state.borrow_mut().bump();
    state.borrow_mut().frame_tree_query_id = Some(id);
    send(host, &json_msg(id, "Page.getFrameTree", "{}"));
    // Match the web content's color scheme to the Karere theme (#160).
    apply_color_scheme(host);
    // Read the current WhatsApp locale; `handle` reconciles it against the
    // override and reloads once if needed. Skip when no reload is pending.
    // Also probe now in case the page is already loaded (re-attach); the empty
    // `html_lang` guard in reconcile_lang makes a too-early probe a no-op.
    send_lang_probe(host, state);
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

/// Ask the page realm for its rendered language + stored pref (handled in
/// `handle` → `reconcile_lang`). No-op once a reload has already been issued.
fn send_lang_probe(host: &BrowserHost, state: &Rc<RefCell<State>>) {
    if state.borrow().lang_reloaded {
        return;
    }
    let id = state.borrow_mut().bump();
    state.borrow_mut().lang_query_id = Some(id);
    send(
        host,
        &json_msg(
            id,
            "Runtime.evaluate",
            &eval_params(
                "JSON.stringify({h:document.documentElement.lang,\
                 p:localStorage.getItem('WALangUserPref')})",
            ),
        ),
    );
}

/// Process one CDP frame. `session` is `None` for the page (root) realm and
/// `Some` for a flattened child session (e.g. a service worker).
// NOTE: `send` re-enters this observer synchronously, so the `state` borrow is
// taken in short scopes and always released before any `send` (see `arm`).
fn handle(state: &Rc<RefCell<State>>, host: &BrowserHost, v: &serde_json::Value) {
    // Response to the WhatsApp-locale probe (see `arm`): reconcile + maybe reload.
    if let Some(id) = v.get("id").and_then(serde_json::Value::as_u64)
        && state.borrow().lang_query_id == Some(id as u32)
    {
        state.borrow_mut().lang_query_id = None;
        let current = v
            .get("result")
            .and_then(|r| r.get("result"))
            .and_then(|r| r.get("value"))
            .and_then(|x| x.as_str())
            .map(str::to_owned);
        reconcile_lang(state, host, current.as_deref());
        return;
    }

    // Response to the main-frame discovery request (see `arm`).
    if let Some(id) = v.get("id").and_then(serde_json::Value::as_u64)
        && state.borrow().frame_tree_query_id == Some(id as u32)
    {
        let frame = v
            .pointer("/result/frameTree/frame/id")
            .and_then(|f| f.as_str())
            .map(str::to_owned);
        let mut st = state.borrow_mut();
        st.frame_tree_query_id = None;
        if frame.is_some() {
            st.main_frame_id = frame;
        }
        return;
    }

    let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let session = v.get("sessionId").and_then(|s| s.as_str());

    // Page finished loading (page realm only) — now the rendered language is
    // set, so re-probe and reconcile against the override.
    if method == "Page.loadEventFired" && session.is_none() {
        send_lang_probe(host, state);
        return;
    }

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
                let ctx = params_context(v);
                if let Some((ctx_id, frame_id)) = ctx {
                    state.borrow_mut().default_contexts.insert(ctx_id, frame_id);
                }
                let id = state.borrow_mut().bump();
                send(
                    host,
                    &json_msg(id, "Runtime.evaluate", &eval_params(PAGE_PATCH)),
                );
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
        "Runtime.executionContextDestroyed" if session.is_none() => {
            if let Some(ctx_id) = v
                .pointer("/params/executionContextId")
                .and_then(serde_json::Value::as_i64)
            {
                state.borrow_mut().default_contexts.remove(&ctx_id);
            }
        }
        "Runtime.executionContextsCleared" if session.is_none() => {
            state.borrow_mut().default_contexts.clear();
        }
        // A top-level navigation reports the main frame (no parentId).
        "Page.frameNavigated" if session.is_none() => {
            let frame = v.pointer("/params/frame");
            let is_top = frame.is_some_and(|f| f.get("parentId").is_none());
            if is_top
                && let Some(id) = frame.and_then(|f| f.get("id")).and_then(|i| i.as_str())
            {
                state.borrow_mut().main_frame_id = Some(id.to_owned());
            }
        }
        "Runtime.consoleAPICalled" => {
            if let Some(payload) = console_payload(v) {
                if !marker_trusted(&state.borrow(), session, v) {
                    log::debug!("cdp: dropped notification marker from an untrusted context");
                    return;
                }
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

/// `(context id, frame id)` of an `executionContextCreated` event when it is a
/// default (main-world) frame context; `None` for isolated worlds and workers.
fn params_context(v: &serde_json::Value) -> Option<(i64, String)> {
    let ctx = v.pointer("/params/context")?;
    let aux = ctx.get("auxData")?;
    if aux.get("isDefault").and_then(|d| d.as_bool()) != Some(true) {
        return None;
    }
    let frame = aux.get("frameId")?.as_str()?.to_owned();
    Some((ctx.get("id")?.as_i64()?, frame))
}

/// A notification marker is trusted only from the page's main-frame default
/// context, or from a service-worker session we patched. Subframes, isolated
/// worlds and other child sessions cannot raise or close banners.
fn marker_trusted(st: &State, session: Option<&str>, v: &serde_json::Value) -> bool {
    match session {
        Some(s) => st.sw_sessions.contains(s),
        None => {
            let Some(ctx) = v
                .pointer("/params/executionContextId")
                .and_then(serde_json::Value::as_i64)
            else {
                return false;
            };
            match (st.default_contexts.get(&ctx), st.main_frame_id.as_deref()) {
                (Some(frame), Some(main)) => frame == main,
                _ => false,
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
        if p.event == "close" {
            log::debug!("cdp: notification closed by page (account={account_id})");
            crate::notifications::tracker().on_closed_for(&account_id, &p.tag);
            return;
        }
        let icon = if p.icon.is_empty() {
            None
        } else {
            Some(p.icon.as_str())
        };
        log::debug!("cdp: notification (account={account_id})");
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
    /// Empty for a new notification; `"close"` when the page closed `tag`.
    #[serde(default)]
    event: String,
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

/// Reconcile the *rendered* WhatsApp Web language against the override. The
/// probe reports the document language (`h`, e.g. `pt`) and stored pref (`p`).
/// We compare the rendered language — not just the stored pref — because
/// WhatsApp's service worker can keep serving a cached bundle even after the
/// pref is set. On a mismatch, set the pref and trigger ONE cache-bypassing
/// reload (a plain reload keeps the cached bundle). At most one reload per
/// browser (`lang_reloaded`).
fn reconcile_lang(state: &Rc<RefCell<State>>, host: &BrowserHost, probe: Option<&str>) {
    let desired = {
        let st = state.borrow();
        if st.lang_reloaded {
            return;
        }
        st.desired_lang.clone()
    };
    let (html_lang, pref) =
        match probe.and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok()) {
            Some(v) => (
                v.get("h")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_lowercase(),
                v.get("p").and_then(|x| x.as_str()).map(str::to_owned),
            ),
            None => (String::new(), None),
        };
    let base = |s: &str| s.split(['-', '_']).next().unwrap_or(s).to_lowercase();
    let needs_reload = match &desired {
        // Reload when the rendered language's base differs from the wanted base
        // (`pt_BR` → `pt`). Skip when `html_lang` is empty: the page hasn't set
        // it yet (probed too early), and treating "" as a mismatch caused a
        // spurious reload on every launch that disrupted the service-worker
        // notification patch (native "Chromium" notifications leaked).
        Some(code) => !html_lang.is_empty() && base(&html_lang) != base(code),
        // Clear only a previously-forced real value (not absent / WhatsApp's "null").
        None => matches!(pref.as_deref(), Some(c) if c != "null"),
    };
    if !needs_reload {
        return;
    }
    state.borrow_mut().lang_reloaded = true;

    // Switching WhatsApp Web's language needs THREE things together, then a
    // reload — any one alone is overridden on boot:
    //   1. purge the `wa_web_*pref*`/lang CacheStorage (it pins the old locale),
    //   2. set the `wa_web_lang_pref` cookie (underscore form, e.g. `pt_BR`),
    //   3. set `WALangUserPref` (hyphen form). For System Default, clear cookie +
    //      pref so WhatsApp falls back to the linked phone's language.
    // location.reload() runs after the async cache purge resolves; with the
    // pref-cache gone WhatsApp re-derives the locale from the cookie/phone.
    let purge = "var ks=await caches.keys();\
                 await Promise.all(ks.filter(k=>/pref|lang/i.test(k)).map(k=>caches.delete(k)));";
    let set_js = match &desired {
        Some(code) => format!(
            "(async()=>{{try{{{purge}}}catch(e){{}}\
             try{{document.cookie='wa_web_lang_pref={code};domain=.web.whatsapp.com;path=/;max-age=31536000';\
             localStorage.setItem('WALangUserPref',JSON.stringify({}));}}catch(e){{}}\
             location.reload();}})()",
            json_string(&crate::i18n::whatsapp_locale_for(code)),
        ),
        None => format!(
            "(async()=>{{try{{{purge}}}catch(e){{}}\
             try{{document.cookie='wa_web_lang_pref=;domain=.web.whatsapp.com;path=/;max-age=0';\
             localStorage.removeItem('WALangUserPref');}}catch(e){{}}\
             location.reload();}})()"
        ),
    };
    let id = state.borrow_mut().bump();
    send(
        host,
        &json_msg(id, "Runtime.evaluate", &eval_params(&set_js)),
    );
    log::info!("cdp: switching WhatsApp Web locale -> {desired:?}");
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
