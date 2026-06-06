// 00-bootstrap.js — always-on renderer bridge, runs first (lexical `00-`).
// Wires host IPC, forwards console, registers browser->page DOM listeners.
// Whole body try/catch'd so a throw never takes down the renderer.
(function () {
  "use strict";

  // Send a typed RendererMessage via native window.karere_send (bound in
  // on_context_created). Must not route through console (would recurse).
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

    // Mirror log/warn/error to the browser (ConsoleLog) and still call native.
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

    // Browser -> page DOM events (handled by later scripts in the bundle).
    document.addEventListener("karere:dispatch-paste", function (_ev) {
    });
    document.addEventListener("karere:close-notif", function (_ev) {
    });
  } catch (err) {
    var msg = "karere bootstrap failed: " + (err && err.stack ? err.stack : err);
    try {
      send("ConsoleLog", { level: "error", msg: msg });
    } catch (e) {}
    try {
      if (typeof console.error === "function") console.error(msg);
    } catch (e) {}
  }
})();
