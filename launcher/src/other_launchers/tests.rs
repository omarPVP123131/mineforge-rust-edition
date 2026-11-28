use super::*;
use serde_json::json;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("horizon-other-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[test]
fn prism_and_multimc_instances() {
    let root = scratch("prism");
    let fabric = root.join("instances/Fabric 1.21.1");
    write(
        &fabric.join("instance.cfg"),
        "[General]\nInstanceType=OneSix\nname=My Fabric\niconKey=default\n",
    );
    let pack = json!({"formatVersion": 1, "components": [
        {"uid": "org.lwjgl3", "version": "3.3.3", "dependencyOnly": true},
        {"uid": "net.minecraft", "version": "1.21.1", "important": true},
        {"uid": "net.fabricmc.intermediary", "version": "1.21.1"},
        {"uid": "net.fabricmc.fabric-loader", "version": "0.16.9"}
    ]});
    write(&fabric.join("mmc-pack.json"), &pack.to_string());
    write(&fabric.join("minecraft/mods/sodium.jar"), "jar");
    // MultiMC's game folder, and no instance.cfg: the folder's name.
    let old = root.join("instances/old");
    write(
        &old.join("mmc-pack.json"),
        &json!({"components": [{"uid": "net.minecraft", "version": "1.12.2"},
            {"uid": "net.minecraftforge", "version": "14.23.5.2860"}]})
        .to_string(),
    );
    fs::create_dir_all(old.join(".minecraft")).unwrap();
    // Not an instance.
    fs::create_dir_all(root.join("instances/_MMC_TEMP")).unwrap();

    let found = prism_instances(&root, "Prism Launcher");
    assert_eq!(
        found,
        [
            Found {
                launcher: "Prism Launcher",
                name: "My Fabric".into(),
                minecraft: Some("1.21.1".into()),
                loader: Some(Loader {
                    name: "fabric",
                    version: "0.16.9".into()
                }),
                game: fabric.join("minecraft"),
                shared: false,
            },
            Found {
                launcher: "Prism Launcher",
                name: "old".into(),
                minecraft: Some("1.12.2".into()),
                loader: Some(Loader {
                    name: "forge",
                    version: "14.23.5.2860".into()
                }),
                game: old.join(".minecraft"),
                shared: false,
            },
        ]
    );
    assert_eq!(in_folder(&fabric, None)[0].name, "My Fabric");
    // The launcher's folder and its instances folder find the same instances, never taken for
    // the CurseForge app's Instances (one name where case is ignored).
    let games = |found: &[Found]| -> Vec<(String, PathBuf)> {
        found
            .iter()
            .map(|f| (f.name.clone(), f.game.clone()))
            .collect()
    };
    assert_eq!(games(&in_folder(&root, None)), games(&found));
    assert_eq!(
        games(&in_folder(&root.join("instances"), None)),
        games(&found)
    );
    // The loader comes along at the same version.
    assert_eq!(
        loader(&found[0]),
        Some(crate::loaders::Loader {
            kind: crate::loaders::Kind::Fabric,
            version: "0.16.9".into()
        })
    );
    assert_eq!(
        loader(&found[1]).map(|l| (l.kind, l.version)),
        Some((crate::loaders::Kind::Forge, "14.23.5.2860".into()))
    );
    assert_eq!(in_folder(&root.join("instances"), Some("MultiMC")).len(), 2);
    let named = root.join("MultiMC/instances/old");
    fs::create_dir_all(named.parent().unwrap()).unwrap();
    fs::rename(&old, &named).unwrap();
    assert_eq!(in_folder(&named, None)[0].launcher, "MultiMC");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn official_launcher_profiles() {
    let root = scratch("official");
    let custom = root.join("custom");
    let profiles = json!({"profiles": {
        "a": {"type": "latest-release", "lastVersionId": "latest-release", "name": ""},
        "b": {"type": "custom", "name": "Fabric", "lastVersionId": "fabric-loader-0.16.9-1.21.1",
              "gameDir": custom.to_string_lossy()},
        "c": {"type": "custom", "name": "Old Forge", "lastVersionId": "1.20.1-forge-47.3.0"},
        "d": {"type": "custom", "name": "", "lastVersionId": "1.21.4"},
        "e": {"type": "custom", "name": "Broken"}
    }});
    write(&root.join("launcher_profiles.json"), &profiles.to_string());
    let profile = |id: &str, parent: &str| {
        let file = root.join(format!("versions/{id}/{id}.json"));
        write(
            &file,
            &json!({"id": id, "inheritsFrom": parent}).to_string(),
        );
    };
    profile("fabric-loader-0.16.9-1.21.1", "1.21.1");
    profile("1.20.1-forge-47.3.0", "1.20.1");

    let found = in_folder(&root, None);
    let short: Vec<_> = found
        .iter()
        .map(|f| {
            (
                f.name.as_str(),
                f.minecraft.as_deref().unwrap(),
                f.loader.as_ref().map(|l| (l.name, l.version.as_str())),
                f.shared,
            )
        })
        .collect();
    assert_eq!(
        short,
        [
            ("1.21.4", "1.21.4", None, true),
            ("Fabric", "1.21.1", Some(("fabric", "0.16.9")), false),
            ("Latest release", "latest", None, true),
            ("Old Forge", "1.20.1", Some(("forge", "47.3.0")), true),
        ]
    );
    assert_eq!(found[1].game, custom);
    assert_eq!(found[0].game, root);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn copies_an_instance_but_only_what_a_shared_folder_holds_for_one() {
    let root = scratch("copy");
    let game = root.join("game");
    for f in [
        "saves/World/level.dat",
        "mods/a.jar",
        "kubejs/startup.js",
        "options.txt",
        "logs/latest.log",
        "versions/1.21.1/1.21.1.jar",
        "launcher_profiles.json",
    ] {
        write(&game.join(f), f);
    }
    let mut found = Found {
        launcher: "Prism Launcher",
        name: "x".into(),
        minecraft: Some("1.21.1".into()),
        loader: None,
        game: game.clone(),
        shared: false,
    };
    let listed = |dir: &Path| {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    let own = root.join("own");
    copy_game(&found, &own).unwrap();
    // The launchers' account files never come along: a copy gets no one's accounts.
    assert_eq!(
        listed(&own),
        ["kubejs", "mods", "options.txt", "saves", "versions"]
    );
    assert_eq!(
        fs::read_to_string(own.join("saves/World/level.dat")).unwrap(),
        "saves/World/level.dat"
    );

    found.shared = true;
    let shared = root.join("shared");
    copy_game(&found, &shared).unwrap();
    assert_eq!(listed(&shared), ["mods", "options.txt", "saves"]);
    // The other launcher's folder is left as it was.
    assert!(game.join("logs/latest.log").is_file());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn curseforge_and_modrinth_app_instances_are_their_game_folders() {
    let root = scratch("apps");
    let curseforge = root.join("Documents/curseforge/minecraft");
    write(&curseforge.join("Instances/All the Mods/mods/a.jar"), "jar");
    write(
        &curseforge.join("Instances/All the Mods/minecraftinstance.json"),
        "{}",
    );
    write(&curseforge.join("Instances/All the Mods/.curseclient"), "");
    fs::create_dir_all(curseforge.join("Instances/empty")).unwrap();
    let modrinth = root.join("data/ModrinthApp");
    write(
        &modrinth.join("profiles/Fabulously Optimized/options.txt"),
        "",
    );

    let folders = Folders {
        home: root.clone(),
        data: root.join("data"),
        config: root.join("config"),
    };
    let found = scan(Os::MacOs, &folders);
    let short: Vec<_> = found
        .iter()
        .map(|f| (f.launcher, f.name.as_str(), f.minecraft.is_none(), f.shared))
        .collect();
    assert_eq!(
        short,
        [
            (MODRINTH_APP, "Fabulously Optimized", true, false),
            (CURSEFORGE, "All the Mods", true, false),
        ]
    );
    let pack = curseforge.join("Instances/All the Mods");
    assert_eq!(in_folder(&curseforge, None), [found[1].clone()]);
    assert_eq!(in_folder(&pack, None), [found[1].clone()]);
    // The app's Instances folder itself, the one it shows the player, finds the same.
    assert_eq!(
        in_folder(&curseforge.join("Instances"), None),
        [found[1].clone()]
    );
    // A folder of game folders with none of the app's files is not taken for it, and is no
    // instance itself: the game folders in it are, each picked on its own.
    let plain = root.join("plain");
    write(&plain.join("world/options.txt"), "");
    assert_eq!(in_folder(&plain, None), []);
    assert_eq!(in_folder(&plain.join("world"), None).len(), 1);
    assert_eq!(in_folder(&modrinth, None), [found[0].clone()]);

    // The app's own files stay behind.
    let to = root.join("copy");
    copy_game(&found[1], &to).unwrap();
    assert!(to.join("mods/a.jar").is_file());
    assert!(!to.join("minecraftinstance.json").exists() && !to.join(".curseclient").exists());

    let err = import(&found[1], "x", None).unwrap_err();
    assert_eq!(
        err,
        "the CurseForge app doesn't say which Minecraft version All the Mods plays here: name \
         it with --minecraft."
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn atlauncher_instances_are_read_from_their_instance_json() {
    // Synthetic folders holding only the fields four real instance.json files agree on.
    let root = scratch("atl");
    let forge = json!({"id": "1.20.1", "launcher": {"name": "My Pack", "pack": "Minecraft",
        "loaderVersion": {"version": "47.3.12", "rawVersion": "1.20.1-47.3.12",
            "recommended": false, "type": "Forge"}}});
    let neo = json!({"id": "1.21.1", "launcher": {"name": "Neo",
        "loaderVersion": {"version": "21.1.219", "type": "NeoForge"}}});
    let plain = json!({"id": "1.21.1", "launcher": {"name": ""}});
    let unknown = json!({"id": "1.20.1", "launcher": {"name": "Odd",
        "loaderVersion": {"version": "1", "type": "SomethingElse"}}});
    for (dir, doc) in [
        ("Pack1", &forge),
        ("Neo", &neo),
        ("Plain", &plain),
        ("Odd", &unknown),
    ] {
        write(
            &root.join("instances").join(dir).join("instance.json"),
            &doc.to_string(),
        );
    }
    write(&root.join("instances/Pack1/mods/a.jar"), "x");
    write(&root.join("instances/NotOne/readme.txt"), "x");
    write(
        &root.join("instances/NoLauncher/instance.json"),
        r#"{"id": "1.20.1"}"#,
    );
    write(
        &root.join("instances/NoId/instance.json"),
        r#"{"launcher": {"name": "x"}}"#,
    );
    let found = atlauncher_instances(&root);
    let names: Vec<_> = found.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["My Pack", "Neo", "Odd", "Plain"]);
    let pack = &found[0];
    assert_eq!(pack.launcher, "ATLauncher");
    assert_eq!(pack.minecraft.as_deref(), Some("1.20.1"));
    assert_eq!(pack.loader.as_ref().unwrap().name, "forge");
    assert_eq!(pack.loader.as_ref().unwrap().version, "47.3.12");
    assert_eq!(pack.game, root.join("instances/Pack1"));
    assert_eq!(found[1].loader.as_ref().unwrap().name, "neoforge");
    assert!(found[2].loader.is_none());
    assert!(found[3].loader.is_none());
    assert_eq!(in_folder(&root, None).len(), 4);
    assert_eq!(in_folder(&root.join("instances"), None).len(), 4);
    assert_eq!(in_folder(&root.join("instances/Pack1"), None).len(), 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn atlauncher_loader_types_match_whatever_the_case() {
    for (kind, want) in [
        ("fabric", "fabric"),
        ("Quilt", "quilt"),
        ("NEOFORGE", "neoforge"),
    ] {
        let launcher = json!({"loaderVersion": {"type": kind, "version": "1.0"}});
        assert_eq!(atlauncher_loader(&launcher).unwrap().name, want);
    }
    assert!(atlauncher_loader(&json!({"loaderVersion": {"type": "Forge"}})).is_none());
    assert!(atlauncher_loader(&json!({})).is_none());
}

#[test]
fn curseforge_instances_read_their_version_and_loader() {
    let root = scratch("cf-target");
    let instance = |name: &str, files: &[(&str, &str)]| {
        let dir = root.join("Instances").join(name);
        fs::create_dir_all(dir.join("mods")).unwrap();
        for (file, text) in files {
            write(&dir.join(file), text);
        }
    };
    instance(
        "fabric",
        &[(
            "minecraftinstance.json",
            r#"{"gameVersion": "1.21.5", "baseModLoader": {"name": "fabric-0.16.14-1.21.5"}}"#,
        )],
    );
    instance(
        "neo",
        &[(
            "minecraftinstance.json",
            r#"{"gameVersion": "1.21.1", "baseModLoader": {"name": "neoforge-21.1.133"}}"#,
        )],
    );
    instance(
        "manifest",
        &[(
            "manifest.json",
            r#"{"minecraft": {"version": "1.20.1", "modLoaders": [{"id": "forge-47.2.0", "primary": true}]}}"#,
        )],
    );
    instance("neither", &[]);
    instance("broken", &[("minecraftinstance.json", "{ not json")]);
    instance(
        "unknown",
        &[(
            "minecraftinstance.json",
            r#"{"gameVersion": "1.12.2", "baseModLoader": {"name": "liteloader-1.0"}}"#,
        )],
    );
    instance(
        "vanilla",
        &[("minecraftinstance.json", r#"{"gameVersion": "1.21.4"}"#)],
    );
    let found = curseforge_instances(&root);
    let by = |name: &str| found.iter().find(|f| f.name == name).unwrap();
    let loader = |name, version: &str| {
        Some(Loader {
            name,
            version: version.into(),
        })
    };
    assert_eq!(by("fabric").minecraft.as_deref(), Some("1.21.5"));
    assert_eq!(by("fabric").loader, loader("fabric", "0.16.14"));
    assert_eq!(by("neo").minecraft.as_deref(), Some("1.21.1"));
    assert_eq!(by("neo").loader, loader("neoforge", "21.1.133"));
    assert_eq!(by("manifest").minecraft.as_deref(), Some("1.20.1"));
    assert_eq!(by("manifest").loader, loader("forge", "47.2.0"));
    assert_eq!(by("unknown").minecraft.as_deref(), Some("1.12.2"));
    assert_eq!(by("unknown").loader, None);
    assert_eq!(by("vanilla").minecraft.as_deref(), Some("1.21.4"));
    assert_eq!(by("vanilla").loader, None);
    for name in ["neither", "broken"] {
        assert_eq!(
            (by(name).minecraft.clone(), by(name).loader.clone()),
            (None, None)
        );
    }
    // Without a recorded version, a version is still asked for; with one, it is not.
    assert!(import(by("neither"), "x", None)
        .unwrap_err()
        .contains("--minecraft"));
    // The Instances folder itself reads all of them the same way.
    let all = in_folder(&root.join("Instances"), None);
    assert_eq!(all, found);
    // A folder pointing at one instance reads it the same way.
    let one = in_folder(&root.join("Instances/neo"), None);
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].minecraft.as_deref(), Some("1.21.1"));
    assert_eq!(one[0].loader, loader("neoforge", "21.1.133"));
    // The Modrinth App's profiles are left as they were.
    let profile = root.join("profiles/p");
    write(
        &profile.join("minecraftinstance.json"),
        r#"{"gameVersion": "1.21.1"}"#,
    );
    fs::create_dir_all(profile.join("mods")).unwrap();
    let modrinth = modrinth_app_profiles(&root);
    assert_eq!(modrinth[0].minecraft, None);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn tlauncher_instances_are_its_version_folders_that_hold_games() {
    let root = scratch("tlauncher");
    write(&root.join("TlauncherProfiles.json"), "{\"accounts\": {}}");
    // A TLmods pack: a version folder with game data of its own, whose json inherits a Forge id
    // that inherits the game's version.
    let pack = root.join("versions/Mejor Pack");
    write(&pack.join("mods/a.jar"), "jar");
    write(&pack.join("saves/World/level.dat"), "level");
    write(
        &pack.join("Mejor Pack.json"),
        &json!({"id": "Mejor Pack", "inheritsFrom": "forge-1.16.5-36.2.39"}).to_string(),
    );
    write(
        &root.join("versions/forge-1.16.5-36.2.39/forge-1.16.5-36.2.39.json"),
        &json!({"id": "forge-1.16.5-36.2.39", "inheritsFrom": "1.16.5"}).to_string(),
    );
    // A Fabric pack, one json deep.
    let fabric = root.join("versions/Sky Factory");
    write(&fabric.join("options.txt"), "");
    write(
        &fabric.join("Sky Factory.json"),
        &json!({"inheritsFrom": "fabric-loader-0.16.9-1.21.1"}).to_string(),
    );
    // A plain version folder: no game data, no instance.
    write(&root.join("versions/1.21.1/1.21.1.jar"), "jar");
    // The everyday game folder itself.
    write(&root.join("options.txt"), "");
    write(&root.join("saves/Other/level.dat"), "level");

    let found = tlauncher_instances(&root);
    let short: Vec<_> = found
        .iter()
        .map(|f| {
            (
                f.name.as_str(),
                f.minecraft.as_deref(),
                f.loader.as_ref().map(|l| (l.name, l.version.as_str())),
                f.shared,
            )
        })
        .collect();
    assert_eq!(
        short,
        [
            (
                "Mejor Pack",
                Some("1.16.5"),
                Some(("forge", "36.2.39")),
                false
            ),
            (
                "Sky Factory",
                Some("1.21.1"),
                Some(("fabric", "0.16.9")),
                false
            ),
            ("TLauncher", None, None, true),
        ]
    );
    assert_eq!(found[0].game, pack);
    // The game folder itself finds them; so does a pack folder picked on its own.
    assert_eq!(in_folder(&root, None), found);
    let one = in_folder(&pack, None);
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].minecraft.as_deref(), Some("1.16.5"));
    assert_eq!(one[0].loader.as_ref().unwrap().name, "forge");
    // The scan of a computer whose .minecraft TLauncher uses finds them too.
    let folders = Folders {
        home: root.parent().unwrap().to_path_buf(),
        data: root.parent().unwrap().to_path_buf(),
        config: root.parent().unwrap().to_path_buf(),
    };
    fs::rename(&root, folders.home.join(".minecraft")).unwrap();
    let scanned = scan(Os::Linux, &folders);
    assert!(scanned.iter().any(|f| f.name == "Mejor Pack"));
    let root = folders.home.join(".minecraft");
    // Without TLauncher's file, the folder is any other game folder: one shared instance, its
    // single plain version the guess.
    fs::remove_file(root.join("TlauncherProfiles.json")).unwrap();
    let found = in_folder(&root, None);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].launcher, "the chosen folder");
    assert_eq!(found[0].minecraft.as_deref(), Some("1.21.1"));
    assert!(found[0].shared);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_plain_game_folder_imports_as_one_instance() {
    let root = scratch("plain-game");
    let game = root.join("game");
    write(&game.join("saves/World/level.dat"), "level");
    write(&game.join("mods/a.jar"), "jar");
    let found = in_folder(&game, None);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].launcher, "the chosen folder");
    assert_eq!(found[0].name, "game");
    assert_eq!(found[0].minecraft, None);
    assert!(!found[0].shared);
    assert_eq!(found[0].game, game);
    // A whole game folder: only what an instance holds is copied from it, and the single
    // version in it is the one it probably plays.
    write(&game.join("versions/1.21.1/1.21.1.jar"), "jar");
    write(&game.join("libraries/x"), "x");
    let found = in_folder(&game, None);
    assert_eq!(found[0].minecraft.as_deref(), Some("1.21.1"));
    assert!(found[0].shared);
    // Two versions: neither is a guess.
    write(&game.join("versions/1.20.1/1.20.1.jar"), "jar");
    assert_eq!(in_folder(&game, None)[0].minecraft, None);
    // A folder with no game in it is no instance.
    fs::create_dir_all(root.join("empty")).unwrap();
    assert_eq!(in_folder(&root.join("empty"), None), []);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn version_ids_name_their_loaders() {
    let cases = [
        (
            "fabric-loader-0.16.9-1.21.1",
            Some(("fabric", "0.16.9", Some("1.21.1"))),
        ),
        (
            "quilt-loader-0.27.1-1.21.1",
            Some(("quilt", "0.27.1", Some("1.21.1"))),
        ),
        ("neoforge-21.1.77", Some(("neoforge", "21.1.77", None))),
        (
            "1.20.1-forge-47.3.0",
            Some(("forge", "47.3.0", Some("1.20.1"))),
        ),
        (
            "forge-1.16.5-36.2.39",
            Some(("forge", "36.2.39", Some("1.16.5"))),
        ),
        ("1.21.1", None),
        ("Mejor Pack", None),
        ("OptiFine1.16.5", None),
    ];
    for (id, want) in cases {
        let parsed = id_loader(id);
        let got = parsed
            .as_ref()
            .map(|(l, mc)| (l.name, l.version.as_str(), mc.as_deref()));
        assert_eq!(got, want, "{id}");
    }
    assert!(looks_like_version("1.21.1") && looks_like_version("1.21"));
    assert!(looks_like_version("1.21.1-pre1"));
    assert!(!looks_like_version("25w14a"));
    assert!(!looks_like_version("forge-1.16.5-36.2.39"));
}
