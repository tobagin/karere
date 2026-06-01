// 50-copy-bridge.js — outbound clipboard mirror (renderer side).
//
// CEF's offscreen (windowless) mode owns no real system clipboard, so copying
// or selecting text in the page never reaches the GDK clipboard / PRIMARY
// selection. This script reports page selections to the host, which writes them
// out (see RendererMessage::SetClipboard):
//   * `copy` event   → regular clipboard (Ctrl+C / context-menu copy)
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
    // Ctrl+C / context-menu copy → regular clipboard. Our listener runs in the
    // bubble phase (after the page sets clipboardData), so prefer the data the
    // page actually put on the clipboard, then fall back to the selection.
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
            text = sel ? sel.toString() : "";
          }
          try {
            console.log("karere copy event: " + (text ? text.length + " chars" : "EMPTY"));
          } catch (_) {}
          if (text) {
            send("SetClipboard", { text: text, primary: false });
            // Stop Chromium's OSR-native copy from clobbering our GDK clipboard
            // write (its windowless clipboard doesn't reach the system clipboard).
            e.preventDefault();
          }
        } catch (_) {}
      },
      false
    );

    // Selection → PRIMARY (debounced; selectionchange fires rapidly). An empty
    // selection sends nothing, leaving any existing PRIMARY intact (matches X11
    // behavior where the primary selection persists).
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
