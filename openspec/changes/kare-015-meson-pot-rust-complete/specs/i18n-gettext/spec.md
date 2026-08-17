## MODIFIED Requirements

### Requirement: Meson wires gettext domain

The meson build SHALL include the `po` subdir and wire the gettext domain `karere` via `po/meson.build`: it SHALL declare LINGUAS-driven per-locale `.mo` install targets for the domain `karere` that build to `karere.mo` and install to `<localedir>/<lang>/LC_MESSAGES/karere.mo` with `install_tag: 'i18n'` and a `karere-gmo` alias, and invoking the `karere-pot` target SHALL run `tools/update-po.sh` (the canonical two-pass extraction).

#### Scenario: meson build declares karere domain via hand-rolled wiring

- **WHEN** parsing the top-level `meson.build` and `po/meson.build`
- **THEN** the top-level contains `subdir('po')` and keeps `i18n = import('i18n')` (consumed by `data/meson.build` `i18n.merge_file`)
- **AND** `po/meson.build` parses `po/LINGUAS` via the `fs` module (skipping blank/`#` lines), and for each locale declares a `custom_target('karere-<lang>.mo', output: 'karere.mo', install_dir: get_option('localedir') / lang / 'LC_MESSAGES', install_tag: 'i18n')` that builds with `msgfmt -o @OUTPUT@ @INPUT@`, collecting them into `alias_target('karere-gmo', gmotargets)`
- **AND** it declares `run_target('karere-pot', tools/update-po.sh)` and `run_target('karere-update-po', tools/update-po.sh)` (both names preserved; both run the full regenerate+merge procedure)
- **AND** when `msgfmt` is absent it emits `Gettext not found, all translation (po) targets will be ignored.` and defines no po targets, without failing configure

#### Scenario: POT regeneration extracts strings from blp and rs

- **WHEN** the meson `karere-pot` (or equivalent `karere-update-po`) target is invoked
- **AND** `data/ui/*.blp` contains a translatable string
- **AND** `src/**/*.rs` contains a call to `gettext("...")` or `tr!("...")` (including fully-qualified `gettextrs::gettext`)
- **THEN** both strings appear as `msgid` entries in the regenerated `po/karere.pot` (Rust-complete: the target invoked `tools/update-po.sh`'s two-pass extraction — non-Rust auto-detect + Rust `--language=C --join-existing` — so all 23 qualified Rust msgids are present, total 513 msgids, and `tools/verify-po.sh` prints `ALL CHECKS PASSED`)
