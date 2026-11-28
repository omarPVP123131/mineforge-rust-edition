//! Instances other launchers keep, found without changing anything there, and copied into new
//! Minecraft: Java Edition instances. Built from the formats' public descriptions only:
//!
//! - Prism Launcher and MultiMC (the same instance format: an instance folder copied from one
//!   works in the other): `instances/<id>/` holds `instance.cfg` (INI, its `name`),
//!   `mmc-pack.json` (`components`, each a `uid` and a `version`: `net.minecraft` is the game,
//!   the others its loader) and the game folder, `minecraft/` or `.minecraft/`.
//!   <https://prismlauncher.org/wiki/getting-started/migrating-multimc/>,
//!   <https://prismlauncher.org/wiki/getting-started/data-location/>,
//!   <https://prismlauncher.org/wiki/overview/troubleshooting/>
//! - The official launcher: `launcher_profiles.json`'s `profiles`, each a `name`, a `type`
//!   (`custom`, `latest-release`, `latest-snapshot`), the version it plays (`lastVersionId`, an
//!   id of `versions/<id>/<id>.json`) and its game folder (`gameDir`, else the launcher's own).
//!   <https://minecraft.wiki/w/Launcher_profiles.json>
//! - The CurseForge app: its modding folder (`~/curseforge/minecraft` on Windows,
//!   `~/Documents/curseforge/minecraft` on macOS and Linux) keeps each modpack in
//!   `Instances/<name>/`, the game folder itself beside the app's `minecraftinstance.json` and
//!   `.curseclient`.
//!   <https://support.curseforge.com/support/solutions/articles/9000218572-minecraft-getting-started>,
//!   <https://support.curseforge.com/support/solutions/articles/9000238895-how-to-move-your-minecraft-modding-folder>,
//!   <https://help.akliz.net/docs/make-a-curseforge-launcher-modpack-and-server-pack>
//! - The Modrinth App: its folder (`ModrinthApp`, `com.modrinth.theseus` before 0.8.0) keeps each
//!   instance's game folder in `profiles/<name>/`.
//!   <https://support.modrinth.com/en/articles/8797641-modrinth-app-storage-folder-location>,
//!   <https://github.com/modrinth/code/issues/5825>
//! - ATLauncher: no public page describes the files of an instance, so this follows what real
//!   instances show. Four `instance.json` files that players committed to public repositories
//!   (`Benbentwo-Games/CLIENT-CustomModPack`, `Zencal/cobbleverse-neoforge-zen`,
//!   `Shadowdragon019/The-Break-Engineer-Modpack`, `Phremic/Gobbomon-Forge`) sit at the root of
//!   an instance folder, beside its `config/`, `saves/` and other game files, so that folder is
//!   the game folder. They agree on `id` (the Minecraft version), `launcher.name` and
//!   `launcher.loaderVersion` (a `type` of `Forge` or `NeoForge` and a `version`). Fabric and
//!   Quilt never appear in them: those two names come from the loader types the ATLauncher wiki
//!   lists for packs (<https://wiki.atlauncher.com/pack-admin/xml/loaders/>), and an instance
//!   whose type is none of the four copies without a loader. A user's guide shows the instances
//!   in a folder named `instances` (<https://github.com/Griefed/ServerPackCreator>, README).
//!   Every other field is ignored.
//! - TLauncher: no public page describes its files, so this follows what real folders show. It
//!   plays in a Minecraft game folder of its own choosing (the official launcher's by default),
//!   where `TlauncherProfiles.json` holds its accounts. Its TLmods packs are folders in that
//!   folder's `versions/` holding game data of their own (`mods/`, `saves/`, `config/`), each
//!   with a `<name>.json` whose `inheritsFrom` chain ends in the game's version and may name a
//!   loader in the ids it passes (`fabric-loader-0.16.9-1.21.1`, `forge-1.16.5-36.2.39`,
//!   `1.20.1-forge-47.3.0`). The game folder itself is one more instance, shared, as the
//!   official launcher's.
//! - Any other folder: a folder holding game data (`saves/`, `mods/`, `options.txt`…) that no
//!   mark above is in imports as one instance of its own; a whole game folder (with `versions/`,
//!   `libraries/` and `assets/`) gives only what an instance holds, as a shared one.
//!
//! Neither the CurseForge app nor the Modrinth App describes where it records an instance's
//! Minecraft version, so the player names it when copying one.
use crate::instances::{self, Instance};
use crate::loaders;
use crate::sources::{self, Folders, Os};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const OFFICIAL: &str = "the official launcher";

/// A mod loader and its version, as the other launcher names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loader {
    /// `fabric`, `quilt`, `neoforge` or `forge`.
    pub name: &'static str,
    pub version: String,
}

/// An instance found in another launcher.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub launcher: &'static str,
    pub name: String,
    /// The Minecraft version: an id (`1.21.1`), `latest` or `latest-snapshot`; none when the
    /// launcher keeps it where Horizon doesn't read.
    pub minecraft: Option<String>,
    pub loader: Option<Loader>,
    /// Its game folder: saves, mods, packs, options.
    pub game: PathBuf,
    /// The game folder is the launcher's own (the official launcher's default): only what an
    /// instance holds is copied, not the launcher's versions, libraries and assets.
    pub shared: bool,
}

/// What a game folder shared with a launcher gives a new instance.
const SHARED: [&str; 9] = [
    "saves",
    "mods",
    "config",
    "resourcepacks",
    "shaderpacks",
    "screenshots",
    "options.txt",
    "servers.dat",
    "optionsshaders.txt",
];
/// What is never copied from an instance's own game folder: logs, the CurseForge app's own
/// files and the launchers' account files (a copy gets no one's accounts).
const SKIPPED: [&str; 8] = [
    "logs",
    "crash-reports",
    "minecraftinstance.json",
    ".curseclient",
    // Far terrain's copies of what servers sent.
    "server-terrain",
    // The browser's list of what it installed (and may delete): only this launcher writes it.
    "mod-browser.json",
    "launcher_profiles.json",
    "TlauncherProfiles.json",
];
const CURSEFORGE: &str = "the CurseForge app";
const MODRINTH_APP: &str = "the Modrinth App";
const ATLAUNCHER: &str = "ATLauncher";
const TLAUNCHER: &str = "TLauncher";
/// A folder that holds a game but no launcher's mark: the instance is the folder's own.
const FOLDER: &str = "the chosen folder";

/// The name a launcher goes by on its own window, for a list: `launcher` as [`Found`] names it.
pub fn app_name(launcher: &'static str) -> &'static str {
    match launcher {
        OFFICIAL => "Minecraft Launcher",
        CURSEFORGE => "CurseForge",
        MODRINTH_APP => "Modrinth App",
        other => other,
    }
}

fn loader_of_name(name: &str) -> Option<&'static str> {
    match name.to_ascii_lowercase().as_str() {
        "fabric" => Some("fabric"),
        "quilt" => Some("quilt"),
        "neoforge" => Some("neoforge"),
        "forge" => Some("forge"),
        _ => None,
    }
}

fn loader_of_uid(uid: &str) -> Option<&'static str> {
    match uid {
        "net.fabricmc.fabric-loader" => Some("fabric"),
        "org.quiltmc.quilt-loader" => Some("quilt"),
        "net.neoforged" => Some("neoforge"),
        "net.minecraftforge" => Some("forge"),
        _ => None,
    }
}

/// `key=value` of an INI file's lines, outside any section or in `[General]`.
fn ini_value(text: &str, key: &str) -> Option<String> {
    let mut section = "";
    for line in text.lines().map(str::trim) {
        if let Some(s) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = s;
            continue;
        }
        if !matches!(section, "" | "General") {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

/// One Prism or MultiMC instance folder.
pub fn prism_instance(dir: &Path, launcher: &'static str) -> Option<Found> {
    let pack: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("mmc-pack.json")).ok()?).ok()?;
    let components = pack["components"].as_array()?;
    let version = |uid: &str| {
        components
            .iter()
            .find(|c| c["uid"] == uid)
            .and_then(|c| c["version"].as_str())
            .map(String::from)
    };
    let minecraft = version("net.minecraft")?;
    let loader = components.iter().find_map(|c| {
        Some(Loader {
            name: loader_of_uid(c["uid"].as_str()?)?,
            version: c["version"].as_str()?.to_string(),
        })
    });
    let game = ["minecraft", ".minecraft"]
        .into_iter()
        .map(|g| dir.join(g))
        .find(|g| g.is_dir())
        .unwrap_or_else(|| dir.join("minecraft"));
    let folder = dir.file_name()?.to_string_lossy().into_owned();
    let name = fs::read_to_string(dir.join("instance.cfg"))
        .ok()
        .and_then(|t| ini_value(&t, "name"))
        .filter(|n| !n.is_empty())
        .unwrap_or(folder);
    Some(Found {
        launcher,
        name,
        minecraft: Some(minecraft),
        loader,
        game,
        shared: false,
    })
}

/// Every instance in a Prism or MultiMC folder (its root, or its `instances/` itself).
pub fn prism_instances(root: &Path, launcher: &'static str) -> Vec<Found> {
    let instances = if root.join("instances").is_dir() {
        root.join("instances")
    } else {
        root.to_path_buf()
    };
    let mut found: Vec<Found> = fs::read_dir(&instances)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| prism_instance(&e.path(), launcher))
        .collect();
    found.sort_by_key(|f| f.name.to_lowercase());
    found
}

/// The game version and loader a version id of the official launcher plays: a loader's
/// profile names the game's version in `inheritsFrom`, and the loader in its id
/// (`fabric-loader-0.16.9-1.21.1`, `quilt-loader-0.27.1-1.21.1`, `neoforge-21.1.77`,
/// `1.20.1-forge-47.3.0`).
fn official_version(root: &Path, id: &str) -> (String, Option<Loader>) {
    let file = root.join("versions").join(id).join(format!("{id}.json"));
    let parent = fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["inheritsFrom"].as_str().map(String::from));
    let Some(minecraft) = parent else {
        return (id.to_string(), None);
    };
    let after = |prefix: &str| {
        id.strip_prefix(prefix)
            .map(|rest| rest.strip_suffix(&format!("-{minecraft}")).unwrap_or(rest))
            .map(String::from)
    };
    let loader = after("fabric-loader-")
        .map(|v| ("fabric", v))
        .or_else(|| after("quilt-loader-").map(|v| ("quilt", v)))
        .or_else(|| after("neoforge-").map(|v| ("neoforge", v)))
        .or_else(|| {
            id.split_once("-forge-")
                .map(|(_, v)| ("forge", v.to_string()))
        })
        .map(|(name, version)| Loader { name, version });
    (minecraft, loader)
}

/// The official launcher's profiles in its folder `root`.
pub fn official_profiles(root: &Path) -> Vec<Found> {
    let Some(text) = fs::read_to_string(root.join("launcher_profiles.json")).ok() else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let mut found: Vec<Found> = json["profiles"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(_, p)| {
            let kind = p["type"].as_str().unwrap_or("custom");
            let id = p["lastVersionId"].as_str().unwrap_or_default();
            let (minecraft, loader): (String, _) = match kind {
                "latest-release" => ("latest".to_string(), None),
                "latest-snapshot" => ("latest-snapshot".to_string(), None),
                _ if id.is_empty() => return None,
                _ => official_version(root, id),
            };
            let name = p["name"].as_str().unwrap_or_default().trim();
            let name = match (name, kind) {
                ("", "latest-release") => "Latest release".to_string(),
                ("", "latest-snapshot") => "Latest snapshot".to_string(),
                ("", _) => id.to_string(),
                (n, _) => n.to_string(),
            };
            let game = p["gameDir"]
                .as_str()
                .filter(|d| !d.is_empty())
                .map(PathBuf::from);
            Some(Found {
                launcher: OFFICIAL,
                name,
                minecraft: Some(minecraft),
                loader,
                shared: game.as_ref().is_none_or(|g| g == root),
                game: game.unwrap_or_else(|| root.to_path_buf()),
            })
        })
        .collect();
    found.sort_by_key(|f| f.name.to_lowercase());
    found
}

/// `dir` holds a game's own files: what marks a folder as one an instance plays in.
fn is_game_folder(dir: &Path) -> bool {
    ["saves", "mods", "options.txt", "resourcepacks", "config"]
        .iter()
        .any(|f| dir.join(f).exists())
}

/// The game folders in `dir` (each folder in it that holds one), as `launcher`'s instances.
fn game_folders(dir: &Path, launcher: &'static str) -> Vec<Found> {
    let mut found: Vec<Found> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_game_folder(p))
        .filter_map(|game| {
            Some(Found {
                launcher,
                name: game.file_name()?.to_string_lossy().into_owned(),
                minecraft: None,
                loader: None,
                game,
                shared: false,
            })
        })
        .collect();
    found.sort_by_key(|f| f.name.to_lowercase());
    found
}

/// The CurseForge app's instances: in its modding folder's `Instances/`.
pub fn curseforge_instances(root: &Path) -> Vec<Found> {
    curseforge_instances_in(&root.join("Instances"))
}

/// The CurseForge instances in `dir`, the app's `Instances/` folder itself.
fn curseforge_instances_in(dir: &Path) -> Vec<Found> {
    game_folders(dir, CURSEFORGE)
        .into_iter()
        .map(with_curseforge_target)
        .collect()
}

/// One of the folders in `dir` holds `file`: `minecraftinstance.json` in a folder of CurseForge
/// instances, `mmc-pack.json` in a Prism or MultiMC one.
fn child_holds(dir: &Path, file: &str) -> bool {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.path().join(file).is_file())
}

/// `found` with the Minecraft version and loader its CurseForge instance records
/// (`minecraftinstance.json`, else `manifest.json`). An instance that records neither, or a
/// loader Horizon doesn't name, keeps none.
fn with_curseforge_target(mut found: Found) -> Found {
    let Some((minecraft, loader, version)) =
        horizon_port_pack::import::curseforge_instance_target(&found.game)
    else {
        return found;
    };
    found.minecraft = Some(minecraft);
    found.loader = loader_of_name(&loader)
        .filter(|_| !version.is_empty())
        .map(|name| Loader { name, version });
    found
}

/// The Modrinth App's instances: in its folder's `profiles/`.
pub fn modrinth_app_profiles(root: &Path) -> Vec<Found> {
    game_folders(&root.join("profiles"), MODRINTH_APP)
}

/// The loader of an ATLauncher instance: its `launcher.loaderVersion` holds a `type` and a
/// `version`. Forge and NeoForge are what real instances show; Fabric and Quilt are the names the
/// wiki lists. Anything else is no loader.
fn atlauncher_loader(launcher: &Value) -> Option<Loader> {
    let entry = launcher.get("loaderVersion")?;
    let kind = entry.get("type")?.as_str()?.trim();
    let version = entry.get("version")?.as_str()?.trim();
    let name = ["fabric", "quilt", "neoforge", "forge"]
        .into_iter()
        .find(|n| n.eq_ignore_ascii_case(kind))?;
    (!version.is_empty()).then(|| Loader {
        name,
        version: version.to_string(),
    })
}

/// An ATLauncher instance: the folder holding an `instance.json` that has an `id` (the Minecraft
/// version) and a `launcher` object. The folder is the game folder.
pub fn atlauncher_instance(dir: &Path) -> Option<Found> {
    let text = fs::read_to_string(dir.join("instance.json")).ok()?;
    let json: Value = serde_json::from_str(&text).ok()?;
    let minecraft = json.get("id")?.as_str()?.trim();
    let launcher = json.get("launcher").filter(|l| l.is_object())?;
    if minecraft.is_empty() {
        return None;
    }
    let name = launcher
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(String::from)
        .or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()))?;
    Some(Found {
        launcher: ATLAUNCHER,
        name,
        minecraft: Some(minecraft.to_string()),
        loader: atlauncher_loader(launcher),
        game: dir.to_path_buf(),
        shared: false,
    })
}

/// The ATLauncher instances under `root`: in its `instances` folder (any capitalisation) when it
/// has one, else directly in `root`.
pub fn atlauncher_instances(root: &Path) -> Vec<Found> {
    let children = |dir: &Path| -> Vec<PathBuf> {
        fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect()
    };
    let dir = children(root)
        .into_iter()
        .find(|p| {
            p.is_dir()
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("instances"))
        })
        .unwrap_or_else(|| root.to_path_buf());
    let mut found: Vec<Found> = children(&dir)
        .iter()
        .filter_map(|p| atlauncher_instance(p))
        .collect();
    found.sort_by_key(|f| f.name.to_lowercase());
    found
}

/// What one folder holds: an official launcher's or TLauncher's profiles, a Prism or MultiMC
/// root (or its `instances/`), the CurseForge app's modding folder (or its `Instances/`), the
/// Modrinth App's folder, a single instance folder, or any folder holding a game.
pub fn in_folder(dir: &Path, launcher: Option<&'static str>) -> Vec<Found> {
    // TLauncher plays in the official launcher's folder as often as its own: what both record
    // is offered together.
    let mut profiles = official_profiles(dir);
    profiles.extend(tlauncher_instances(dir));
    if !profiles.is_empty() {
        return profiles;
    }
    if dir.join("minecraftinstance.json").is_file() {
        let parent = dir.parent().unwrap_or(dir);
        let name = dir.file_name();
        return game_folders(parent, CURSEFORGE)
            .into_iter()
            .filter(|f| f.game.file_name() == name)
            .map(with_curseforge_target)
            .collect();
    }
    if dir.join("instance.json").is_file() {
        return atlauncher_instance(dir).into_iter().collect();
    }
    let atlauncher = atlauncher_instances(dir);
    if !atlauncher.is_empty() {
        return atlauncher;
    }
    // Where names ignore case (Windows, macOS), a Prism root's `instances` is `Instances` too.
    let instances = dir.join("Instances");
    if instances.is_dir() && !child_holds(&instances, "mmc-pack.json") {
        return curseforge_instances(dir);
    }
    if child_holds(dir, "minecraftinstance.json") {
        return curseforge_instances_in(dir);
    }
    if dir.join("profiles").is_dir() {
        return modrinth_app_profiles(dir);
    }
    let named = |names: &[&str]| {
        dir.ancestors()
            .filter_map(Path::file_name)
            .any(|n| names.iter().any(|m| n == *m))
    };
    let launcher = launcher.unwrap_or(if named(&["multimc", "MultiMC"]) {
        "MultiMC"
    } else if named(&["PrismLauncher"]) {
        "Prism Launcher"
    } else {
        "Prism Launcher or MultiMC"
    });
    match prism_instance(dir, launcher) {
        Some(one) => vec![one],
        None => {
            let prism = prism_instances(dir, launcher);
            if !prism.is_empty() {
                return prism;
            }
            // Nothing names this folder's launcher: the game in it is the instance.
            game_folder_instance(dir).into_iter().collect()
        }
    }
}

/// An id that reads as a Minecraft release: `1.21`, `1.21.1` (not a snapshot, not a loader's
/// id with a version of its own).
fn looks_like_version(id: &str) -> bool {
    let base = id.split(['-', '+']).next().unwrap_or(id);
    let mut parts = base.split('.');
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let (Some(major), Some(minor)) = (parts.next(), parts.next()) else {
        return false;
    };
    digits(major) && digits(minor) && parts.all(digits)
}

/// The loader a version id names and the Minecraft version inside it. The official launcher's
/// ids (`fabric-loader-0.16.9-1.21.1`, `neoforge-21.1.77`, `1.20.1-forge-47.3.0`) and
/// TLauncher's own (`forge-1.16.5-36.2.39`) both hold them; anything else names no loader.
fn id_loader(id: &str) -> Option<(Loader, Option<String>)> {
    let named = |name: &'static str, rest: &str| Loader {
        name,
        version: rest.to_string(),
    };
    if let Some(rest) = id
        .strip_prefix("fabric-loader-")
        .or_else(|| id.strip_prefix("quilt-loader-"))
    {
        let name = if id.starts_with("fabric") {
            "fabric"
        } else {
            "quilt"
        };
        // The loader's version, then the game's (`0.16.9-1.21.1`).
        return Some(match rest.rsplit_once('-') {
            Some((version, mc)) if looks_like_version(mc) => {
                (named(name, version), Some(mc.to_string()))
            }
            _ => (named(name, rest), None),
        });
    }
    if let Some(version) = id.strip_prefix("neoforge-") {
        return Some((named("neoforge", version), None));
    }
    if let Some(rest) = id.strip_prefix("forge-") {
        // The game's version, then the loader's (`1.16.5-36.2.39`).
        return Some(match rest.split_once('-') {
            Some((mc, version)) if looks_like_version(mc) => {
                (named("forge", version), Some(mc.to_string()))
            }
            _ => (named("forge", rest), None),
        });
    }
    if let Some((mc, version)) = id.split_once("-forge-") {
        return Some((
            named("forge", version),
            looks_like_version(mc).then(|| mc.to_string()),
        ));
    }
    None
}

/// The Minecraft version and loader a TLauncher version folder plays: what the ids in its
/// `<name>.json`'s `inheritsFrom` chain name, the chain's end the game's version when it reads
/// as one.
fn tlauncher_target(root: &Path, name: &str) -> (Option<String>, Option<Loader>) {
    let (mut minecraft, mut loader) = (None, None);
    let mut id = name.to_string();
    // Chains are a handful of ids deep; a loop in them ends here.
    for _ in 0..8 {
        match id_loader(&id) {
            Some((l, mc)) => {
                loader.get_or_insert(l);
                if let Some(mc) = mc {
                    minecraft.get_or_insert(mc);
                }
            }
            None if looks_like_version(&id) => {
                minecraft.get_or_insert(id.clone());
            }
            None => {}
        }
        let file = root.join("versions").join(&id).join(format!("{id}.json"));
        let Some(parent) = fs::read_to_string(&file)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| {
                v["inheritsFrom"]
                    .as_str()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
            })
        else {
            break;
        };
        id = parent;
    }
    (minecraft, loader)
}

/// TLauncher's instances: the TLmods packs in its game folder's `versions/` (each a version
/// folder holding game data of its own), and the game folder itself, where its default profile
/// plays (shared, as the official launcher's own folder). `TlauncherProfiles.json` says
/// TLauncher keeps the folder.
pub fn tlauncher_instances(root: &Path) -> Vec<Found> {
    if !root.join("TlauncherProfiles.json").is_file() {
        return Vec::new();
    }
    let mut found: Vec<Found> = fs::read_dir(root.join("versions"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_game_folder(p))
        .filter_map(|game| {
            let name = game.file_name()?.to_string_lossy().into_owned();
            let (minecraft, loader) = tlauncher_target(root, &name);
            Some(Found {
                launcher: TLAUNCHER,
                name,
                minecraft,
                loader,
                game,
                shared: false,
            })
        })
        .collect();
    if is_game_folder(root) {
        found.push(Found {
            launcher: TLAUNCHER,
            name: TLAUNCHER.to_string(),
            minecraft: None,
            loader: None,
            game: root.to_path_buf(),
            shared: true,
        });
    }
    found.sort_by_key(|f| f.name.to_lowercase());
    found
}

/// The single version `dir/versions/` holds, when exactly one folder in it reads as a
/// Minecraft version: the version a game folder with no launcher's mark probably plays.
fn only_version(dir: &Path) -> Option<String> {
    let mut ids = fs::read_dir(dir.join("versions"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|id| looks_like_version(id));
    let one = ids.next()?;
    ids.next().is_none().then_some(one)
}

/// A folder holding a game that no launcher's mark is in: one instance, named after the
/// folder. In a TLauncher root's `versions/`, the root records what it plays; elsewhere the
/// version comes from `versions/` when exactly one is there, else the player names it. A whole
/// game folder (`versions/`, `libraries/`, `assets/` beside the game's own files) is shared:
/// only what an instance holds is copied from it.
fn game_folder_instance(dir: &Path) -> Option<Found> {
    if !is_game_folder(dir) {
        return None;
    }
    let name = dir.file_name()?.to_string_lossy().into_owned();
    let (minecraft, loader) = dir
        .parent()
        .filter(|p| p.file_name().is_some_and(|n| n == "versions"))
        .and_then(|versions| versions.parent())
        .filter(|root| root.join("TlauncherProfiles.json").is_file())
        .map(|root| tlauncher_target(root, &name))
        .unwrap_or_else(|| (only_version(dir), None));
    let shared = ["versions", "libraries", "assets"]
        .iter()
        .any(|d| dir.join(d).is_dir());
    Some(Found {
        launcher: FOLDER,
        name,
        minecraft,
        loader,
        game: dir.to_path_buf(),
        shared,
    })
}

/// The folders [`scan`] looks in, whether they exist or not.
pub fn scanned_folders(os: Os, folders: &Folders) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let known = sources::known_folders(os, folders)
        .into_iter()
        .map(|(_, d)| d);
    for dir in known.chain(curseforge_folders(os, folders)) {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    dirs
}

/// Every instance the launchers on this computer keep in their default folders.
pub fn scan(os: Os, folders: &Folders) -> Vec<Found> {
    let mut found = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();
    for (launcher, dir) in sources::known_folders(os, folders) {
        if seen.contains(&dir) || !dir.is_dir() {
            continue;
        }
        seen.push(dir.clone());
        match launcher {
            OFFICIAL => {
                found.extend(official_profiles(&dir));
                // TLauncher plays in a Minecraft folder of its own choosing, the official
                // launcher's by default: its mark in it says whose instances these are.
                found.extend(tlauncher_instances(&dir));
            }
            "Prism Launcher" | "MultiMC" => found.extend(prism_instances(&dir, launcher)),
            "Modrinth App" => found.extend(modrinth_app_profiles(&dir)),
            ATLAUNCHER => found.extend(atlauncher_instances(&dir)),
            _ => {}
        }
    }
    found.extend(
        curseforge_folders(os, folders)
            .iter()
            .flat_map(|dir| curseforge_instances(dir)),
    );
    found
}

/// The CurseForge app's default modding folder on `os`.
pub fn curseforge_folders(os: Os, folders: &Folders) -> Vec<PathBuf> {
    match os {
        // One component at a time, so the path shows with one kind of separator.
        Os::Windows => vec![folders.home.join("curseforge").join("minecraft")],
        Os::MacOs | Os::Linux => vec![folders
            .home
            .join("Documents")
            .join("curseforge")
            .join("minecraft")],
    }
}

/// Copies `found`'s game folder into `to`: all of it from an instance's own folder but its
/// logs, and only what an instance holds from a folder shared with the launcher. Nothing in
/// the other launcher's folder changes.
pub fn copy_game(found: &Found, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    let Ok(entries) = fs::read_dir(&found.game) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let wanted = if found.shared {
            SHARED.contains(&name.as_ref())
        } else {
            !SKIPPED.contains(&name.as_ref())
        };
        if !wanted {
            continue;
        }
        if entry.file_type()?.is_dir() {
            crate::backend::copy_dir(&entry.path(), &to.join(name.as_ref()))?;
        } else {
            fs::copy(entry.path(), to.join(name.as_ref()))?;
        }
    }
    Ok(())
}

/// The loader `found` runs, at the same version, when Horizon installs it ([`loaders::Kind`]).
pub fn loader(found: &Found) -> Option<loaders::Loader> {
    let l = found.loader.as_ref()?;
    Some(loaders::Loader {
        kind: loaders::Kind::parse(l.name).ok()?,
        version: l.version.clone(),
    })
}

/// A new Java instance named `name` playing what `found` plays (or `minecraft`, the version the
/// player names), with its loader when Horizon installs it ([`loader`]) and a copy of its game
/// folder.
pub fn import(found: &Found, name: &str, minecraft: Option<&str>) -> Result<Instance, String> {
    let Some(minecraft) = minecraft.or(found.minecraft.as_deref()) else {
        return Err(format!(
            "{} doesn't say which Minecraft version {} plays here: name it with --minecraft.",
            found.launcher, found.name
        ));
    };
    let versions = crate::minecraft_versions::load()?;
    let version = versions.find(minecraft)?.id.clone();
    let instance = instances::create_java(name, &version)?;
    if let Err(e) = copy_game(found, &instance.home) {
        let _ = instances::delete(&instance.id);
        return Err(format!("Could not copy {}: {e}", found.game.display()));
    }
    match loader(found) {
        Some(l) => instances::edit(&instance.id, |m| {
            m.java.get_or_insert_with(Default::default).loader = Some(l)
        }),
        None => Ok(instance),
    }
}

#[cfg(test)]
mod tests;
