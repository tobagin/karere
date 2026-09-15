# Change: Make meson karere-pot target Rust-complete (KARE-015)

## Why
`meson compile karere-pot` (via `i18n.gettext`) runs a single `xgettext` pass with per-extension language guessing, extracting only 17 of 40 Rust strings and silently dropping 23 fully-qualified `gettextrs::gettext("...")` msgids (490 vs canonical 513). The committed catalog is then corrupted if the target is used, forcing `po/README.md` to warn contributors away from the broken target. The two-pass extraction (`--language=C --join-existing` Rust join) that correctly handles both XML and Rust already exists in `tools/update-po.sh` (KARE-014); the meson wiring must be fixed to use it, closing the trap KARE-014 documented.

Planning verified from meson 1.11.2 source (`mesonbuild/modules/i18n.py` + `mesonbuild/scripts/gettext.py::run_potgen`) that the alternative "add `--language=C` via `i18n.gettext` args" is impossible — the module passes extra args verbatim to ONE `xgettext` invocation, and a single-pass `--language=C` destroys all 472 XML msgids (measured 41 total). The only viable fix is the task's second alternative: hand-roll `po/meson.build` so `karere-pot` and `karere-update-po` invoke `tools/update-po.sh`.

## What Changes
- Replace `po/meson.build`'s `i18n.gettext('karere', preset: 'glib')` with hand-rolled wiring that reproduces the module's outputs minus the broken pot machinery: parse `po/LINGUAS` via `fs.read()`, create per-locale `custom_target('karere-<lang>.mo', output: 'karere.mo', install_dir: <localedir>/<lang>/LC_MESSAGES, install_tag: 'i18n')` under `po/<lang>/LC_MESSAGES` subdirs (preserving `build/po/<lang>/LC_MESSAGES/karere.mo` layout), `alias_target('karere-gmo')`, and `run_target('karere-pot'/'karere-update-po', tools/update-po.sh)`. `msgfmt` absent → warn-and-skip (mirrors `i18n.gettext` strict-msgfmt behavior).
- Raise root `meson.build` `meson_version` floor `0.59.0 → 0.60.0` (`custom_target` `install_tag` requires 0.60.0) and update `subdir('po')` comment.
- Update docs: `po/README.md` Known-limitations bullet → routing contract (single canonical path via `tools/update-po.sh`, meson targets are aliases), `TESTING.md` add `meson compile -C build-pot karere-pot` + `verify-po.sh` acceptance, `CHANGELOG.md` Unreleased Fixed bullet, openspec MODIFIED delta for "Meson wires gettext domain".

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `i18n-gettext`: the "Meson wires gettext domain" requirement changes — meson no longer uses `i18n.gettext('karere', preset: 'glib')` verbatim; instead `po/meson.build` declares LINGUAS-driven per-locale `.mo` install targets for domain `karere` and `karere-pot` invokes `tools/update-po.sh` (still `subdir('po')`, still `<localedir>/<lang>/LC_MESSAGES/karere.mo`, still `install_tag: 'i18n'` and `karere-gmo` alias).

## Impact
- `po/meson.build` (hand-rolled), `meson.build` (version floor + comment), `po/*/LC_MESSAGES/meson.build` (72 per-locale subdir wrappers), `po/README.md`, `TESTING.md`, `CHANGELOG.md`, `openspec/changes/kare-015-meson-pot-rust-complete/specs/i18n-gettext/spec.md`.
- Build: `meson setup` now requires meson ≥0.60.0, `meson compile karere-pot` runs `tools/update-po.sh` (needs `blueprint-compiler`, `xgettext`, `msgmerge`), `meson compile karere-gmo` / `meson install --tags i18n` still installs 72 `karere.mo` at `<localedir>/<lang>/LC_MESSAGES/karere.mo` (Flathub `mv share/locale` invariant preserved). `msgfmt` absent → configure warns and skips po targets, no failure.
- No Rust code change; no new runtime behavior.
