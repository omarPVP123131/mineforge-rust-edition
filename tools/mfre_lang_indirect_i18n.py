#!/usr/bin/env python3
"""MFRE i18n parity (inc.F): add the 245 indirect strings to fr/de/pt-br.

The Spanish catalog already carries them (tools/mfre_lang_es_indirect.py).
The fr/de/pt-br translations live in tools/mfre_indirect_{lang}.json; this tool
validates placeholders and appends any absent key, keeping the canonical format
(json.dumps indent=2, file order preserved).
"""

import argparse
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LANG = ROOT / "launcher/lang"
DICTS = {"fr": ROOT / "tools/mfre_indirect_fr.json", "de": ROOT / "tools/mfre_indirect_de.json", "pt-br": ROOT / "tools/mfre_indirect_pt-br.json"}


def holes(s: str) -> list:
    return sorted(re.findall(r"\{(\w+)\}", s))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--apply", action="store_true")
    args = ap.parse_args()
    for lang, dpath in DICTS.items():
        table_path = LANG / f"{lang}.json"
        source = table_path.read_text(encoding="utf-8")
        table = json.loads(source)
        indirect = json.loads(dpath.read_text(encoding="utf-8"))
        added = present = 0
        for en, said in indirect.items():
            assert holes(en) == holes(said), f"{lang}: {en!r}"
            assert said.strip(), f"{lang}: {en!r}"
            if en in table:
                present += 1
            else:
                table[en] = said
                added += 1
        updated = json.dumps(table, ensure_ascii=False, indent=2) + "\n"
        print(f"{lang}.json: {len(table)} keys after merge (+{added} added, {present} already present)")
        if added and args.apply:
            table_path.write_text(updated, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
