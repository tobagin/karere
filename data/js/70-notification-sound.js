// 70-notification-sound.js — gate WhatsApp Web's notification / UI "ding".
//
// WhatsApp plays tones by play()-ing an <audio> whose src is a static asset
// (static.whatsapp.net/…​.mp3). The mute toggle is window.__karereMuteNotifSound
// (set by the host on load and on change).
//
// When muted we block only those static-asset tones, NOT:
//   - WebRTC call audio  → srcObject (MediaStream), not src.
//   - voice notes / media → blob:/media URLs, user-initiated.
// so muting never silences a call you answer or a voice message you tap.
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
            // Swallow the tone; resolved promise so awaiters don't throw.
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
