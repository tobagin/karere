# Tasks for KARE-015 — Make meson karere-pot target Rust-complete

## 1. Rewrite po/meson.build; raise root meson floor
- [x] 1.1 Replace `i18n.gettext` with hand-rolled LINGUAS-driven `custom_target` + `alias_target('karere-gmo')` + `run_target('karere-pot'/'karere-update-po' → tools/update-po.sh)` with `msgfmt` warn-and-skip
- [x] 1.2 Raise `meson_version` 0.59.0 → 0.60.0 with comment, update `subdir('po')` comment, keep `i18n = import('i18n')`
- [x] 1.3 `meson setup build-pot` succeeds, `ninja -n karere-pot` resolves without cargo-build dep

## 2. Assert karere-pot is Rust-complete
- [x] 2.1 `meson compile -C build-pot karere-pot` invokes `tools/update-po.sh` and exits 0
- [x] 2.2 `tools/verify-po.sh` → `ALL CHECKS PASSED` (10 sentinels incl. "Mute notifications"), 513 msgids
- [x] 2.3 Churn-scope: only `POT-Creation-Date` changed, catalogs restored; `karere-update-po` also passes with no `~` backups

## 3. Assert .mo install parity
- [x] 3.1 `meson compile -C build-pot karere-gmo` builds 72 targets
- [x] 3.2 `DESTDIR=... meson install --tags i18n` installs 72 `karere.mo` at `share/locale/<lang>/LC_MESSAGES/karere.mo`
- [x] 3.3 `meson install --dry-run` shows 72 `karere.mo` entries (untagged install also includes them)

## 4. Documentation & openspec delta
- [x] 4.1 `po/README.md` Known-limitations bullet → routing contract (single canonical path, meson 0.60 floor, karere-update-po = full regenerate+merge)
- [x] 4.2 Create `openspec/changes/kare-015-meson-pot-rust-complete/{proposal.md,tasks.md,specs/i18n-gettext/spec.md}` with MODIFIED delta, `openspec validate --strict` passes
- [x] 4.3 `TESTING.md` add `meson compile -C build-pot karere-pot` + `verify-po.sh` acceptance, `CHANGELOG.md` Unreleased Fixed bullet

## 5. Verification
- [x] 5.1 `cargo fmt --all -- --check` clean, `tools/verify-po.sh` green, `openspec validate --strict` clean, meson scratch reconfigure clean
