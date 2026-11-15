// Report the page's header background so the host can tint its own chrome to
// match (`match-whatsapp-colors`), including under a user stylesheet.
(function () {
  "use strict";

  if (window.__karerePageBgInstalled) return;
  window.__karerePageBgInstalled = true;

  var last = null;
  // ponytail: a 1 s poll of one computed style. Catching theme edits with a
  // MutationObserver would mean watching every class and style change on the
  // page; switch to that only if the up-to-1 s lag matters.
  setInterval(function () {
    try {
      var header = document.querySelector("header");
      if (!header || typeof window.karere_send !== "function") return;
      var color = getComputedStyle(header).backgroundColor;
      if (color === last) return;
      last = color;
      window.karere_send("PageBackground", JSON.stringify({ color: color }));
    } catch (_e) {
      /* A cosmetic hint must never disturb the page. */
    }
  }, 1000);
})();
