//! Chrome DevTools Protocol bridge for service-worker notifications (M14).
//!
//! WhatsApp Web raises notifications from its **service worker**
//! (`self.registration.showNotification`). That realm is unreachable from the
//! page-injected observer, and CEF 148 exposes no notification API and no
//! service-worker context hook. The one mechanism that *does* reach it is CDP:
//! the process already runs `--remote-debugging-port` (see [`crate::devtools`]),
//! and CDP can attach to the service-worker target and evaluate code **inside
//! the SW global**.
//!
//! This module runs a background thread that:
//!
//!  1. fast-polls `http://127.0.0.1:PORT/json/list` for the running
//!     `service_worker` target (WhatsApp's SW is ephemeral — Chromium stops it
//!     when idle and restarts it to handle a push),
//!  2. attaches directly to its `webSocketDebuggerUrl`, registers a
//!     `Runtime.addBinding("__karereNotify")` and evaluates a patch overriding
//!     `showNotification` (forwards the payload through the binding, suppresses
//!     the native banner by never calling the real method), and
//!  3. **stays attached** — an attached DevTools session keeps the worker alive,
//!     so once patched the SW never goes cold again and every subsequent
//!     notification is branded, and
//!  4. on each `Runtime.bindingCalled`, hops to the glib main thread and emits a
//!     Karere-branded `gio::Notification` via [`crate::notifications`].
//!
//! NOTE: CEF 148's browser-level CDP endpoint does not implement the `Target`
//! domain (it accepts the WS handshake but answers no commands), so the
//! race-free `Target.setAutoAttach` + `waitForDebuggerOnStart` approach is not
//! available — hence the poll-and-attach design above. A notification that
//! arrives on a fully cold SW, before the first attach, can still surface
//! natively once; after that the persistent attachment keeps the patch live.
//!
//! The WebSocket client is hand-rolled (RFC 6455, text frames, client-masked)
//! to avoid pulling a new dependency into the vendored flatpak build.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;

/// JS evaluated inside the service-worker global. Overrides `showNotification`
/// to (a) resolve the avatar to a data URL using the SW's credentialed `fetch`,
/// (b) forward the payload through the `__karereNotify` CDP binding, and (c)
/// suppress the native banner by never calling the real method. Idempotent.
/// Marker line printed by the override via `console.log`. The host parses
/// `Runtime.consoleAPICalled` for this prefix. Using console as the transport
/// (instead of `Runtime.addBinding`) sidesteps the binding's per-session
/// lifecycle: a binding only delivers `bindingCalled` to the exact session that
/// registered it, and an override installed by one (possibly transient) session
/// keeps a dead reference after that session closes. `console.log` is delivered
/// as `consoleAPICalled` to whatever session has `Runtime.enable`d the realm.
const NOTIF_PREFIX: &str = "__KARERE_NOTIF__:";

const SW_PATCH: &str = r#"
(function () {
  var PREFIX = "__KARERE_NOTIF__:";

  // Suppress the native banner (never call the real method) and forward the
  // payload to the host via console.log, which CDP surfaces as
  // Runtime.consoleAPICalled. Re-resolving everything per call so the function
  // has no captured cross-session state.
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
    var iconUrl = opts.icon;
    if (iconUrl) {
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
  // Tag the override so we can detect (and replace) a stale one from a prior
  // eval rather than skipping via a boolean flag that could lock in a dead fn.
  override.__karere = true;

  var did = [];

  // Patch the PROTOTYPE — WhatsApp's handler calls `registration.showNotification`
  // resolved via the prototype, so an own-property patch alone is bypassed.
  try {
    var P = (self.ServiceWorkerRegistration && self.ServiceWorkerRegistration.prototype) || null;
    if (P && typeof P.showNotification === "function" && !P.showNotification.__karere) {
      P.showNotification = override;
      did.push("proto");
    }
  } catch (e) {}

  // Belt-and-suspenders: the live instance's own property too.
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

/// Patch evaluated in the **page** realm. WhatsApp Web raises message
/// notifications with the `new Notification(title, opts)` constructor in the
/// page (confirmed via CDP tracing — not the service worker, not `push`). The
/// build-time bundle observer is meant to do this but does not reliably win the
/// race against the page's own capture of `window.Notification`; evaluating this
/// via CDP after load is deterministic. Replaces `window.Notification` with a
/// Proxy whose `construct` trap suppresses Chromium's native banner (never
/// constructs the real one), forwards the payload via `console.log` (surfaced as
/// `Runtime.consoleAPICalled`, same transport as the SW patch), and returns a
/// `Notification`-shaped stub so page code wiring `onclick`/`close` still works.
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

    function resolveIcon(url) {
      if (!url) return Promise.resolve("");
      if (/^data:/i.test(url)) return Promise.resolve(url);
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
      // WhatsApp (notably Flow/business bots) fires a message notification TWICE
      // with the same tag: the real preview body, then an empty body. The empty
      // one would overwrite the real (same tag) leaving the host's "New message"
      // fallback. Skip an empty body ONLY when a real one already fired for this
      // tag — a standalone empty (a bare ping) still shows so it isn't silent.
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

    // Host -> page hooks for withdraw / click routing by tag.
    window.__karereCloseNotif = function (tag) { var s = liveByTag.get(String(tag)); if (s) s.close(); };
    window.__karereActivateNotif = function (tag) { var s = liveByTag.get(String(tag)); if (s) s._fire("click"); };

    return "patched-page";
  } catch (e) {
    return "error:" + (e && e.message ? e.message : e);
  }
})();
"#;

/// Start the CDP service-worker bridge on a detached background thread. Safe to
/// call once after the browser process is initialized; returns immediately.
pub fn start(port: u16) {
    std::thread::Builder::new()
        .name("karere-cdp".into())
        .spawn(move || supervise(port))
        .expect("spawn cdp thread");
}

/// Supervise loop: connect to the **browser-level** CDP endpoint and run the
/// auto-attach session; reconnect on any drop.
///
/// Why browser-level + auto-attach (not poll-the-SW-target): WhatsApp's SW is
/// stopped when idle. A push wakes it and its handler calls `showNotification`
/// *immediately on startup* — so any approach that polls for a running target
/// and then injects loses the race (the native banner fires first). With
/// `Target.setAutoAttach { waitForDebuggerOnStart }` Chromium pauses every SW
/// (and every restart) the instant it starts, before its code runs; we inject,
/// then release it. No race.
fn supervise(port: u16) {
    loop {
        match browser_ws(port) {
            Some(url) => {
                if let Err(e) = run_session(&url) {
                    log::debug!("cdp: session ended: {e}");
                }
            }
            None => log::debug!("cdp: browser endpoint not ready"),
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Browser-level session: arm auto-attach, then pump events. Each
/// service-worker `attachedToTarget` triggers an inject (paused at startup);
/// each `bindingCalled` dispatches a notification.
fn run_session(ws_url: &str) -> Result<(), String> {
    let mut ws = WsClient::connect(ws_url)?;
    let mut next_id: u32 = 100;

    // Do NOT pause the SW at startup (waitForDebuggerOnStart) — at that point
    // `self.ServiceWorkerRegistration` isn't populated yet, so the patch finds
    // nothing to wrap. Instead let it run and patch the moment each execution
    // context appears (and re-patch defensively). The patch suppresses the
    // banner inside showNotification, so even if WhatsApp calls it slightly
    // before we wrap, only that single first banner can leak.
    ws.send_text(&json_msg(
        1,
        "Target.setAutoAttach",
        "{\"autoAttach\":true,\"waitForDebuggerOnStart\":false,\"flatten\":true}",
    ))?;
    log::info!("cdp: auto-attach armed for service workers");

    // Browser-level auto-attach only covers workers / service workers, NOT
    // top-level page targets — but WhatsApp raises its message notifications via
    // `new Notification()` in the PAGE realm, so we must reach the page too. The
    // build-time bundle observer can't (Notification is not yet exposed at
    // document-start in this CEF context). Discover targets explicitly and attach
    // each WhatsApp page; `Target.targetCreated` also fires for pages spawned
    // later (account switches, full WhatsApp reloads), so re-patching is covered.
    ws.send_text(&json_msg(2, "Target.setDiscoverTargets", "{\"discover\":true}"))?;
    // Also snapshot existing targets now: a page already loaded before we armed
    // discovery would otherwise be missed (its `targetCreated` already fired).
    ws.send_text(&json_msg(3, "Target.getTargets", "{}"))?;
    log::info!("cdp: target discovery armed for page realms");

    // Sessions we have set up, mapped to which patch to apply on each
    // `Runtime.executionContextCreated` for that session (globals — the SW
    // registration, or the page's `window.Notification` — are only guaranteed
    // ready by then). WhatsApp raises message notifications via
    // `new Notification()` in the PAGE realm (confirmed by CDP tracing), so the
    // page patch is the one that matters; the SW patch covers the
    // `showNotification` path defensively.
    let mut patched: std::collections::HashMap<String, Patch> =
        std::collections::HashMap::new();
    // Page targetIds we've already issued an attach for (dedup — we re-snapshot
    // targets on every churn event, so the same page would otherwise re-attach).
    let mut attached_pages: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    loop {
        let msg = ws.recv_text()?;
        let v: serde_json::Value = match serde_json::from_str(&msg) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");

        match method {
            // A page target appeared (existing one at discovery time, or a new
            // one from a reload / account switch). Attach so its realm receives
            // the page Notification patch. SW/workers arrive via auto-attach.
            // Any target churn (workers/SW/page) — re-snapshot the target list.
            // Browser-level auto-attach only grabs the page if it already existed
            // when we armed; a page created later (we start in background, page
            // appears when the window opens) is NOT auto-attached, and its
            // `targetCreated` is not reliably typed "page" here. Polling
            // `getTargets` on churn and attaching unseen pages is robust to all of
            // it (created-after-arm, account switch, reload with a new id).
            "Target.targetCreated" => {
                ws.send_text(&json_msg(3, "Target.getTargets", "{}"))?;
            }
            // Snapshot response: attach any not-yet-attached page target. WhatsApp
            // raises message notifications via `new Notification()` in the PAGE
            // realm, so the page patch is the one that matters.
            _ if v.get("id").and_then(|i| i.as_u64()) == Some(3) => {
                if let Some(infos) = v
                    .get("result")
                    .and_then(|r| r.get("targetInfos"))
                    .and_then(|t| t.as_array())
                {
                    for t in infos {
                        let ttype = t.get("type").and_then(|x| x.as_str()).unwrap_or("");
                        let tid = t.get("targetId").and_then(|x| x.as_str()).unwrap_or("");
                        if ttype == "page" && !tid.is_empty() && !attached_pages.contains(tid) {
                            attached_pages.insert(tid.to_owned());
                            let params =
                                format!("{{\"targetId\":{},\"flatten\":true}}", json_string(tid));
                            log::info!("cdp: attaching page target {}", &tid[..tid.len().min(8)]);
                            ws.send_text(&json_msg(next_id, "Target.attachToTarget", &params))?;
                            next_id += 1;
                        }
                    }
                }
            }
            "Target.attachedToTarget" => {
                let params = v.get("params");
                let ttype = params
                    .and_then(|p| p.get("targetInfo"))
                    .and_then(|t| t.get("type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                let session = params
                    .and_then(|p| p.get("sessionId"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_owned();
                if session.is_empty() {
                    continue;
                }
                let which = match ttype {
                    "service_worker" => Some(Patch::Sw),
                    "page" => Some(Patch::Page),
                    _ => None,
                };
                if let Some(which) = which {
                    log::info!("cdp: {ttype} attached; arming {which:?} patch");
                    // Record BEFORE enabling Runtime: `Runtime.enable` emits
                    // `executionContextCreated` synchronously for the existing
                    // context, and that handler only patches known sessions.
                    patched.insert(session.clone(), which);
                    setup_session(&mut ws, &session, which, &mut next_id)?;
                }
            }
            // The realm is fully initialised — (re)apply its patch.
            "Runtime.executionContextCreated" => {
                let session = v.get("sessionId").and_then(|s| s.as_str()).unwrap_or("");
                if let Some(&which) = patched.get(session) {
                    log::info!("cdp: executionContextCreated session={session} patch={which:?}");
                    evaluate_patch(&mut ws, session, which, &mut next_id)?;
                }
            }
            "Runtime.consoleAPICalled" => {
                if let Some(payload) = console_payload(&v) {
                    dispatch(payload);
                }
            }
            _ => {
                // Diagnostic: surface eval results/exceptions/cmd errors so we
                // can confirm the patch evaluated inside the SW realm.
                if v.get("id").is_some() {
                    if let Some(val) = v
                        .get("result")
                        .and_then(|r| r.get("result"))
                        .and_then(|r| r.get("value"))
                    {
                        log::info!("cdp: eval result -> {val}");
                    } else if let Some(exc) =
                        v.get("result").and_then(|r| r.get("exceptionDetails"))
                    {
                        log::warn!("cdp: eval exception -> {exc}");
                    } else if let Some(err) = v.get("error") {
                        log::warn!("cdp: cmd error -> {err}");
                    }
                }
            }
        }
    }
}

/// Which notification patch a session should receive.
#[derive(Clone, Copy, Debug)]
enum Patch {
    /// Service-worker realm: override `registration.showNotification`.
    Sw,
    /// Page realm: override `window.Notification` (WhatsApp's actual path).
    Page,
}

impl Patch {
    fn js(self) -> &'static str {
        match self {
            Patch::Sw => SW_PATCH,
            Patch::Page => PAGE_PATCH,
        }
    }
}

/// Per-session setup: enable the Runtime domain so we receive
/// `executionContextCreated` and `consoleAPICalled` for this realm, then apply
/// the patch (covering an already-running context that emits no later event).
/// Both patches forward via `console.log` → `consoleAPICalled`, which has no
/// per-session binding lifecycle.
fn setup_session(
    ws: &mut WsClient,
    session: &str,
    which: Patch,
    next_id: &mut u32,
) -> Result<(), String> {
    ws.send_text(&json_msg_sess(*next_id, "Runtime.enable", "{}", session))?;
    *next_id += 1;
    evaluate_patch(ws, session, which, next_id)
}

/// Evaluate the (re-runnable) patch in the session's realm. Idempotent: each
/// patch guards against double-wrapping via its own marker.
fn evaluate_patch(
    ws: &mut WsClient,
    session: &str,
    which: Patch,
    next_id: &mut u32,
) -> Result<(), String> {
    let patch_params = format!(
        "{{\"expression\":{},\"returnByValue\":true}}",
        json_string(which.js())
    );
    ws.send_text(&json_msg_sess(*next_id, "Runtime.evaluate", &patch_params, session))?;
    *next_id += 1;
    Ok(())
}

/// Schedule a branded notification emit on the glib main thread.
fn dispatch(payload_json: String) {
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
        log::info!("cdp: SW notification tag={:?} title={:?}", p.tag, p.title);
        crate::notifications::tracker().on_seen(&p.tag, &p.title, &p.body, icon, "");
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

/// Query `/json/version` and return the browser-level `webSocketDebuggerUrl`
/// (the endpoint that speaks the `Target` domain for auto-attach).
fn browser_ws(port: u16) -> Option<String> {
    let body = http_get(port, "/json/version").ok()?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    v.get("webSocketDebuggerUrl")
        .and_then(|u| u.as_str())
        .map(|s| s.to_owned())
}

/// Extract our notification payload from a `Runtime.consoleAPICalled` event.
/// The SW override forwards via `console.log(NOTIF_PREFIX + json)`; here we find
/// a string argument starting with that prefix and return the trailing JSON.
fn console_payload(v: &serde_json::Value) -> Option<String> {
    let args = v.get("params")?.get("args")?.as_array()?;
    for a in args {
        if a.get("type").and_then(|t| t.as_str()) == Some("string") {
            if let Some(s) = a.get("value").and_then(|s| s.as_str()) {
                if let Some(rest) = s.strip_prefix(NOTIF_PREFIX) {
                    return Some(rest.to_owned());
                }
            }
        }
    }
    None
}

/// Build a CDP request frame `{"id":N,"method":M,"params":P}` where `params` is
/// a raw JSON object string.
fn json_msg(id: u32, method: &str, params: &str) -> String {
    format!("{{\"id\":{id},\"method\":\"{method}\",\"params\":{params}}}")
}

/// Like [`json_msg`] but addressed to a flattened child session via `sessionId`.
fn json_msg_sess(id: u32, method: &str, params: &str, session: &str) -> String {
    format!(
        "{{\"id\":{id},\"method\":\"{method}\",\"params\":{params},\"sessionId\":{}}}",
        json_string(session)
    )
}

/// JSON-encode `s` as a double-quoted string literal.
fn json_string(s: &str) -> String {
    serde_json::Value::String(s.to_owned()).to_string()
}

// ---- minimal HTTP GET (CDP discovery) -------------------------------------

const TIMEOUT: Duration = Duration::from_secs(2);

fn http_get(port: u16, path: &str) -> Result<String, String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream =
        TcpStream::connect_timeout(&addr, TIMEOUT).map_err(|e| format!("connect: {e}"))?;
    stream.set_read_timeout(Some(TIMEOUT)).ok();
    let req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n");
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("write: {e}"))?;
    let mut buf = Vec::with_capacity(8192);
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        if let Some(pos) = find_sub(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
        let n = stream.read(&mut chunk).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            return Err("closed before headers".into());
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let headers = String::from_utf8_lossy(&buf[..header_end]).to_ascii_lowercase();
    let len = headers
        .split("content-length:")
        .nth(1)
        .and_then(|s| s.split("\r\n").next())
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let total = header_end + len;
    while buf.len() < total {
        let n = stream.read(&mut chunk).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    Ok(String::from_utf8_lossy(&buf[header_end..total]).into_owned())
}

fn find_sub(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

// ---- minimal WebSocket client (RFC 6455) ----------------------------------

struct WsClient {
    stream: TcpStream,
}

impl WsClient {
    /// Connect + perform the opening handshake. `url` is `ws://host:port/path`.
    fn connect(url: &str) -> Result<Self, String> {
        let rest = url.strip_prefix("ws://").ok_or("not a ws:// url")?;
        let (hostport, path) = rest.split_once('/').unwrap_or((rest, ""));
        let (host, port) = hostport.split_once(':').ok_or("no port in ws url")?;
        let port: u16 = port.parse().map_err(|_| "bad port")?;
        let addr = SocketAddr::from((
            host.parse::<std::net::Ipv4Addr>().map_err(|e| e.to_string())?,
            port,
        ));
        let mut stream =
            TcpStream::connect_timeout(&addr, TIMEOUT).map_err(|e| format!("connect: {e}"))?;
        // Long-lived: no read timeout (we block on events).
        stream.set_nodelay(true).ok();

        let key = B64.encode(rand16());
        let req = format!(
            "GET /{path} HTTP/1.1\r\nHost: {hostport}\r\nUpgrade: websocket\r\n\
             Connection: Upgrade\r\nSec-WebSocket-Key: {key}\r\n\
             Sec-WebSocket-Version: 13\r\n\r\n"
        );
        stream
            .write_all(req.as_bytes())
            .map_err(|e| format!("handshake write: {e}"))?;

        // Read response headers (until CRLFCRLF).
        let mut buf = Vec::with_capacity(1024);
        let mut one = [0u8; 1];
        loop {
            let n = stream.read(&mut one).map_err(|e| format!("handshake read: {e}"))?;
            if n == 0 {
                return Err("handshake closed".into());
            }
            buf.push(one[0]);
            if buf.ends_with(b"\r\n\r\n") {
                break;
            }
            if buf.len() > 8192 {
                return Err("handshake too large".into());
            }
        }
        let resp = String::from_utf8_lossy(&buf);
        if !resp.starts_with("HTTP/1.1 101") {
            return Err(format!("handshake not 101: {}", &resp[..resp.len().min(40)]));
        }
        Ok(Self { stream })
    }

    /// Send a masked text frame.
    fn send_text(&mut self, text: &str) -> Result<(), String> {
        let payload = text.as_bytes();
        let mut frame = Vec::with_capacity(payload.len() + 14);
        frame.push(0x81); // FIN + text opcode
        let mask = rand4();
        let n = payload.len();
        if n < 126 {
            frame.push(0x80 | n as u8);
        } else if n < 65536 {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(n as u16).to_be_bytes());
        } else {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&(n as u64).to_be_bytes());
        }
        frame.extend_from_slice(&mask);
        frame.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        self.stream
            .write_all(&frame)
            .map_err(|e| format!("send: {e}"))
    }

    /// Receive the next text message, reassembling continuation frames and
    /// answering control frames (ping/close) inline.
    fn recv_text(&mut self) -> Result<String, String> {
        let mut message: Vec<u8> = Vec::new();
        loop {
            let (fin, opcode, payload) = self.read_frame()?;
            match opcode {
                0x1 | 0x2 | 0x0 => {
                    message.extend_from_slice(&payload);
                    if fin {
                        return String::from_utf8(message).map_err(|e| e.to_string());
                    }
                }
                0x8 => return Err("ws closed by peer".into()),
                0x9 => self.send_pong(&payload)?, // ping → pong
                0xA => {}                          // pong → ignore
                other => return Err(format!("bad opcode {other}")),
            }
        }
    }

    fn read_frame(&mut self) -> Result<(bool, u8, Vec<u8>), String> {
        let mut h = [0u8; 2];
        self.read_exact(&mut h)?;
        let fin = h[0] & 0x80 != 0;
        let opcode = h[0] & 0x0f;
        let masked = h[1] & 0x80 != 0;
        let mut len = (h[1] & 0x7f) as usize;
        if len == 126 {
            let mut e = [0u8; 2];
            self.read_exact(&mut e)?;
            len = u16::from_be_bytes(e) as usize;
        } else if len == 127 {
            let mut e = [0u8; 8];
            self.read_exact(&mut e)?;
            len = u64::from_be_bytes(e) as usize;
        }
        let mut mask = [0u8; 4];
        if masked {
            self.read_exact(&mut mask)?;
        }
        let mut payload = vec![0u8; len];
        self.read_exact(&mut payload)?;
        if masked {
            for (i, b) in payload.iter_mut().enumerate() {
                *b ^= mask[i % 4];
            }
        }
        Ok((fin, opcode, payload))
    }

    fn send_pong(&mut self, data: &[u8]) -> Result<(), String> {
        let mut frame = Vec::with_capacity(data.len() + 6);
        frame.push(0x8A); // FIN + pong
        let mask = rand4();
        frame.push(0x80 | data.len().min(125) as u8);
        frame.extend_from_slice(&mask);
        frame.extend(
            data.iter()
                .take(125)
                .enumerate()
                .map(|(i, b)| b ^ mask[i % 4]),
        );
        self.stream
            .write_all(&frame)
            .map_err(|e| format!("pong: {e}"))
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), String> {
        self.stream.read_exact(buf).map_err(|e| format!("read: {e}"))
    }
}

/// 16 random bytes for the WS key. Uses the address of a stack local + a process
/// counter as a cheap entropy source (the key only needs to be unique per
/// handshake, not cryptographically strong — this is loopback).
fn rand16() -> [u8; 16] {
    let mut out = [0u8; 16];
    let seed = seed();
    for (i, b) in out.iter_mut().enumerate() {
        out_byte(b, seed, i);
    }
    out
}

fn rand4() -> [u8; 4] {
    let mut out = [0u8; 4];
    let seed = seed();
    for (i, b) in out.iter_mut().enumerate() {
        out_byte(b, seed, i);
    }
    out
}

fn out_byte(b: &mut u8, seed: u64, i: usize) {
    let x = seed.wrapping_mul(6364136223846793005).wrapping_add(i as u64 + 1);
    *b = (x >> ((i % 8) * 8)) as u8;
}

fn seed() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static CTR: AtomicU64 = AtomicU64::new(0x9e3779b97f4a7c15);
    let local = 0u8;
    let addr = std::ptr::addr_of!(local) as u64;
    CTR.fetch_add(0x2545f4914f6cdd1d, Ordering::Relaxed) ^ addr
}
