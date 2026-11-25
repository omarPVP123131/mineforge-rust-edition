#!/usr/bin/env python3
"""MFRE (D25 closure, patch 0033): find the English strings the launcher shows
but no catalog translates, because they reach `tr` through a variable (a widget
helper, a closure, a const) and the i18n test only collects direct `tr("...")`
literals (`launcher/src/i18n/tests.rs`: `asked` vs `all`).

Upstream philosophy keeps the tables sparse with an English-source fallback, so
these strings are optional to translate; the fork's promise is a complete
Spanish UI, so this tool lists what the Spanish table still leaves in English,
curated to user-facing surfaces: test modules, fixtures, MCP/agent tool prose,
CLI-only help, Windows registry names, paths and protocol literals stay out.

    python3 tools/mfre_lang_indirect.py            # candidates, one per line
    python3 tools/mfre_lang_indirect.py --lang fr  # against another catalog
    python3 tools/mfre_lang_indirect.py --with-files

The curation lives in the EXCLUDE set below; every entry names why it stays in
English. Add a line here when a candidate is proven non-user-facing, instead of
deleting it from the output by hand.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# Whole files whose literals never reach a player's screen.
SKIP_FILES = {
    "launcher/src/i18n/tests.rs",  # language names and collector fixtures
    "launcher/src/mcp.rs",  # MCP/agent tool descriptions (agent-facing prose)
    "launcher/src/game_tools.rs",  # same, for the in-game agent control
    "launcher/src/agents.rs",  # agent discovery fixtures and prose
    "launcher/src/agents_page.rs",  # agent connect help (developer surface)
    "launcher/src/cli.rs",  # CLI help text (docs/launcher-commands.md carries it)
    "launcher/src/cli_tools.rs",
    "launcher/src/cli_native_permission.rs",
    "launcher/src/cli_catalog_tests.rs",
    "launcher/src/installer.rs",  # Windows registry value names
    "launcher/src/i18n.rs",  # language endonyms, shown as their own names on purpose
    "launcher/src/rust_toolchain.rs",  # compiler/toolchain diagnostics, kept verbatim
}

# Substring marks of literals that are data, not prose.
DATA_MARKS = (
    ".rs", ".exe", ".json", ".zip", ".jar", ".toml", ".mcmeta", ".mca", ".dat",
    ".lnk", ".icns", ".plist", "::", "->", "http", "HTTP/", "Bearer", "GET /",
    "DELETE /", "POST /", "=", "{", "}", "\\", "AppData", "Application Support",
    "Users/", "Contents/", "Documents/", "Example/", "World/", "Folder Pack",
    "x86_64", "aarch64", "mingw", "MinGW", "GCC ", "Cargo.lock", "Cargo.toml",
    "XBL3.0", "X-Horizon", "URLInfoAbout", "UninstallString", "DisplayVersion",
    "EstimatedSize", "ExecutablePath", "CommandLine", "BatchMode", "ServerAlive",
    "ConnectTimeout", "ContentModList", "BackendState", "CurrentTailnet",
    "TailscaleIPs", "DisplayClaims", "ForbiddenOperationException", "AuthMethod",
    "AuthURL", "TokenType", "UserTokens", "Content-Range", "ZIP central",
)

# Curated: proven non-user-facing, or proper names the fork keeps as shipped.
EXCLUDE = {
    "A-B- C- (MineForge).lnk": "shortcut test fixture",
    "Tom & Jerry": "shortcut test fixture",
    "Zed (2).zip": "pack fixture",
    "TTTTTTTTTtPP": "wardrobe fixture",
    "TTTtSSSSSSSS": "wardrobe fixture",
    "Creative test": "mod-state fixture",
    "Survival 1": "mod-state fixture",
    "Fix the picker": "upstream commit-message fixture",
    "Fix the picker (#6389, fixes #6342)": "upstream commit-message fixture",
    "Claude Code": "agent product name",
    "Codex": "agent product name",
    "Xbox Live": "service name",
    "CurseForge": "service name",
    "Tailscale": "service name",
    "SteamOS": "system name",
    "Vulkan": "renderer name",
    "Iris": "mod name",
    "OptiFine": "mod name",
    "ATLauncher": "launcher name",
    "Fabulously Optimized": "pack name fixture",
    "Fresh Animations": "pack name fixture",
    "Terralith": "pack name fixture",
    "All the Mods": "pack name fixture",
    "Yet Another Bingo: Ultimate": "pack name fixture",
    "Yet Another Bingo- Ultimate-2.15.1.mrpack": "pack file fixture",
    "Faithful 32x.zip": "pack file fixture",
    "Faithful_v10.zip": "pack file fixture",
    "FreshAnimations_v1.11.1_26.1+.zip": "pack file fixture",
    "Textures.zip": "pack file fixture",
    "Trades (2).zip": "pack file fixture",
    "Ünïcode pack.zip": "pack file fixture",
    "Tools: \"Extra\"?-1.zip": "pack file fixture",
    "AUX .zip": "pack file fixture",
    "Small Pack": "instance fixture",
    "Family pack": "instance fixture",
    "Family copy": "instance fixture",
    "Base (1)": "world fixture",
    "Base (2)": "world fixture",
    "Vanilla 1.21.4": "instance fixture",
    "Fabric 1.21.4": "instance fixture",
    "Dan": "account fixture",
    "Bea": "account fixture",
    "Cal": "account fixture",
    "Steve": "Minecraft proper name",
    "Alex": "Minecraft proper name",
    "Été 2026": "server fixture (already French)",
    "Configurações": "language list fixture",
    "Einstellungen": "language list fixture",
    "Ustawienia": "language list fixture",
    "Настройки": "language list fixture",
    "Налаштування": "language list fixture",
    "Игры": "server fixture",
    "Игры with spaces": "server fixture",
    "Ayarlar": "language list fixture",
    "Unirse": "language list fixture",
    "Beitreten": "language list fixture",
    "Chapeau": "theme fixture",
    "Ajustes": "language list fixture",
    "A warm dawn": "theme name, kept as shipped",
    "Autumn Forest": "theme name, kept as shipped",
    "Blocky hills under square clouds": "theme name, kept as shipped",
    "Cherry Blossom": "theme name, kept as shipped",
    "Cozy ": "theme name, kept as shipped",
    "Dawn": "theme name, kept as shipped",
    "Deep Ocean": "theme name, kept as shipped",
    "Falling leaves at dusk": "theme name, kept as shipped",
    "Snow on the spruce woods": "theme name, kept as shipped",
    "Sunrise": "theme name, kept as shipped",
    "Taiga": "theme name, kept as shipped",
    "The forge ember": "MFRE theme name, kept as shipped",
    "The launcher's first sky": "theme name, kept as shipped",
    "Warm Light": "theme name, kept as shipped",
    "Apple M4 Pro (Metal)": "telemetry fixture",
    "Ask\n//! whoever pays before you buy. Refunds within 14 days for any reason.": "shop fixture with doc-comment artifact",
    "My\n    /// Horizon": "new-instance fixture with doc-comment artifact",
    "Dawn Client": "source fixture",
    "Equipped Founder Visor.": "wardrobe fixture",
    "Founder Visor": "cosmetic name, kept as shipped",
    "HORIZON15 · 15% off": "promo fixture",
    "Hypr Neon": "theme name, kept as shipped",
    "Midnight Aurora": "theme name, kept as shipped",
    "Light through deep water": "theme name, kept as shipped",
    "Imported · 2026-10-30": "instance fixture",
    "Java 1.21.4": "instance fixture",
    "M on the official SMP": "account fixture",
    "MFRE 2026-11-23 (base Horizon 0949a05)": "fork version banner, kept verbatim",
    "Minecraft 1.21.1 · neoforge 21.1.259 · 496 mods": "modpack fixture",
    "Lifting card": "specimen fixture",
    "N playing now": "specimen placeholder",
    "Building [": "progress fragment, not prose",
    "Compiling ": "progress fragment, not prose",
    "Exited with": "progress fragment, not prose",
    "Cannot lock": "error fragment completed at runtime",
    "Can't be carried over: mod id ": "error fragment completed at runtime",
    "Made for other Horizon versions: ": "label fragment completed at runtime",
    "Horizon · mode · size": "server row template, kept verbatim",
    "Minecraft Launcher": "other launcher's name",
    "Modrinth App": "other launcher's name",
    "Horizon Official": "server list proper name",
    "Horizon shader pack": "pack kind label kept as shipped",
    "Old School": "theme name, kept as shipped",
    "Petals on a pink evening": "theme name, kept as shipped",
    "Polar night and frost": "theme name, kept as shipped",
    "Nov 13\n    /// update": "version-card specimen with doc-comment artifact",
    "Nov 13 update, with your mods": "version-card specimen",
    "Nov 7 update": "version-card specimen",
    "Oct 31": "home specimen",
    "Playing for 5 min now": "home specimen",
    "Playing for…": "progress fragment, not prose",
    "You've played for 41 h 06 min! Last played today.": "home specimen",
    "Runs its 12 functions, 1 predicate. Not read yet: 40 worldgen, 3 recipes.": "datapack specimen",
    "PipeWire ALSA [horizon-client]": "audio device fixture",
    "OS string serialization": "lease fixture",
    "Segoe UI": "font name",
    "Prism Launcher": "other launcher's name",
    "Prism Launcher or MultiMC": "other launchers' names",
    "XboxLive.signin offline_access": "OAuth scope literal",
    "Session ID is": "message fragment completed at runtime",
    "Stay on": "picker fragment completed at runtime",
    "Ready · 26.3": "shell template, kept verbatim",
    "Launcher 0.2.0": "frame specimen",
    "Made for nightly-2026-11-10. Fits this game (nightly-2026-11-13).": "picker specimen",
    "Update\n//! registry mods automatically": "settings label with doc-comment artifact",
    "Name (2).mrpack": "pack file fixture",
    "Needs\n/// build": "label with doc-comment artifact",
    "Neon on pure black": "theme name, kept as shipped",
    "Northern lights": "theme name, kept as shipped",
    "Nightly\n/// channel": "label with doc-comment artifact",
    "Nightly ": "channel specimen",
    "Nightly 2026-10-28 · 0c02067": "channel specimen",
    "Nightly · 514ecf9": "channel specimen",
    "Nov 1": "version-card specimen",
    "Nov 11 update": "version-card specimen",
    "Nothing is waiting. One example of each kind, from this machine; the game fills in the GPU and screen it uses where these say example (a benchmark upload is the report itself, see docs/benchmark-report.md):": "telemetry specimen",
}


def is_test_file(path: Path) -> bool:
    return (
        path.name.endswith("_tests.rs")
        or path.name == "tests.rs"
        or any(part == "tests" for part in path.parts)
    )


def candidates(lang: str) -> dict[str, str]:
    table = json.loads((ROOT / "launcher" / "lang" / f"{lang}.json").read_text())
    out: dict[str, str] = {}
    for path in sorted((ROOT / "launcher" / "src").rglob("*.rs")):
        rel = str(path.relative_to(ROOT))
        if rel in SKIP_FILES or is_test_file(path):
            continue
        text = path.read_text(errors="ignore")
        # Comments first: the Rust collector reads tokens, so a quoted example in
        # a doc comment is not a string the UI can show (16 stale keys taught us).
        text = re.sub(r"//[^\n]*", "", text)
        text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
        # Drop #[cfg(test)] mod bodies: they hold fixtures, not UI.
        text = re.sub(r"#\[cfg\(test\)\]\s*(?:mod|pub mod)\s+\w+\s*\{.*", "", text, flags=re.S)
        for match in re.finditer(r'"((?:[^"\\]|\\.)*)"', text):
            said = match.group(1)
            if said in table or said in EXCLUDE:
                continue
            if len(said) < 3 or not said[0].isupper():
                continue
            if not re.search(r"[a-z]", said):
                continue
            if " " not in said and len(said) < 12:
                continue
            if any(mark in said for mark in DATA_MARKS):
                continue
            if said.count(" ") == 0:
                continue
            out.setdefault(said, rel)
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--lang", default="es")
    ap.add_argument("--with-files", action="store_true")
    args = ap.parse_args()
    found = candidates(args.lang)
    for said, where in sorted(found.items()):
        print(f"{said}  <-- {where}" if args.with_files else said)
    print(f"# {len(found)} candidates absent from {args.lang}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
