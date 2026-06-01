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

### Requirement: po tree copied from karere v3

The project SHALL include a `po/` directory containing karere v3's `LINGUAS`, `POTFILES.in`, all `.po` files, and `meson.build`, copied byte-for-byte.

#### Scenario: po directory contents match v3

- **WHEN** listing `po/*.po`, `po/LINGUAS`, `po/POTFILES.in`, `po/meson.build`
- **THEN** every file present in `/home/tobagin/Projects/karere/po/` is present here with identical content

### Requirement: Meson wires gettext domain

The meson build SHALL declare the gettext domain `karere` and include the `po` subdir.

#### Scenario: meson.build declares karere domain

- **WHEN** parsing the top-level `meson.build`
- **THEN** it contains a call equivalent to `i18n.gettext('karere', preset: 'glib')`
- **AND** it contains `subdir('po')`

#### Scenario: POT regeneration extracts strings from blp and rs

- **WHEN** the meson `karere-pot` (or equivalent) target is invoked
- **AND** `data/ui/*.blp` contains a translatable string
- **AND** `src/**/*.rs` contains a call to `gettext("...")` or `tr!("...")`
- **THEN** both strings appear as `msgid` entries in the regenerated `po/karere.pot`

### Requirement: POTFILES.in lists blueprint and rust sources

`po/POTFILES.in` SHALL list every file that contains translatable strings, including `.blp` and `.rs` files.

#### Scenario: POTFILES.in includes blueprint sources

- **WHEN** inspecting `po/POTFILES.in`
- **THEN** entries for `data/ui/*.blp` files are present (as inherited verbatim from karere v3)

#### Scenario: POTFILES.in includes rust sources

- **WHEN** inspecting `po/POTFILES.in`
- **THEN** entries for `src/*.rs` files that contain user-visible strings are present
