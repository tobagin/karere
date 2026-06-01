// 20-spellcheck-contextmenu.js — let Chromium's native context menu (and its
// spellcheck suggestions + "Add to dictionary") appear inside editable fields.
//
// WhatsApp Web's composer registers its own `contextmenu` handler that calls
// `preventDefault()`, which makes Chromium suppress the native menu entirely —
// so the CEF context-menu handler never fires and the spellcheck suggestions
// Chromium already computed are never shown.
//
// Fix: register a CAPTURE-phase `contextmenu` listener on the window. Because
// this bundle runs in `on_context_created` (before any page script), our
// capture listener is the first to see the event. When the right-click lands on
// an editable element we call `stopImmediatePropagation()` so WhatsApp's own
// (bubble-phase) handler never runs and never `preventDefault()`s — letting the
// native menu, with spellcheck suggestions, show. Outside editable fields we do
// nothing, so WhatsApp's own message/menu UX is untouched.
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
          // Composer and any other rich-text/editable input.
          const editable = t.closest('[contenteditable="true"], input, textarea');
          if (editable) {
            // Stop WhatsApp's handler from preventing the native menu. We do NOT
            // call preventDefault(), so Chromium renders its spellcheck menu.
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
