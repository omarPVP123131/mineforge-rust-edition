#!/usr/bin/env python3
"""MFRE: clean the launcher's language catalogs (launcher/lang/*.json).

The catalogs are sparse: a missing key falls back to the English source (`i18n::tr_in`).
This script does two things the fork needs and upstream's `launcher_add_lang_keys.py`
does not (that one fills every table with dummy English entries):

1. Remove stale keys — catalog entries no literal in `launcher/src` or
   `crates/horizon-mod-package/src` mentions any more (the same scan
   `i18n::tests::translations_are_valid_and_coverage_is_reported` does; it reports
   them, this removes them). Unreachable strings keep no translation, Microsoft
   sign-in strings among them.

2. Add the fork's own Spanish (es) translations for the strings its patches
   introduced (SPANSISH below), so the fork's UI is translated where it matters.
   Other languages stay sparse on purpose: a missing key shows English, and a
   translation can be added here later.

    python3 tools/mfre_lang_cleanup.py --report   # what would change, nothing written
    python3 tools/mfre_lang_cleanup.py --apply    # write it

Every table is validated after the edit the way the Rust test validates it: the
{name} holes of a value match its key's, and no value is empty.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LANG_DIR = ROOT / "launcher" / "lang"
SCAN_DIRS = [ROOT / "launcher" / "src", ROOT / "crates" / "horizon-mod-package" / "src"]

# The fork's own strings (patches 0001-0006) in Spanish. Holes must match the key's.
SPANSISH = {
    "\n  • Accept Minecraft's EULA to download {version} from Mojang with no account: "
    "horizon-launcher eula accept. Every player must own the game.":
        "\n  • Acepta el EULA de Minecraft para descargar {version} de Mojang sin cuenta: "
        "horizon-launcher eula accept. Cada jugador debe poseer el juego.",
    "A name, 3 to 16 letters": "Un nombre, de 3 a 16 letras",
    "Cape": "Capa",
    "Skin": "Skin",
    "Remove cape": "Quitar capa",
    "Remove skin": "Quitar skin",
    "Skin and cape kept here": "Skin y capa guardados aquí",
    "Skin kept here": "Skin guardado aquí",
    "Cape kept here": "Capa guardada aquí",
    "{name}'s cape is kept here.": "La capa de {name} está guardada aquí.",
    "{name}'s cape was removed.": "La capa de {name} se quitó.",
    "{name}'s skin is kept here.": "El skin de {name} está guardado aquí.",
    "{name}'s skin was removed.": "El skin de {name} se quitó.",
    "Minecraft downloads come from Mojang; every player must own Minecraft: Java Edition.":
        "Las descargas de Minecraft vienen de Mojang; cada jugador debe poseer "
        "Minecraft: Java Edition.",
    "Not prepared. Play downloads Minecraft {version} from Mojang and prepares it.":
        "Sin preparar. Jugar descarga Minecraft {version} de Mojang y lo prepara.",
    "Offline name": "Nombre offline",
    "Offline profile": "Perfil offline",
    "Offline profile · single player and LAN": "Perfil offline · un jugador y LAN",
    "Sending is turned off in this build.": "El envío está desactivado en esta compilación.",
    "Type a name and play: single player and LAN need no account and no sign-in.":
        "Escribe un nombre y juega: un jugador y LAN no necesitan cuenta ni inicio de sesión.",
    "Type a name and play: single player and LAN need no account.":
        "Escribe un nombre y juega: un jugador y LAN no necesitan cuenta.",
    "You haven't played this one yet. Play offline to start.":
        "Aún no has jugado a este. Juega offline para empezar.",
}


def scan_file(text: str) -> set[str]:
    """Every string literal in one Rust source file.

    A sequential walk, not a tokenizer: comments are skipped, so a quoted example
    in one is not a literal (the Rust test reads tokens and agrees); cooked strings
    are decoded through json.loads, raw strings (r"…", r#"…"#) are taken verbatim,
    byte strings lose their b. Lifetimes are not char literals: a char ends in '
    right away, a lifetime is a bare word.
    """
    out: set[str] = set()
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c == "/" and i + 1 < n:
            if text[i + 1] == "/":
                j = text.find("\n", i)
                i = n if j < 0 else j
                continue
            if text[i + 1] == "*":
                depth, j = 1, i + 2
                while j < n and depth:
                    if text.startswith("/*", j):
                        depth, j = depth + 1, j + 2
                    elif text.startswith("*/", j):
                        depth, j = depth - 1, j + 2
                    else:
                        j += 1
                i = j
                continue
        if c == "b" and i + 1 < n and text[i + 1] == '"':
            i += 1
            continue
        if c == "r" and re.match(r'r#*"', text[i:]):
            m = re.match(r"r(#*)\"", text[i:])
            hashes = m.group(1)
            end = text.find('"' + hashes, i + m.end())
            if end < 0:
                i = n
                continue
            out.add(text[i + m.end():end])
            i = end + 1 + len(hashes)
            continue
        if c == '"':
            j, buf = i + 1, []
            while j < n:
                ch = text[j]
                if ch == "\\":
                    # Rust's line continuation: a backslash, the newline and the next
                    # line's leading whitespace all disappear (json.loads knows no
                    # such escape, so it is applied before the decode).
                    m = re.match(r"\\\s*\n\s*", text[j:])
                    if m:
                        j += m.end()
                        continue
                    buf.append(text[j:j + 2])
                    j += 2
                    continue
                if ch == '"':
                    break
                buf.append(ch)
                j += 1
            try:
                out.add(json.loads('"' + "".join(buf) + '"'))
            except ValueError:
                pass
            i = j + 1
            continue
        if c == "'":
            m = re.match(r"'(\\.|[^'\\])'", text[i:])
            if m:
                i += m.end()
                continue
            m = re.match(r"'[A-Za-z_][A-Za-z0-9_]*", text[i:])
            i += m.end() if m else 1
            continue
        i += 1
    return out


def source_literals() -> set[str]:
    literals: set[str] = set()
    for directory in SCAN_DIRS:
        for path in sorted(directory.rglob("*.rs")):
            literals |= scan_file(path.read_text(encoding="utf-8"))
    return literals


def holes(s: str) -> set[str]:
    return {m.group(1) for m in re.finditer(r"\{([^{}]*)\}", s)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--report", action="store_true", help="print what would change")
    group.add_argument("--apply", action="store_true", help="write the changes")
    args = parser.parse_args()

    literals = source_literals()
    print(f"source literals: {len(literals)}", file=sys.stderr)

    total_removed = 0
    for path in sorted(LANG_DIR.glob("*.json")):
        source = path.read_text(encoding="utf-8")
        table = json.loads(source)
        stale = [k for k in table if k not in literals]
        removed = len(stale)
        for key in stale:
            del table[key]
        added = 0
        if path.name == "es.json":
            # New keys go at the end, as upstream's own tool appends them; the order
            # of a catalog carries no meaning (json.dumps above keeps it stable).
            for key, said in SPANSISH.items():
                if key in table:
                    continue
                assert holes(key) == holes(said), f"holes differ: {key!r}"
                assert said.strip(), f"empty translation: {key!r}"
                table[key] = said
                added += 1
        for key, said in table.items():
            if holes(key) != holes(said):
                raise SystemExit(f"{path.name}: holes differ: {key!r}")
            if not said.strip():
                raise SystemExit(f"{path.name}: empty value: {key!r}")
        updated = json.dumps(table, ensure_ascii=False, indent=2) + "\n"
        if args.report:
            print(f"{path.name}: {len(table)} keys, -{removed} stale"
                  + (f", +{added} es" if added else ""))
            continue
        if updated != source:
            path.write_text(updated, encoding="utf-8")
        total_removed += removed
    if args.apply:
        print(f"done: {total_removed} stale entries removed across the catalogs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
