## MODIFIED Requirements

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

### Requirement: POTFILES.in lists all v4 translatable sources

`po/POTFILES.in` SHALL list every v4 source file containing translatable strings, including all `.rs` files with `gettext`/`tr!`/`ngettext` calls and all `data/ui/*.blp` (or compiled `.ui`) files with `_()` markup, plus the desktop and metainfo inputs.

#### Scenario: no extractable string is missed

- **WHEN** grepping `src/**/*.rs` for `gettext(`, `tr!(`, or `ngettext(` and `data/ui/*.blp` for `_(`
- **THEN** every file containing a hit appears in `po/POTFILES.in`

#### Scenario: regenerated POT captures the v4 string set

- **WHEN** the POT regeneration target runs after POTFILES.in is updated
- **THEN** `po/karere.pot` contains a `msgid` for each translatable string in the listed sources
- **AND** strings from v4-added modules (account, preferences, tray, CEF bridges) that were previously missing are present
