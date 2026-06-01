// 00-bootstrap.js — always-on renderer-side bridge.
//
// Runs first (lexical `00-` prefix) in every main-frame V8 context. It wires
// the host IPC channel exposed by the native `register_extension` handler
// (`window.karere.send(name, payloadJson)`), forwards console output to the
// browser process, and registers the browser->page DOM event listeners that
// later milestones (M17 paste bridge, M20 notification observer) build on.
//
// The whole body is wrapped in try/catch: a throw here must never take down
// the renderer or block the rest of the bundle.
(function () {
  "use strict";

  // `window.karere_send(name, json)` is the native function bound by the
  // render-process handler in on_context_created. Forward a typed message to
  // the browser process: `name` is the RendererMessage variant tag, `payload`
  // its inner fields (the native side wraps it into the envelope). Never routes
  // through console — that would recurse into the shim below.
  function send(name, payload) {
    try {
      if (typeof window.karere_send === "function") {
        window.karere_send(name, payload === undefined ? "" : JSON.stringify(payload));
      }
    } catch (e) {
      /* channel unavailable — drop silently */
    }
  }

  try {
    // Compatibility aliases over the native bridge.
    if (typeof window.karere_send === "function") {
      window.karere = { send: window.karere_send };
      window.cefQuery = window.karere_send;
    }

    // --- Console forwarding -------------------------------------------------
    // Replace console.log/warn/error with shims that mirror each call to the
    // browser process as RendererMessage::ConsoleLog while still invoking the
    // native console.
    ["log", "warn", "error"].forEach(function (level) {
      var original =
        typeof console[level] === "function"
          ? console[level].bind(console)
          : function () {};
      console[level] = function () {
        try {
          var parts = Array.prototype.map.call(arguments, function (a) {
            if (typeof a === "string") return a;
            try {
              return JSON.stringify(a);
            } catch (e) {
              return String(a);
            }
          });
          send("ConsoleLog", { level: level, msg: parts.join(" ") });
        } catch (e) {
          /* never let logging throw */
        }
        return original.apply(console, arguments);
      };
    });

    // --- Browser -> page DOM events ----------------------------------------
    // The renderer dispatcher converts inbound BrowserMessage variants into
    // these DOM events. Bodies are no-op stubs in M13; real handling lands in
    // M17 (paste) and M20 (notifications).
    document.addEventListener("karere:dispatch-paste", function (_ev) {
      // M17: consume _ev.detail = { mime, payload }
    });
    document.addEventListener("karere:close-notif", function (_ev) {
      // M20: consume _ev.detail = { tag }
    });
  } catch (err) {
    // Bootstrap failed — report via the channel and the native console, then
    // let the remainder of the bundle continue.
    var msg = "karere bootstrap failed: " + (err && err.stack ? err.stack : err);
    try {
      send("ConsoleLog", { level: "error", msg: msg });
    } catch (e) {}
    try {
      if (typeof console.error === "function") console.error(msg);
    } catch (e) {}
  }
})();
