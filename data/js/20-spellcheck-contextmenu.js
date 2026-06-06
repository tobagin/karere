// 20-spellcheck-contextmenu.js — let Chromium's native context menu (spellcheck
// suggestions + "Add to dictionary") show inside editable fields.
//
// WhatsApp's composer contextmenu handler preventDefault()s, killing the native
// menu. A capture-phase window listener sees the event first; on editable
// targets it stopImmediatePropagation() (so WhatsApp's bubble-phase handler
// never preventDefault()s) but never preventDefault()s itself, so Chromium
// renders its menu. Non-editable targets: untouched.
(function () {
  "use strict";

  try {
    window.addEventListener(
      "contextmenu",
      function (e) {
        try {
          const t = e.target;
          if (!t || typeof t.closest !== "function") {
            return;
          }
          const editable = t.closest('[contenteditable="true"], input, textarea');
          if (editable) {
            e.stopImmediatePropagation();
          }
        } catch (_err) {
          /* never let this break the page's own menu handling */
        }
      },
      true, // capture: run before the page's bubble-phase listeners
    );
  } catch (_e) {
    /* addEventListener unavailable — nothing to do */
  }
})();
