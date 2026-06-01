// notification_observer.js — intercept Web Notifications for Karere branding.
//
// WhatsApp Web calls `new Notification(title, opts)` to raise a banner. Left to
// Chromium, that banner is attributed to "Chromium", not Karere. To re-brand it
// we replace `window.Notification` with a Proxy whose `construct` trap:
//
//   - suppresses Chromium's native banner (it never constructs the real
//     `Notification`),
//   - forwards the full payload to the browser process as
//     `RendererMessage::NotificationSeen { account_id, title, body, icon, tag }`
//     (the browser re-emits a Karere-branded `gio::Notification`), and
//   - returns a `Notification`-shaped stub so page code that wires `onclick` /
//     `close()` / `addEventListener` keeps working.
//
// Two host entry points let the browser drive page state:
//   - `window.__karereCloseNotif(tag)`    — withdraw: close the matching stub.
//   - `window.__karereActivateNotif(tag)` — click: fire the stub's click so the
//     page opens the originating chat.
//
// SCOPE: this only covers the `new Notification()` constructor path. WhatsApp
// Web actually raises notifications from its SERVICE WORKER
// (`self.registration.showNotification`), a realm this page script cannot reach
// and for which CEF 148 exposes no interception hook — see
// openspec/changes/m14-notifications-sounds/tasks.md. Those banners are rendered
// natively by Chromium and are not branded/handled here.
//
// Runs inside the always-injected bundle (after 00-bootstrap.js), before any
// page script constructs a Notification, because the whole bundle is injected
// at V8 context creation.
(function () {
  "use strict";

  // Reuse the bootstrap bridge; fall back to the raw native binding.
  function send(name, payload) {
    try {
      if (window.karere && typeof window.karere.send === "function") {
        window.karere.send(name, payload === undefined ? "" : JSON.stringify(payload));
      } else if (typeof window.karere_send === "function") {
        window.karere_send(name, payload === undefined ? "" : JSON.stringify(payload));
      }
    } catch (e) {
      /* channel unavailable — drop silently */
    }
  }

  try {
    var OrigNotification = window.Notification;
    if (typeof OrigNotification !== "function") {
      // No Notification API in this context — nothing to wrap.
      return;
    }

    // Live stubs keyed by tag, so the host can close/activate by tag and the
    // page's repeat-by-tag notifications collapse onto one entry.
    var liveByTag = new Map();
    var tagSeq = 0;

    // A minimal Notification-shaped object. We never construct the real
    // Notification, so this stand-in carries the event surface the page uses.
    function NotifStub(title, opts) {
      opts = opts || {};
      this.title = title == null ? "" : String(title);
      this.body = opts.body == null ? "" : String(opts.body);
      this.icon = opts.icon == null ? "" : String(opts.icon);
      this.tag = opts.tag == null ? "__karere_" + (++tagSeq) : String(opts.tag);
      this.data = opts.data;
      this.onclick = null;
      this.onclose = null;
      this.onerror = null;
      this.onshow = null;
      this._listeners = { click: [], close: [], show: [], error: [] };
      this._closed = false;
    }
    NotifStub.prototype.addEventListener = function (type, cb) {
      if (this._listeners[type] && typeof cb === "function") {
        this._listeners[type].push(cb);
      }
    };
    NotifStub.prototype.removeEventListener = function (type, cb) {
      var list = this._listeners[type];
      if (!list) return;
      var i = list.indexOf(cb);
      if (i >= 0) list.splice(i, 1);
    };
    NotifStub.prototype.dispatchEvent = function (event) {
      var type = event && event.type;
      var handler = this["on" + type];
      try {
        if (typeof handler === "function") handler.call(this, event);
      } catch (e) {}
      var list = this._listeners[type] || [];
      for (var i = 0; i < list.length; i++) {
        try {
          list[i].call(this, event);
        } catch (e) {}
      }
      return true;
    };
    NotifStub.prototype._fire = function (type) {
      var event;
      try {
        event = new Event(type);
      } catch (e) {
        event = { type: type, target: this };
      }
      this.dispatchEvent(event);
    };
    NotifStub.prototype.close = function () {
      if (this._closed) return;
      this._closed = true;
      liveByTag.delete(this.tag);
      send("NotificationClosed", { tag: this.tag });
      this._fire("close");
    };

    // Resolve an avatar URL to an inline data URL renderer-side: the browser
    // process can't re-fetch a blob:/authenticated URL. Falls back to null.
    function resolveIcon(url) {
      if (!url) return Promise.resolve(null);
      if (/^data:/i.test(url)) return Promise.resolve(url);
      return fetch(url)
        .then(function (r) {
          return r.ok ? r.blob() : null;
        })
        .then(function (blob) {
          if (!blob) return null;
          return new Promise(function (resolve) {
            var reader = new FileReader();
            reader.onloadend = function () {
              resolve(typeof reader.result === "string" ? reader.result : null);
            };
            reader.onerror = function () {
              resolve(null);
            };
            reader.readAsDataURL(blob);
          });
        })
        .catch(function () {
          return null;
        });
    }

    // The page-supplied account identity (set by a future M20 hook); empty for
    // the single-account case so the browser falls back to the default account.
    function accountId() {
      return typeof window.__karereAccountId === "string" ? window.__karereAccountId : "";
    }

    function construct(title, opts) {
      var stub = new NotifStub(title, opts);
      // Overwrite any prior stub for this tag (WhatsApp reuses tags per chat).
      liveByTag.set(stub.tag, stub);

      resolveIcon(stub.icon).then(function (icon) {
        send("NotificationSeen", {
          account_id: accountId(),
          title: stub.title,
          body: stub.body,
          icon: icon,
          tag: stub.tag,
        });
      });
      return stub;
    }

    // Proxy the constructor: `construct` re-brands; `get`/`set`/`has` forward to
    // the real Notification so `Notification.permission` and
    // `Notification.requestPermission()` keep their native behaviour.
    var ProxyNotification = new Proxy(OrigNotification, {
      construct: function (_target, args) {
        return construct(args[0], args[1]);
      },
      get: function (target, prop, receiver) {
        return Reflect.get(target, prop, receiver);
      },
      set: function (target, prop, value, receiver) {
        return Reflect.set(target, prop, value, receiver);
      },
      has: function (target, prop) {
        return Reflect.has(target, prop);
      },
    });

    try {
      Object.defineProperty(window, "Notification", {
        configurable: true,
        writable: true,
        value: ProxyNotification,
      });
    } catch (e) {
      window.Notification = ProxyNotification;
    }

    // Host -> page: withdraw the banner for `tag` by closing its stub. Closing
    // posts NotificationClosed and drops the entry, keeping page state in sync
    // after the host already withdrew the platform notification.
    window.__karereCloseNotif = function (tag) {
      var stub = liveByTag.get(String(tag));
      if (stub) stub.close();
    };

    // Host -> page: the user clicked the Karere banner. Fire the stub's click so
    // the page's own handler navigates to the originating chat.
    window.__karereActivateNotif = function (tag) {
      var stub = liveByTag.get(String(tag));
      if (stub) stub._fire("click");
    };
  } catch (err) {
    var msg = "karere notification observer failed: " + (err && err.stack ? err.stack : err);
    try {
      send("ConsoleLog", { level: "error", msg: msg });
    } catch (e) {}
  }
})();
