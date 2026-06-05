// 70-notification-sound.js — gate WhatsApp Web's notification / UI "ding".
//
// WhatsApp plays its incoming-message tone (and other UI tones / call ringtone)
// by `play()`-ing an <audio> whose `src` is a static asset, e.g.
// `https://static.whatsapp.net/rsrc.php/…​.mp3`. The "Notification Sounds" toggle
// (and the master notification toggle) are surfaced here as
// `window.__karereMuteNotifSound` (set by the host on load and on change).
//
// When muted we block exactly those static-asset tones. We deliberately do NOT
// touch:
//   - WebRTC call audio  → played via `srcObject` (a MediaStream), not `src`.
//   - voice notes / media → `blob:` / media URLs, user-initiated playback.
// so disabling notification sounds never silences a call you answer or a voice
// message you tap.
(function () {
  "use strict";
  try {
    var proto = HTMLMediaElement && HTMLMediaElement.prototype;
    if (!proto || proto.__karereSoundHooked) return;
    proto.__karereSoundHooked = true;

    var origPlay = proto.play;
    proto.play = function () {
      try {
        if (window.__karereMuteNotifSound && !this.srcObject) {
          var s = this.currentSrc || this.src || "";
          if (s.indexOf("static.whatsapp.net") !== -1) {
            // Swallow the tone: return a resolved promise so callers awaiting
            // play() don't throw.
            return Promise.resolve();
          }
        }
      } catch (e) {
        /* fall through to real play on any error */
      }
      return origPlay.apply(this, arguments);
    };
  } catch (e) {
    /* never break the page over a sound hook */
  }
})();
