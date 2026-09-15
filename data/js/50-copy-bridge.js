// 50-copy-bridge.js — outbound clipboard mirror (renderer side).
//
// CEF offscreen mode owns no real system clipboard, so page copy/selection
// never reaches the GDK clipboard / PRIMARY. Report selections to the host
// (SetClipboard), which writes them out:
//   * `copy` event   → regular clipboard (Ctrl+C / context-menu copy)
//   * `navigator.clipboard.writeText/write` → regular clipboard (WhatsApp's
//     own message menu "Copy")
//   * `selectionchange` (debounced) → PRIMARY selection (Linux middle-click)
//
// Wrapped in try/catch so a throw never takes down the renderer.
(function () {
  "use strict";

  function send(name, payload) {
    try {
      if (typeof window.karere_send === "function") {
        window.karere_send(name, JSON.stringify(payload));
      }
    } catch (e) {
      /* channel unavailable — drop silently */
    }
  }

  try {
    // Host-triggered explicit Copy reads selection synchronously in this renderer,
    // rather than racing the debounced PRIMARY mirror. Empty/collapsed selections
    // deliberately send nothing so an existing regular clipboard is preserved.
    function copyLiveSelection() {
      try {
        var sel = window.getSelection();
        var text = sel && !sel.isCollapsed ? sel.toString() : "";
        if (text) send("SetClipboard", { text: text, primary: false });
      } catch (_) {}
    }
    window.addEventListener("karere:copy-selection", copyLiveSelection, false);

    // WhatsApp's own message context menu ("Copy") writes through the async
    // Clipboard API, which in OSR only reaches Chromium's internal clipboard.
    // Mirror those writes to the host as well. (#178)
    var cb = typeof navigator !== "undefined" ? navigator.clipboard : null;
    if (cb && typeof cb.writeText === "function") {
      var origWriteText = cb.writeText;
      cb.writeText = function (text) {
        if (typeof text === "string" && text) {
          send("SetClipboard", { text: text, primary: false });
        }
        return origWriteText.apply(cb, arguments);
      };
    }
    if (cb && typeof cb.write === "function") {
      var origWrite = cb.write;
      cb.write = function (items) {
        try {
          Array.prototype.forEach.call(items || [], function (item) {
            if (item && item.types && item.types.indexOf("text/plain") !== -1) {
              item
                .getType("text/plain")
                .then(function (blob) { return blob.text(); })
                .then(function (text) {
                  if (text) send("SetClipboard", { text: text, primary: false });
                })
                .catch(function () {});
            }
          });
        } catch (_) {}
        return origWrite.apply(cb, arguments);
      };
    }

    // Keep listening for page-originated Copy operations. Host Ctrl+C/menu actions
    // dispatch the private host event above because OSR does not reliably emit
    // this DOM event.
    // (after the page sets clipboardData): prefer that data, else the selection.
    document.addEventListener(
      "copy",
      function (e) {
        try {
          var text = "";
          try {
            text = e.clipboardData && e.clipboardData.getData("text/plain");
          } catch (_) {}
          if (!text) {
            var sel = window.getSelection();
            text = sel && !sel.isCollapsed ? sel.toString() : "";
          }
          try {
            console.log("karere copy event: " + (text ? text.length + " chars" : "EMPTY"));
          } catch (_) {}
          if (text) {
            send("SetClipboard", { text: text, primary: false });
            // Stop Chromium's OSR-native copy (windowless, never reaches the
            // system clipboard) from clobbering our GDK write.
            e.preventDefault();
          }
        } catch (_) {}
      },
      false
    );

    // Selection → PRIMARY (debounced; selectionchange fires rapidly). Empty
    // selection sends nothing, so existing PRIMARY persists (X11 behavior).
    var timer = 0;
    document.addEventListener(
      "selectionchange",
      function () {
        try {
          if (timer) clearTimeout(timer);
          timer = setTimeout(function () {
            try {
              var sel = window.getSelection();
              var text = sel ? sel.toString() : "";
              if (text) send("SetClipboard", { text: text, primary: true });
            } catch (_) {}
          }, 50);
        } catch (_) {}
      },
      false
    );
  } catch (err) {
    try {
      console.error(
        "karere copy bridge failed: " + (err && err.stack ? err.stack : err)
      );
    } catch (e) {}
  }
})();
