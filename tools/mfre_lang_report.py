#!/usr/bin/env python3
"""MFRE (Fase 17): the fork's translation flow — coverage report and batch input.

The flow a language follows (Spanish set the pattern in Fase 16):

1. `python3 tools/mfre_lang_report.py` — coverage per language, worst first.
2. `python3 tools/mfre_lang_report.py --dump pt-br` — the exact keys that
   language lacks, as a JSON list, ready for a batch script.
3. A sibling of `tools/mfre_lang_es.py` (say `tools/mfre_lang_pt_br.py`) holds
   the translations, validates the {holes} and writes the catalog sparse.
4. The gate: `HORIZON_LAUNCHER_I18N_DETAILS=1 cargo test --manifest-path
   launcher/Cargo.toml --lib i18n::tests -- --nocapture` shows no missing list
   for that language any more.

The report reads what the Rust test reports: run it (default) or parse a saved
`--nocapture` output with `--details PATH`. Catalogs stay sparse and no dummy
English is ever written into them (philosophy of patch 0006): a missing key
shows its English source at run time.
"""

from __future__ import annotations

import argparse
import ast
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def details_text(saved: str | None) -> str:
    if saved:
        return Path(saved).read_text(encoding="utf-8", errors="replace")
    with tempfile.NamedTemporaryFile("w+", encoding="utf-8") as out:
        env = {"HORIZON_LAUNCHER_I18N_DETAILS": "1"}
        cmd = [
            "cargo", "test", "--quiet", "--manifest-path", "launcher/Cargo.toml",
            "--lib", "i18n::tests::translations_are_valid", "--", "--nocapture",
        ]
        run = subprocess.run(cmd, cwd=ROOT, env={**__import__("os").environ, **env},
                             stdout=out, stderr=subprocess.DEVNULL)
        if run.returncode != 0:
            sys.exit("the i18n test failed; fix it before reporting coverage")
        out.seek(0)
        return out.read()


def parse(text: str) -> tuple[dict[str, tuple[list[str], list[str]]], str | None]:
    """Per language: (missing, stale) key lists; plus the coverage summary line."""
    tables: dict[str, tuple[list[str], list[str]]] = {}
    summary = None
    lines = text.splitlines()
    i = 0
    while i < len(lines):
        line = lines[i]
        m = re.match(r"lang/([a-z-]+)\.json missing: \[$", line)
        if m:
            lang, missing, stale = m.group(1), [], []
            mode = "missing"
            i += 1
            while i < len(lines):
                l = lines[i]
                if l.endswith("]; stale: {}"):
                    break
                if l.endswith("]; stale: {"):
                    mode = "stale"
                    i += 1
                    continue
                if l in ("]", "}"):
                    break
                s = l.strip().rstrip(",")
                if s:
                    (missing if mode == "missing" else stale).append(ast.literal_eval(s))
                i += 1
            tables[lang] = (missing, stale)
        if line.startswith("Launcher locale coverage:"):
            summary = line
        i += 1
    return tables, summary


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--details", help="a saved --nocapture output of the i18n test")
    ap.add_argument("--dump", metavar="LANG", help="write that language's missing keys as JSON")
    ap.add_argument("--out", default="lang-missing.json", help="with --dump, the output file")
    args = ap.parse_args()
    tables, summary = parse(details_text(args.details))
    if args.dump:
        missing, stale = tables.get(args.dump, ([], []))
        Path(args.out).write_text(
            json.dumps(missing, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        print(f"{args.dump}: {len(missing)} missing keys written to {args.out} "
              f"({len(stale)} stale)")
        return 0
    print(summary or "no coverage line found")
    print(f"{'lang':8} {'missing':>8} {'stale':>6}")
    for lang, (missing, stale) in sorted(tables.items(), key=lambda kv: len(kv[1][0])):
        if missing or stale:
            print(f"{lang:8} {len(missing):8} {len(stale):6}")
    # Languages with nothing missing or stale print no list at all, so they are the
    # complement of what was parsed; the summary line carries the language count.
    total = int(re.search(r"(\d+) languages", summary).group(1)) if summary else 0
    print(f"with gaps: {len(tables)} of {total} languages; "
          f"the other {total - len(tables)} are complete (es among them since Fase 16)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
