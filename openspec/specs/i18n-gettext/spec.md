# i18n-gettext Specification

## Purpose

Defines gettext runtime initialisation, the `po/` tree layout copied from
karere v3, the meson wiring that registers the `karere` gettext domain,
and the POT extraction pipeline that pulls translatable strings out of
blueprint (`.blp` / compiled `.ui`) and Rust sources.
## Requirements
### Requirement: Gettext runtime initialization

The application SHALL initialize gettext at startup so that translatable strings load from the system locale.

#### Scenario: Locale and textdomain bound on startup

- **WHEN** `main()` runs before any GTK initialization
- **THEN** `setlocale(LocaleCategory::LcAll, "")` is called
- **AND** `bindtextdomain("karere", "/app/share/locale")` is called
- **AND** `textdomain("karere")` is called
- **AND** the call sequence matches the pattern in `/home/tobagin/Projects/karere/src/main.rs` lines 23-80

#### Scenario: Translated string round-trip

- **WHEN** `LANG=pt_BR.UTF-8` is set and a `.mo` file for `pt_BR` exists at `/app/share/locale/pt_BR/LC_MESSAGES/karere.mo`
- **THEN** `gettext("Karere")` returns the translated string (if the catalog provides one), otherwise returns `"Karere"` unchanged

### Requirement: po tree aligned to spellcheck dictionary set

The project SHALL include a `po/` directory whose `LINGUAS` and `.po` set cover one locale per Chromium spellcheck dictionary in `src/spellcheck.rs` `KNOWN_LANGUAGES`, using gettext locale names (BCP-47 `-` replaced with `_`, region upper-cased). Every listed locale SHALL have a `.po` file that compiles to a `.mo`.

#### Scenario: LINGUAS covers every dictionary locale

- **WHEN** comparing `po/LINGUAS` against `KNOWN_LANGUAGES` in `src/spellcheck.rs`
- **THEN** for each BCP-47 code there is a matching `LINGUAS` entry with `-` replaced by `_` (e.g. `en-AU`→`en_AU`, `es-419`→`es_419`, `pt-BR`→`pt_BR`)
- **AND** the only `LINGUAS` entries absent from `KNOWN_LANGUAGES` are v3-inherited locales whose translations are retained (`ar`, `ga`, `kk`, `it_IT`); no other extras are added. (Chromium ships no spellcheck dictionary for `ar`/`ga`/`kk`, so they remain translation-only locales; `it_IT` rides on the dict's `it`.)

#### Scenario: every locale has a compilable catalog

- **WHEN** `meson compile` (or `msgfmt`) runs over `po/*.po`
- **THEN** each `.po` produces a `.mo` without error
- **AND** the number of installed `karere.mo` catalogs under `/app/share/locale` equals the number of `LINGUAS` entries

#### Scenario: existing translations preserved

- **WHEN** the v3-inherited locales (`ar en_GB en_US es ga it_IT kk pt_BR pt_PT`) are merged against the regenerated POT
- **THEN** their previously translated `msgstr` values are retained (not blanked)

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

### Requirement: POTFILES.in lists all v4 translatable sources

`po/POTFILES.in` SHALL list every v4 source file containing translatable strings, including all `.rs` files with `gettext`/`tr!`/`ngettext` calls and all `data/ui/*.blp` (or compiled `.ui`) files with `_()` markup, plus the desktop and metainfo inputs.

#### Scenario: no extractable string is missed

- **WHEN** grepping `src/**/*.rs` for `gettext(`, `tr!(`, or `ngettext(` and `data/ui/*.blp` for `_(`
- **THEN** every file containing a hit appears in `po/POTFILES.in`

#### Scenario: regenerated POT captures the v4 string set

- **WHEN** the POT regeneration target runs after POTFILES.in is updated
- **THEN** `po/karere.pot` contains a `msgid` for each translatable string in the listed sources
- **AND** strings from v4-added modules (account, preferences, tray, CEF bridges) that were previously missing are present

