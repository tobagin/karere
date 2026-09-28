// Keep the static masked wallpaper out of the conversation's scrolling raster
// work. On a large DPR-2 viewport it otherwise limits updates to about 24/s.
(function () {
  "use strict";

  function install() {
    try {
      const id = "karere-conversation-wallpaper";
      if (document.getElementById(id)) return;

      const style = document.createElement("style");
      style.id = id;
      // WhatsApp puts the mask URL inline on an empty child of #main. Scope the
      // hint to that decoration, never messages, media or the scroll container.
      // CSS follows chat/background replacement without polling or observing
      // every message mutation. A missing/changed markup match is a safe no-op.
      style.textContent = `
        #main > div:empty[style*="mask-image: url("] {
          will-change: transform !important;
        }
      `;
      (document.head || document.documentElement).appendChild(style);
    } catch (_err) {
      /* A rendering hint must never prevent the page from loading. */
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", install, { once: true });
  } else {
    install();
  }
})();
