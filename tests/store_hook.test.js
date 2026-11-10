// Regression harness for data/js/60-store-hook.js: exact wid matching, name
// fallback, poll back-off and avatar validation. Uses only Node built-ins.
"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");

const source = fs.readFileSync("data/js/60-store-hook.js", "utf8");
const tick = () => new Promise((r) => setImmediate(r));
const flush = async () => {
  for (let i = 0; i < 20; i++) await tick();
};

function load({ rows, wid, eurl, fetchImpl, databases }) {
  const sent = [];
  const timers = [];
  const store = { "last-wid-md": JSON.stringify(wid) };
  if (eurl) store.WACachedProfilePicEURL = JSON.stringify(eurl);
  const idb = {
    opened: 0,
    open() {
      idb.opened++;
      const req = {};
      setImmediate(() =>
        req.onsuccess && req.onsuccess({
          target: req,
        })
      );
      req.result = {
        close() {},
        transaction: () => ({
          objectStore: () => ({
            getAll() {
              const rq = { result: rows };
              setImmediate(() => rq.onsuccess());
              return rq;
            },
          }),
        }),
      };
      return req;
    },
    databases: () => Promise.resolve(databases || [{ name: "model-storage" }]),
  };
  const context = {
    window: {
      karere_send(name, json) {
        sent.push({ name, payload: json ? JSON.parse(json) : null });
      },
    },
    localStorage: { getItem: (k) => (k in store ? store[k] : null) },
    indexedDB: idb,
    fetch: fetchImpl || (() => Promise.reject(new Error("no fetch"))),
    FileReader: class {
      readAsDataURL() {
        this.result = "data:image/jpeg;base64,QUJD";
        Promise.resolve().then(() => this.onload());
      }
    },
    setTimeout(cb, ms) {
      timers.push({ cb, ms });
      return timers.length;
    },
  };
  vm.createContext(context);
  vm.runInContext(source, context, { filename: "60-store-hook.js" });
  return { sent, timers, idb, store };
}

const identities = (sent) => sent.filter((m) => m.name === "ProfileIdentity").map((m) => m.payload);

(async () => {
  const wid = "15550001234:19@c.us";

  // Exact match only: a group id containing the number must not win; the named
  // exact row is preferred over an unnamed exact row.
  {
    const rows = [
      { id: "15550001234-1700000000@g.us", name: "Some Group" },
      { id: "115550001234@c.us", name: "Other Person" },
      { id: "15550001234@c.us" },
      { id: { _serialized: "15550001234@c.us", user: "15550001234", server: "c.us" }, pushname: "Me" },
    ];
    const { sent } = load({ rows, wid });
    await flush();
    assert.deepEqual(identities(sent), [{ wid: "15550001234@c.us", pushname: "Me", source: "store" }]);
  }

  // No named row: fall back to the phone number so pairing state clears.
  {
    const { sent, timers } = load({ rows: [{ id: "15550001234@c.us" }, { id: "x@c.us", name: "Nope" }], wid });
    await flush();
    assert.deepEqual(identities(sent), [{ wid: "15550001234@c.us", pushname: "15550001234", source: "store" }]);
    // Identity emitted: the next poll is scheduled at ~60 s, not 3 s.
    assert.equal(timers[timers.length - 1].ms, 60000);
  }

  // Unpaired (no wid): the poll stays fast and AwaitingPairing is announced.
  {
    const { sent, timers } = load({ rows: [], wid: "" });
    await flush();
    assert.equal(timers[timers.length - 1].ms, 3000);
    timers.filter((t) => t.ms === 500).forEach((t) => t.cb());
    assert.ok(sent.some((m) => m.name === "AwaitingPairing"));
  }

  // A missing model-storage database is never opened (no empty DB created).
  {
    const { sent, idb } = load({ rows: [], wid, databases: [{ name: "other" }] });
    await flush();
    assert.equal(idb.opened, 0);
    assert.equal(identities(sent)[0].pushname, "15550001234");
  }

  // Avatar: error responses are not forwarded; images are.
  {
    const bad = () => Promise.resolve({ ok: false, status: 403, headers: { get: () => "text/html" }, blob: () => Promise.resolve({}) });
    const { sent } = load({ rows: [], wid, eurl: "https://pps.example/a", fetchImpl: bad });
    await flush();
    assert.equal(sent.filter((m) => m.name === "ProfileAvatar").length, 0);
    assert.ok(sent.some((m) => m.name === "ConsoleLog"));

    const html200 = () => Promise.resolve({ ok: true, status: 200, headers: { get: () => "text/html" }, blob: () => Promise.resolve({}) });
    const r2 = load({ rows: [], wid, eurl: "https://pps.example/a", fetchImpl: html200 });
    await flush();
    assert.equal(r2.sent.filter((m) => m.name === "ProfileAvatar").length, 0);

    const good = () => Promise.resolve({ ok: true, status: 200, headers: { get: () => "image/jpeg" }, blob: () => Promise.resolve({}) });
    const r3 = load({ rows: [], wid, eurl: "https://pps.example/a", fetchImpl: good });
    await flush();
    assert.deepEqual(r3.sent.filter((m) => m.name === "ProfileAvatar").map((m) => m.payload), [
      { base64_png: "QUJD", source: "store" },
    ]);
  }

  // Avatar removed: a later tick without the URL sends an empty clear, once.
  {
    const good = () => Promise.resolve({ ok: true, status: 200, headers: { get: () => "image/jpeg" }, blob: () => Promise.resolve({}) });
    const r = load({ rows: [], wid, eurl: "https://pps.example/a", fetchImpl: good });
    await flush();
    delete r.store.WACachedProfilePicEURL;
    r.timers[r.timers.length - 1].cb();
    await flush();
    r.timers[r.timers.length - 1].cb();
    await flush();
    assert.deepEqual(r.sent.filter((m) => m.name === "ProfileAvatar").map((m) => m.payload), [
      { base64_png: "QUJD", source: "store" },
      { base64_png: "", source: "store" },
    ]);
    // Never had a picture: nothing is sent.
    const r0 = load({ rows: [], wid });
    await flush();
    assert.equal(r0.sent.filter((m) => m.name === "ProfileAvatar").length, 0);
  }

  console.log("store hook: ok");
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
