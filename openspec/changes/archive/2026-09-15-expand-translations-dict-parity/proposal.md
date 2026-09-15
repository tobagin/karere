## Why

Karere v4's `po/` tree still carries v3's 9 languages, but several v4 source files (account manager, switcher, preferences, tray, CEF bridges) added new user-visible strings that are untranslated or not even extracted into the POT. Meanwhile the Chromium spellchecker exposes 50 dictionary locales (`src/spellcheck.rs` `KNOWN_LANGUAGES`), so a user can spell-check in Polish or Korean while the UI stays English. This brings UI translation coverage to parity with the dictionary list: every language you can spell-check in, you can also read the app in.

## What Changes

- Audit `po/POTFILES.in` against all v4 sources and regenerate `po/karere.pot` so every `gettext`/`tr`/blueprint `_()` string is captured (parity with v3 string set + v4 additions).
- Expand `po/LINGUAS` from 9 to 50 locales — one per `KNOWN_LANGUAGES` BCP-47 entry, mapped to gettext locale names (`en-AU`→`en_AU`, `es-419`→`es_419`, `pt-BR`→`pt_BR`, …).
- Add a `.po` file for each new locale (41 new files), `msgmerge`'d against the fresh POT.
- Machine-translate (LLM) all `msgstr` for every locale, including refreshing the existing 9.
- Re-build `.mo` output via meson so all 50 catalogs ship.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `i18n-gettext`: the "po tree copied from karere v3" requirement changes — `LINGUAS` and the `.po` set are no longer v3's 9 languages byte-for-byte but a 50-locale set aligned to the spellcheck dictionary list; POTFILES.in must list all v4 translatable sources.

## Impact

- `po/LINGUAS`, `po/POTFILES.in`, `po/karere.pot`, `po/*.po` (9 updated + 41 new).
- Build: meson `i18n.gettext` compiles 50 `.mo` files into `/app/share/locale`.
- Flatpak install size grows modestly (one `.mo` per locale).
- No Rust code change required; `src/spellcheck.rs` `KNOWN_LANGUAGES` is the source of truth for the locale list.
- Caveat: `es_419` is not a glibc locale name; its catalog only loads when `LANG`/`LANGUAGE` explicitly selects it. Documented, not blocking.
