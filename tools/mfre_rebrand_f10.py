#!/usr/bin/env python3
"""MFRE Fase 10: the UI rebrand, applied once and idempotently.

Renames the player-visible self-references of the product ("Horizon" ->
"MineForge", "Horizon Launcher" -> "MineForge Launcher") in chrome, window
titles, welcome, About, instances, setup and the .desktop entries — and in
every language catalog, keys and values together so translations keep
resolving. References to the UPSTREAM project and its services (its registry,
its releases, "made for Horizon" compatibility metadata, telemetry copy, the
wardrobe, CLI depth, log and crash-report prefixes, registry/AppImage/flatpak
internals) are deliberately left alone: they describe Horizon, not this fork,
or they are wire/internal names (D1: UI-only rebrand).

Idempotent: each replacement is skipped when its old text is gone.

Usage: python3 tools/mfre_rebrand_f10.py   (from the repo root)
"""
import json
import os
import re
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# (file, old, new) exact snippets in launcher/ and packaging/.
CODE = [
    # Chrome, window titles, menus.
    ("src/titlebar.rs", '"Horizon Launcher"', '"MineForge Launcher"'),
    ("src/blocks_chrome.rs", '"Horizon Launcher"', '"MineForge Launcher"'),
    ("src/main.rs", '"Quit Horizon Launcher"', '"Quit MineForge Launcher"'),
    ("src/main.rs", '"Horizon Launcher"', '"MineForge Launcher"'),
    # Setup / installer (display names only; the registry keys stay internal).
    ("src/setup_page.rs", '"Horizon Launcher', '"MineForge Launcher'),
    ("src/setup_page.rs", 'tr("Launch Horizon")', 'tr("Launch MineForge")'),
    ("src/installer.rs", 'APP_NAME: &str = "Horizon Launcher";', 'APP_NAME: &str = "MineForge Launcher";'),
    ("src/installer.rs", 'PUBLISHER: &str = "Horizon";', 'PUBLISHER: &str = "MineForge";'),
    ("src/installer.rs", '//! "Horizon Launcher" ([`folder_for`])', '//! "MineForge Launcher" ([`folder_for`])'),
    ("src/installer.rs", 'a folder called "Horizon Launcher"', 'a folder called "MineForge Launcher"'),
    # Welcome.
    ("src/welcome.rs", "the game tells Horizon you're playing", "the game tells the launcher you're playing"),
    ("src/welcome.rs", '"Make Horizon yours"', '"Make MineForge yours"'),
    ("src/welcome.rs", '"Installing Horizon… {percent}%"', '"Installing MineForge… {percent}%"'),
    # Settings: the logo row, the About tiles, the desktop-menu row.
    ("src/glass_settings.rs", '"Show the Horizon logo at startup"', '"Show the MineForge logo at startup"'),
    ("src/glass_settings.rs", 'self.horizon_mark(42.),\n                "Horizon".into(),', 'self.horizon_mark(42.),\n                "MineForge".into(),'),
    ("src/glass_settings.rs", '"Horizon and its icon are in your desktop\'s menu."', '"MineForge and its icon are in your desktop\'s menu."'),
    ("src/blocks_settings.rs", 'index_row(n + 1, "Horizon", Some(plain(&horizon))', 'index_row(n + 1, "MineForge", Some(plain(&horizon))'),
    ("src/blocks_settings.rs", '"Show the Horizon logo at startup"', '"Show the MineForge logo at startup"'),
    ("src/blocks_settings.rs", '"Horizon and its icon are in your desktop\'s menu."', '"MineForge and its icon are in your desktop\'s menu."'),
    ("src/blocks_settings.rs", '"{version} installs when you close Horizon"', '"{version} installs when you close MineForge"'),
    ("src/launcher_setup.rs", '"Launcher {version} installs when you close Horizon"', '"Launcher {version} installs when you close MineForge"'),
    # Home, palette, instances.
    ("src/home.rs", '} else {\n                "Horizon"\n            },', '} else {\n                "MineForge"\n            },'),
    ("src/home.rs", '"Horizon {}"', '"MineForge {}"'),
    ("src/glass_palette.rs", '|| "Horizon".to_owned()', '|| "MineForge".to_owned()'),
    ("src/instances.rs", '//! "Horizon", with its saves', '//! "MineForge", with its saves'),
    ("src/instances.rs", '"Horizon".into(),\n        Kind::Default,', '"MineForge".into(),\n        Kind::Default,'),
    ("src/instances.rs", 'crate::i18n::tr("Horizon")', 'crate::i18n::tr("MineForge")'),
    ("src/instances_page.rs", '"My Horizon"', '"My MineForge"'),
    ("src/instances_page.rs", '|| "Horizon".into()', '|| "MineForge".into()'),
    ("src/new_instance.rs", 'tr("My Horizon")', 'tr("My MineForge")'),
    ("src/new_instance.rs", '(NewKind::Horizon, "Horizon"),', '(NewKind::Horizon, "MineForge"),'),
    ("src/glass_instance.rs", 'format!("Horizon · {}"', 'format!("MineForge · {}"'),
    ("src/glass_instance.rs", 'None => "Horizon".into(),', 'None => "MineForge".into(),'),
    ("src/glass_instance.rs", 'tr("Horizon"),\n                    tr("Minecraft rebuilt in Rust, with Horizon mods")', 'tr("MineForge"),\n                    tr("Minecraft rebuilt in Rust, with MineForge mods")'),
    ("src/glass_instance.rs", '"A new Horizon instance"', '"A new MineForge instance"'),
    ("src/glass_instance.rs", '.map_or_else(|| "Horizon".into(), |i| i.name.clone());', '.map_or_else(|| "MineForge".into(), |i| i.name.clone());'),
    ("src/glass_sheet.rs", 'NewKind::Horizon => tr("Horizon"),', 'NewKind::Horizon => tr("MineForge"),'),
    ("src/glass_mods.rs", '|| "Horizon".to_owned()', '|| "MineForge".to_owned()'),
    ("src/mods_page.rs", 'tr("Horizon").to_owned()', 'tr("MineForge").to_owned()'),
    ("src/mods_page.rs", '|| "Horizon".to_string()', '|| "MineForge".to_string()'),
    ("src/glass_home.rs", '"Horizon: your Horizon instances"', '"MineForge: your MineForge instances"'),
    ("src/instance_mods.rs", 'tr("Horizon mods")', 'tr("MineForge mods")'),
    # Errors and data moves the player reads.
    ("src/blocks_play.rs", "Couldn't download Horizon.", "Couldn't download MineForge."),
    ("src/blocks_play.rs", "Couldn't get the list of Horizon versions.", "Couldn't get the list of MineForge versions."),
    ("src/blocks_play.rs", '"Not prepared. Horizon can download Minecraft {version} from Mojang for your account, or use your own install."', '"Not prepared. MineForge can download Minecraft {version} from Mojang for your account, or use your own install."'),
    ("src/blocks_play.rs", '"None needed: Horizon downloads Minecraft from Mojang for your account."', '"None needed: MineForge downloads Minecraft from Mojang for your account."'),
    ("src/blocks_play.rs", '/// "Horizon crashed": the report', '/// "MineForge crashed": the report'),
    ("src/blocks_play.rs", 'tr("Horizon crashed")', 'tr("MineForge crashed")'),
    ("src/widgets.rs", 'tr("Horizon couldn\'t reach the internet. Check your connection, then try again.")', 'tr("MineForge couldn\'t reach the internet. Check your connection, then try again.")'),
    ("src/move_window.rs", '"Moving Horizon\'s data"', '"Moving MineForge\'s data"'),
    ("src/location_page.rs", '"Choose an empty folder for Horizon\'s data."', '"Choose an empty folder for MineForge\'s data."'),
    ("src/data_location.rs", '"Horizon left these folders in {target} as they were: {folders}"', '"MineForge left these folders in {target} as they were: {folders}"'),
    ("src/refusals.rs", '"Horizon stopped before starting', '"MineForge stopped before starting'),
    ("src/refusals.rs", '"Horizon can\'t use {folder}', '"MineForge can\'t use {folder}'),
    ("src/refusals.rs", '"A folder Horizon uses is on a file system', '"A folder MineForge uses is on a file system'),
    ("src/export.rs", '"{name}, exported from Horizon."', '"{name}, exported from MineForge."'),
    ("src/instances_mrpack.rs", "which Horizon can't install", "which MineForge can't install"),
    ("src/glass_mod_states.rs", '"Install Horizon first."', '"Install MineForge first."'),
    ("src/native_build.rs", '"Install Horizon first."', '"Install MineForge first."'),
    ("src/glass_storage.rs", '"Not allowed: Horizon asks before it builds the first one."', '"Not allowed: MineForge asks before it builds the first one."'),
    ("src/internet.rs", '(Horizon never sees it)', '(MineForge never sees it)'),
    ("src/internet.rs", '"Horizon never turns on Funnel', '"MineForge never turns on Funnel'),
    # Shortcuts the player sees in their file manager.
    ("src/shortcut.rs", '{clean} (Horizon).{}', '{clean} (MineForge).{}'),
    ("src/shortcut.rs", '# Plays the Horizon instance {}', '# Plays the MineForge instance {}'),
    ("src/shortcut.rs", '"Play {name} in Horizon"', '"Play {name} in MineForge"'),
    # Comments and copy that name the product.
    ("src/native_status.rs", 'beside "Horizon"', 'beside "MineForge"'),
    ("src/themes_page.rs", 'go back to how Horizon ships.', 'go back to how MineForge ships.'),
    ("src/themes_page.rs", 'back to how Horizon ships', 'back to how MineForge ships'),
    # Linux desktop entries (the app-menu name); filenames stay horizon*.
    ("src/desktop.rs", 'Name=Horizon\\n', 'Name=MineForge\\n'),
    ("src/desktop.rs", 'play Horizon, Minecraft: Java Edition', 'play MineForge, Minecraft: Java Edition'),
    ("../packaging/linux/horizon-launcher.desktop", 'Name=Horizon\n', 'Name=MineForge\n'),
    ("../packaging/linux/horizon-launcher.desktop", 'play Horizon, Minecraft', 'play MineForge, Minecraft'),
    ("../packaging/linux/horizon.desktop", 'Name=Horizon\n', 'Name=MineForge\n'),
    # The game window's title (crates/, not launcher/).
    ("../crates/horizon-client/src/game/runtime/no_resources.rs", 'with_title("Horizon")', 'with_title("MineForge")'),
    ("../crates/horizon-client/src/game/runtime/loading_screen.rs", 'with_title("Horizon")', 'with_title("MineForge")'),
    ("../crates/horizon-client/src/game/runtime/split/windows.rs", 'format!("Horizon {key}")', 'format!("MineForge {key}")'),
    ("../crates/horizon-client/src/game/runtime/split/windows.rs", 'with_title("Horizon")', 'with_title("MineForge")'),
]


# Catalog renames the quoted-literal scan cannot derive on its own (the i18n
# test reported exactly these two as stale in every language): applied verbatim.
EXTRA_KEY_RENAMES = {
    "Horizon never turns on Funnel: the server isn't put on the public internet, only shared with the people you invite.":
        "MineForge never turns on Funnel: the server isn't put on the public internet, only shared with the people you invite.",
    "Tailscale is installed. Sign in with your own account in Tailscale (Horizon never sees it), then come back here.":
        "Tailscale is installed. Sign in with your own account in Tailscale (MineForge never sees it), then come back here.",
}


def transform(text):
    """The brand substitution, longest first so 'Horizon Launcher' wins."""
    return text.replace("Horizon Launcher", "MineForge Launcher").replace(
        "Horizon", "MineForge"
    )


def unescape(lit):
    """Rust string escapes as the compiler sees them, so literals match catalogs."""
    out, i = [], 0
    while i < len(lit):
        c = lit[i]
        if c == "\\" and i + 1 < len(lit):
            n = lit[i + 1]
            out.append({"n": "\n", "t": "\t", "r": "\r", "0": "\0"}.get(n, n))
            i += 2
        else:
            out.append(c)
            i += 1
    return "".join(out)


def live_strings():
    """Every quoted string literal left in the launcher and game sources."""
    out = set()
    for base in ("launcher/src", "crates/horizon-client/src", "crates/horizon-game/src"):
        for root, _dirs, files in os.walk(os.path.join(REPO, base)):
            for f in files:
                if not f.endswith(".rs"):
                    continue
                src = open(os.path.join(root, f), encoding="utf-8").read()
                out.update(
                    unescape(m.group(1)) for m in re.finditer(r'"((?:[^"\\]|\\.)*)"', src)
                )
    return out


def main():
    launcher = os.path.join(REPO, "launcher")
    applied = skipped = 0
    for rel, old, new in CODE:
        path = os.path.join(launcher, rel)
        src = open(path, encoding="utf-8").read()
        n = src.count(old)
        if n == 0:
            skipped += 1
            continue
        open(path, "w", encoding="utf-8").write(src.replace(old, new))
        applied += n
        print(f"{rel}: {n}x {old[:48]!r}")
    print(f"code: {applied} replacements, {skipped} snippets already applied")

    # A catalog key moves only when the code now asks for its transformed form.
    live = live_strings()
    lang = os.path.join(launcher, "lang")
    for name in sorted(os.listdir(lang)):
        if not name.endswith(".json"):
            continue
        path = os.path.join(lang, name)
        table = json.loads(open(path, encoding="utf-8").read())
        out = {}
        changed = 0
        for k, v in table.items():
            nk = EXTRA_KEY_RENAMES.get(k) or (transform(k) if "Horizon" in k else k)
            if nk != k and (nk in live or k in EXTRA_KEY_RENAMES):
                out[nk] = transform(v)
                changed += 1
            else:
                out[k] = v
        if name == "es.json" and "Make MineForge yours" not in out:
            out["Make MineForge yours"] = "Haz tuyo MineForge"
            changed += 1
        if changed:
            with open(path, "w", encoding="utf-8") as f:
                f.write(json.dumps(out, ensure_ascii=False, indent=2) + "\n")
            print(f"lang/{name}: {changed} keys")
    return 0


if __name__ == "__main__":
    sys.exit(main())
