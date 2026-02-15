# Karere — TODO

## Critical / High Severity

- [ ] **JavaScript injection via filenames** — Filenames from paste/drop are interpolated into JS without escaping. A crafted filename like `"; alert('xss'); //"` could inject arbitrary script in the WhatsApp Web context. Use `serde_json::json!(filename).to_string()` for proper JS string encoding.
  - `src/window.rs:1361, 1370`

- [ ] **Path traversal in downloads** — WebKit download filenames are joined to the download directory without sanitizing `../` sequences. Strip `..`, `/`, `\` or use `Path::file_name()`.
  - `src/window.rs:821`

- [ ] **`panic!()` in async notification proxy init** — Crashes the entire app if the XDG notification portal fails. Replace with graceful error handling.
  - `src/window.rs:1155`

## Medium Severity

- [ ] **6 keyboard shortcuts advertised but never implemented** — Ctrl+M (minimize), F11 (fullscreen), Ctrl+Shift+H (high contrast), Ctrl+Shift+F (focus indicators), Ctrl+Shift+N (notifications), Ctrl+Shift+D (DND). Users see them in the shortcut dialog but they do nothing.
  - `data/ui/keyboard-shortcuts.blp:48-51, 119-126, 153-161`

- [ ] **Unsafe `env::set_var`** — Modifies global mutable state; unsafe in Rust 2024 edition.
  - `src/main.rs:49-51`

- [ ] **Symlink/TOCTOU race in `copy_dir_recursive`** — No symlink validation before traversal during account session migration.
  - `src/accounts.rs:16-29`

- [ ] **Non-atomic account file read/write** — Concurrent account operations could corrupt the JSON file. No file locking. Consider the `fs2` crate.
  - `src/accounts.rs:147-166`

- [ ] **Notification ID delimiter parsing** — Uses `find(':')` which breaks if the notification tag itself contains `:`. Should use a unique delimiter or `splitn`.
  - `src/window.rs:2029-2033`

- [ ] **Predictable temp filenames** — `karere-notify.oga` and `karere-preview.oga` in `/tmp` are fixed names, vulnerable to symlink attacks. Use the `tempfile` crate.
  - `src/window.rs:1233`, `src/preferences.rs:450`

- [ ] **Directory creation without explicit permissions** — Config/account dirs inherit the process umask, potentially world-readable. Set explicit `0o700`.
  - `src/accounts.rs:120`

- [ ] **WebKit security not explicitly hardened** — No CSP enforcement, no restriction on `file://` access, no explicit CORS policies beyond WebKit defaults.
  - `src/window.rs:604-606`

- [ ] **Chained `.unwrap()` on action parameters** — `parameter.unwrap().get::<bool>().unwrap()` with no validation.
  - `src/main.rs:87, 232-234, 393-395`

- [ ] **`mobile_layout_transitioning` flag not reset on JS failure** — Can leave layout in an inconsistent state.
  - `src/window.rs:1563-1569`

## Low Severity

- [ ] **`#[allow(dead_code)]` on `match_locale_to_dictionary()`** — Function is never called. Remove or use it.
  - `src/spellcheck.rs:40`

- [ ] **Deprecated `FromIterator` import** — Should use `.collect()` idiom.
  - `src/spellcheck.rs:2, 32, 42`

- [ ] **Debug `println!` statements throughout** — ~20+ raw `println!`/`eprintln!` instead of structured logging. Consider the `tracing` crate.

- [ ] **Incomplete translations** — 7 untranslated strings in `es.po`, `ga.po`, `en_UK.po`, `en_US.po`.
  - `po/`

- [ ] **Screen reader status is static "Inactive"** — No runtime detection, misleading UI.
  - `data/ui/preferences.blp:300`

- [ ] **Release notes parsed via string search** — Fragile, no proper XML parsing; could panic on malformed input.
  - `src/main.rs:522-547`

- [ ] **build.rs fragility** — `to_str().unwrap()` on paths, no blueprint-compiler version check, no batch compilation.
  - `build.rs:12-27`

- [ ] **No JSON size limit** on account file reading.
  - `src/accounts.rs:135-137`
