# Translating MineForge Rust Edition

The launcher speaks 86 languages through sparse catalogs (`launcher/lang/*.json`):
a key that a catalog lacks shows its English source at run time. No catalog is
ever filled with dummy English (the philosophy of patch 0006), and a
translation is added only when someone who knows the language writes it.

## The flow

1. **Report.** `python3 tools/mfre_lang_report.py` runs the launcher's i18n
   test with details and prints, per language, how many of the strings the UI
   asks for (`tr()`/`trf()` literals, plus literals any catalog already
   translates) are missing or stale.
2. **Batch input.** `python3 tools/mfre_lang_report.py --dump pt-br` writes
   that language's missing keys, exactly as the test sees them, to a JSON list.
3. **Translate.** A sibling of `tools/mfre_lang_es.py` — one script per
   language, `tools/mfre_lang_<code>.py` — holds the translations in themed
   batches, checks that every `{hole}` of a value matches its key's and that no
   value is empty, and writes the catalog in its existing format
   (`ensure_ascii=False`, two-space indent). The script is the review unit:
   one pull request per language, reviewers read the diff.
4. **Gate.** `HORIZON_LAUNCHER_I18N_DETAILS=1 cargo test --manifest-path
   launcher/Cargo.toml --lib i18n::tests -- --nocapture` must stop listing the
   language, and `translations_are_valid_and_coverage_is_reported` must pass:
   holes match, nothing empty, nothing stale.

New UI strings land in the English source and, where the patch author can, in
Spanish in the same patch; every other language catches up through the flow
above, never by blocking a patch.

## The set the fork keeps complete

- **es — complete** (Fase 16, 1759 keys): the fork's home language; every
  string the UI asks for is translated.
- **pt-br, fr, de — next in line**, in that order: the largest communities
  behind Spanish among the fork's expected players. Each becomes "complete"
  when its batch script covers the report's list and a reviewer signs off.
- Everything else stays sparse on purpose and keeps working: untranslated
  strings show their English source.

A hosted service (Crowdin or similar) is deliberately not adopted yet: with
one reviewer per language the script flow above is faster and every change
stays reviewable in the patch series. Revisit when external contributors
arrive (roadmap Fase 26 and beyond).

## Terminology

A batch script follows the existing catalog of its language; for Spanish the
agreed terms are compilar/compilación (build), Ajustes (Settings),
Almacenamiento (Storage), Inicio (Home), Acerca de (About), Carpeta de datos
(Data folder), equipo (computer), instancia, pack, mod; product names
(MineForge Launcher, Horizon, Modrinth, horizon-mixins) stay as they are.
