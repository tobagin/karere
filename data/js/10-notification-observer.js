// notification_observer.js — re-brand Web Notifications as Karere.
//
// `new Notification()` would be attributed to "Chromium". Replace
// window.Notification with a Proxy whose `construct` trap suppresses the native
// banner, forwards NotificationSeen (browser re-emits a branded
// gio::Notification), and returns a Notification-shaped stub so page
// onclick/close()/addEventListener keep working. Host drives the stub via
// __karereCloseNotif(tag) (withdraw) and __karereActivateNotif(tag) (open chat).
//
// SCOPE: only the `new Notification()` path. Service-worker banners
// (self.registration.showNotification) live in an unreachable realm and CEF 148
// exposes no hook — they render natively, unbranded.
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
      // no Notification API here
      return;
    }

    // Live stubs keyed by tag (host closes/activates by tag; repeat tags
    // collapse onto one entry).
    var liveByTag = new Map();
    var tagSeq = 0;

    // Notification-shaped stand-in (we never construct the real Notification).
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

    // Resolve avatar URL to an inline data URL: the browser can't re-fetch a
    // blob:/authenticated URL. Falls back to null.
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

    // Page-supplied account identity; empty => browser uses default account.
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

    // Proxy: `construct` re-brands; get/set/has forward so .permission and
    // requestPermission() keep native behaviour.
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

    // Host -> page: withdraw banner for `tag` (closes its stub).
    window.__karereCloseNotif = function (tag) {
      var stub = liveByTag.get(String(tag));
      if (stub) stub.close();
    };

    // Host -> page: banner clicked — fire the stub's click to open the chat.
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
