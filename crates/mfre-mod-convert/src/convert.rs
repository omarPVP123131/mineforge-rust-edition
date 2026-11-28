//! `mfre-mod-convert <jar> [--out <dir>] [--id <id>] [--version <version>]`: a Java content
//! mod into a Horizon content mod's project, ready for `cargo horizon-mod build` (MineForge
//! Rust Edition, F45).
//!
//! A Java mod's blocks and items are registered by its classes; Horizon declares them in
//! `data/<ns>/horizon/block/` and `horizon/item/` instead. The conversion reads the jar's
//! metadata (`fabric.mod.json`, `neoforge.mods.toml`, `mods.toml`, `mcmod.info`, through
//! `horizon_port_pack::jar`), copies its `assets/` and `data/` with the pre-1.21 folder names
//! renamed to 26.3's (`recipes` → `recipe`, `loot_tables` → `loot_table`, `tags/blocks` →
//! `tags/block`…), writes the client item definitions 26.3 items need (`assets/<ns>/items/`),
//! and declares every block its blockstates name and every item its item models name, with the
//! vanilla template its model's parent asks for (`block/cube_all` → `minecraft:stone`,
//! `block/stairs` → `minecraft:oak_stairs`…). Templates are guesses the modder reviews: a
//! template gives the block its behaviour class, and the jar's JSON does not say which one the
//! Java code chose.
//!
//! Java code is not translated (D20): the classes, mixin configs and nested jars stay out, and
//! the report says so. A mod whose behaviour its code implements (machinery, screens, world
//! generation logic) converts only in what its files declare; port that behaviour through the
//! mod API (`docs/modding/src/how-to/native-mods.md`).

use horizon_mod_package::{Content, Manifest, icon::Icon, manifest::MANIFEST};
use horizon_port_pack::zip_intake;
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::Read as _;
use std::{fs, io, path::Path, path::PathBuf};

/// The most bytes one extracted file may hold.
const MAX_ENTRY_BYTES: u64 = 64 << 20;
/// The most bytes a conversion extracts in total (the shared jar limit).
const MAX_TOTAL_BYTES: u64 = 512 << 20;
/// The most model files followed looking for the vanilla parent that names the template.
const MAX_PARENT_DEPTH: u32 = 16;
/// The template of a block whose model gives no vanilla parent to read a shape from.
const DEFAULT_BLOCK: &str = "minecraft:stone";
/// The template of every converted item: a plain stackable one, for the modder to review.
const DEFAULT_ITEM: &str = "minecraft:diamond";

/// What a conversion did: the modder's summary, and what the tests assert.
#[derive(Debug, Default)]
pub struct Report {
    pub project: PathBuf,
    pub id: String,
    pub name: String,
    pub version: String,
    /// Content files written under `assets/` and `data/`.
    pub extracted: usize,
    /// Files whose `data/` folders were renamed to the 26.3 names.
    pub renamed: usize,
    /// Block declarations written: the namespace, the block and the template chosen.
    pub blocks: Vec<(String, String, String)>,
    /// Item declarations written: the namespace and the item.
    pub items: Vec<(String, String)>,
    /// Client item definitions written under `assets/<ns>/items/`.
    pub item_definitions: usize,
    /// The jar's icon, written as `icon.png`.
    pub icon: bool,
    /// Class files in the jar: not converted.
    pub classes: usize,
    /// Mixin configs in the jar: not converted.
    pub mixins: usize,
    /// Jars nested in the jar: their content is not converted.
    pub nested_jars: usize,
    /// What the modder has to review, one line each.
    pub warnings: Vec<String>,
}

pub fn run(args: &[String]) -> Result<(), String> {
    const USAGE: &str = "usage: mfre-mod-convert <jar> [--out <dir>] [--id <id>] \
                         [--version <version>]";
    let jar = args.iter().find(|a| !a.starts_with("--")).ok_or(USAGE)?;
    let report = convert(
        Path::new(jar),
        crate::flag(args, "--out").map(Path::new),
        crate::flag(args, "--id"),
        crate::flag(args, "--version"),
    )?;
    println!(
        "Converted {} into {} (mod id {}, {} {})",
        jar,
        report.project.display(),
        report.id,
        report.name,
        report.version
    );
    println!(
        "  {} content files extracted ({} renamed to the 26.3 folder names)",
        report.extracted, report.renamed
    );
    if !report.blocks.is_empty() {
        println!(
            "  {} block declarations, templates guessed from the models — review them:",
            report.blocks.len()
        );
        for (ns, name, template) in report.blocks.iter().take(40) {
            println!("    {ns}:{name} → {template}");
        }
        if report.blocks.len() > 40 {
            println!("    …and {} more", report.blocks.len() - 40);
        }
    }
    if !report.items.is_empty() {
        println!(
            "  {} item declarations, all `{DEFAULT_ITEM}` — review them",
            report.items.len()
        );
    }
    if report.item_definitions > 0 {
        println!(
            "  {} client item definitions written under assets/<ns>/items/",
            report.item_definitions
        );
    }
    if report.icon {
        println!("  icon.png from the jar's metadata");
    }
    for warning in &report.warnings {
        println!("  warning: {warning}");
    }
    println!(
        "Next: cd {} && cargo horizon-mod build (it checks the content and packs {}.hmod)",
        report.project.display(),
        report.id
    );
    Ok(())
}

/// Converts `jar` into a content mod's project in `out` (default: a folder named after the id).
pub fn convert(
    jar: &Path,
    out: Option<&Path>,
    id: Option<&str>,
    version: Option<&str>,
) -> Result<Report, String> {
    let info = horizon_port_pack::jar::read(jar)?;
    let extra = Extra::read(jar);
    let id = sanitize_id(id.unwrap_or(&info.mod_id))?;
    let version = version
        .map(str::to_owned)
        .or_else(|| (!info.version.is_empty()).then(|| info.version.clone()))
        .ok_or_else(|| {
            format!(
                "{}: the jar's metadata records no version; name one with --version",
                jar.display()
            )
        })?;
    let name = if info.name.is_empty() {
        id.replace('_', " ")
    } else {
        info.name.clone()
    };
    let project = out.map_or_else(|| PathBuf::from(&id), Path::to_path_buf);
    if project.exists() {
        return Err(format!("{} already exists", project.display()));
    }

    let mut report = Report {
        project: project.clone(),
        id: id.clone(),
        name,
        version,
        classes: info.census.classes,
        mixins: info.census.mixin_configs,
        ..Report::default()
    };

    let scan = extract(jar, &project, extra.icon.as_deref(), &mut report)?;
    if report.extracted == 0 {
        let _ = fs::remove_dir_all(&project);
        return Err(format!(
            "{}: no assets/ or data/ content to convert{}",
            jar.display(),
            (info.census.classes > 0)
                .then(|| " (the jar is code only; Horizon needs the content as files)")
                .unwrap_or_default()
        ));
    }
    declare(&project, &scan, &mut report)?;
    let manifest = write_manifest(&project, &info, &extra, &scan, &mut report)?;

    // The same content read the build and the game do: what it refuses, the modder fixes in the
    // project; the files stay (they are the point of the conversion).
    if let Err(e) = Content::read(&project, &manifest) {
        report.warnings.push(format!(
            "the content checks refuse something, and so will the build: {e}"
        ));
    }
    if !info.depends.is_empty() {
        report.warnings.push(format!(
            "the original declares dependencies ({}): convert them too, or the content that \
             refers to them will not work",
            info.depends.join(", ")
        ));
    }
    if info.census.worldgen > 0 || info.census.structures > 0 {
        report.warnings.push(
            "world-generation files were copied as they are: written for an older game, they \
             may need the datapack upgrade (docs/datapack-upgrade.md)"
                .into(),
        );
    }
    if report.classes > 0 {
        report.warnings.push(format!(
            "{} class files not converted: the declarations replace the code's registrations, \
             but behaviour the code implemented (machinery, screens, world-generation logic) is \
             not there",
            report.classes
        ));
    }
    if report.mixins > 0 {
        report.warnings.push(
            "mixin configs not converted: what they changed in vanilla is not changed".into(),
        );
    }
    if report.nested_jars > 0 {
        report.warnings.push(format!(
            "{} nested jars not converted: their content needs a conversion of its own",
            report.nested_jars
        ));
    }
    Ok(report)
}

/// An id from a Java one: lowercase, and every character Horizon refuses becomes `_`.
fn sanitize_id(id: &str) -> Result<String, String> {
    let id: String = id
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else if c.is_ascii_uppercase() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = id.trim_matches('_');
    if trimmed.is_empty() {
        return Err(format!("{id:?} is no mod id; name one with --id"));
    }
    Ok(trimmed.to_owned())
}

/// What the jar's metadata says besides its identity: the manifest's optional lines, and the
/// icon's path in the jar.
#[derive(Debug, Default)]
struct Extra {
    description: String,
    authors: Vec<String>,
    homepage: Option<String>,
    issues: Option<String>,
    icon: Option<String>,
}

impl Extra {
    /// Reads what it can; a jar whose metadata does not parse simply has no extras (the
    /// identity itself was already read, strictly, by `jar::read`).
    fn read(jar: &Path) -> Self {
        let Ok(file) = fs::File::open(jar) else {
            return Self::default();
        };
        let Ok(mut zip) = zip_intake::open(file) else {
            return Self::default();
        };
        let text = |zip: &mut zip_intake::Archive<fs::File>, name: &str| -> Option<String> {
            let entry = zip.by_name(name).ok()?;
            let mut text = String::new();
            entry
                .take(1 << 20)
                .read_to_string(&mut text)
                .ok()
                .map(|_| text)
        };
        if let Some(fabric) =
            text(&mut zip, "fabric.mod.json").and_then(|t| serde_json::from_str::<Value>(&t).ok())
        {
            let mut extra = Self::default();
            extra.description = string(&fabric, "description");
            if let Some(authors) = fabric.get("authors").and_then(|a| a.as_array()) {
                extra.authors = authors
                    .iter()
                    .map(|a| {
                        a.as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| string(a, "name"))
                    })
                    .filter(|a| !a.is_empty())
                    .collect();
            }
            if let Some(contact) = fabric.get("contact") {
                extra.homepage = url(contact, "homepage");
                extra.issues = url(contact, "issues");
            }
            // The icon is a path, or a map of sizes to paths: any of them will do.
            match fabric.get("icon") {
                Some(Value::String(path)) => extra.icon = Some(path.clone()),
                Some(Value::Object(sizes)) => {
                    extra.icon = sizes
                        .get("1x")
                        .or_else(|| sizes.values().next())
                        .and_then(|v| v.as_str())
                        .map(str::to_owned);
                }
                _ => {}
            }
            return extra;
        }
        for name in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
            // `Value::from_str` parses one value, not a document: `from_str::<Table>`
            // is the document parser. A toml that does not parse simply has no extras.
            let Some(table) =
                text(&mut zip, name).and_then(|t| toml::from_str::<toml::Table>(&t).ok())
            else {
                continue;
            };
            let toml = toml::Value::Table(table);
            let mut extra = Self::default();
            let Some(mods) = toml.get("mods").and_then(|m| m.as_array()) else {
                continue;
            };
            if let Some(first) = mods.first() {
                let field = |key: &str| {
                    first
                        .get(key)
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .trim()
                        .to_owned()
                };
                extra.description = field("description");
                let authors = field("authors");
                if !authors.is_empty() {
                    extra.authors = authors
                        .split(',')
                        .map(|a| a.trim().to_owned())
                        .filter(|a| !a.is_empty())
                        .collect();
                }
                let homepage = field("displayURL");
                extra.homepage = (homepage.starts_with("http://")
                    || homepage.starts_with("https://"))
                .then_some(homepage);
            }
            return extra;
        }
        Self::default()
    }
}

/// A string field, trimmed of the whitespace multiline TOML descriptions wrap in.
fn string(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// An `http(s)` field, when it is one.
fn url(value: &Value, key: &str) -> Option<String> {
    let text = string(value, key);
    (text.starts_with("http://") || text.starts_with("https://")).then_some(text)
}

/// What the extraction pass saw, for the declarations that follow.
#[derive(Default)]
struct Scan {
    /// Namespaces under `assets/` and `data/`, besides `minecraft`.
    namespaces: BTreeSet<String>,
    /// `assets/<ns>/blockstates/<name>.json` saw the block `<ns>:<name>`.
    blocks: BTreeSet<(String, String)>,
    /// `assets/<ns>/models/item/<name>.json` saw the item `<ns>:<name>`.
    item_models: BTreeSet<(String, String)>,
    /// `assets/<ns>/items/<name>.json` is already there.
    item_definitions: BTreeSet<(String, String)>,
    /// `data/<ns>/horizon/block/<name>.json` is already there.
    declared_blocks: BTreeSet<(String, String)>,
    /// `data/<ns>/horizon/item/<name>.json` is already there.
    declared_items: BTreeSet<(String, String)>,
}

/// Copies the jar's `assets/` and `data/` into `project`, with the pre-1.21 `data/` folder
/// names renamed, counting what is not content for the report.
fn extract(
    jar: &Path,
    project: &Path,
    icon: Option<&str>,
    report: &mut Report,
) -> Result<Scan, String> {
    let file = fs::File::open(jar).map_err(|e| format!("{}: {e}", jar.display()))?;
    let mut zip = zip_intake::open(file).map_err(|e| format!("{}: {e}", jar.display()))?;
    let mut scan = Scan::default();
    let mut total: u64 = 0;
    let mut icon_bytes: Option<Vec<u8>> = None;
    for i in 0..zip.len() {
        let entry = match zip.by_index(i) {
            Ok(entry) => entry,
            // An entry that does not decompress is the jar's problem, not the conversion's:
            // it is skipped, and the report says so.
            Err(e) => {
                report.warnings.push(format!("entry {i} skipped: {e}"));
                continue;
            }
        };
        let name = match entry.name() {
            Ok(name) => name.into_owned(),
            Err(e) => {
                report.warnings.push(format!("entry {i} skipped: {e}"));
                continue;
            }
        };
        if name.ends_with('/') {
            continue;
        }
        if Some(name.as_str()) == icon {
            let mut bytes = Vec::new();
            if entry.take(MAX_ENTRY_BYTES).read_to_end(&mut bytes).is_ok() {
                icon_bytes = Some(bytes);
            }
            continue;
        }
        let content = name.starts_with("assets/") || name.starts_with("data/");
        if !content {
            if name.ends_with(".jar") {
                report.nested_jars += 1;
            }
            continue;
        }
        let segments: Vec<&str> = name.split('/').collect();
        // A path that escapes the project is the jar's problem, not the conversion's.
        if segments
            .iter()
            .any(|s| s.is_empty() || *s == "." || *s == "..")
        {
            report.warnings.push(format!("path skipped: {name}"));
            continue;
        }
        let (segments, renamed) = if segments[0] == "data" {
            rename_data(&segments)
        } else {
            (segments.iter().map(|s| s.to_string()).collect(), false)
        };
        if renamed {
            report.renamed += 1;
        }
        if segments.len() >= 3 {
            scan.namespaces.insert(segments[1].clone());
        }
        record(&mut scan, &segments);
        let to = segments
            .iter()
            .fold(project.to_path_buf(), |path, s| path.join(s));
        fs::create_dir_all(to.parent().expect("a file in the project"))
            .map_err(|e| format!("{}: {e}", to.display()))?;
        let written = {
            let mut out = fs::File::create(&to).map_err(|e| format!("{}: {e}", to.display()))?;
            io::copy(&mut entry.take(MAX_ENTRY_BYTES + 1), &mut out)
                .map_err(|e| format!("{name}: {e}"))?
        };
        if written > MAX_ENTRY_BYTES {
            let _ = fs::remove_file(&to);
            report.warnings.push(format!(
                "{name} skipped: over {} MiB",
                MAX_ENTRY_BYTES >> 20
            ));
            continue;
        }
        total += written;
        if total > MAX_TOTAL_BYTES {
            return Err(format!(
                "{}: over {} MiB of content; the jar is too big to convert",
                jar.display(),
                MAX_TOTAL_BYTES >> 20
            ));
        }
        report.extracted += 1;
    }
    if let Some(bytes) = icon_bytes {
        match Icon::parse(bytes) {
            Ok(icon) => {
                fs::write(project.join("icon.png"), icon.bytes())
                    .map_err(|e| format!("icon.png: {e}"))?;
                report.icon = true;
            }
            Err(e) => report.warnings.push(format!("icon skipped: {e}")),
        }
    }
    Ok(scan)
}

/// What one extracted content file tells the scan.
fn record(scan: &mut Scan, segments: &[String]) {
    let join = |a: &str, b: &str| (a.to_owned(), b.to_owned());
    match segments {
        [root, ns, kind, file] if root == "assets" && kind == "blockstates" => {
            if let Some(name) = file.strip_suffix(".json") {
                scan.blocks.insert(join(ns, name));
            }
        }
        [root, ns, kind, file] if root == "assets" && kind == "items" => {
            if let Some(name) = file.strip_suffix(".json") {
                scan.item_definitions.insert(join(ns, name));
            }
        }
        [root, ns, models, kind, file]
            if root == "assets" && models == "models" && kind == "item" =>
        {
            if let Some(name) = file.strip_suffix(".json") {
                scan.item_models.insert(join(ns, name));
            }
        }
        [root, ns, horizon, kind, file] if root == "data" && horizon == "horizon" => {
            if let Some(name) = file.strip_suffix(".json") {
                match kind.as_str() {
                    "block" => {
                        scan.declared_blocks.insert(join(ns, name));
                    }
                    "item" => {
                        scan.declared_items.insert(join(ns, name));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

/// The 26.3 names of a `data/` path's folders: the singulars 1.21 renamed, and the tag folders
/// with them. The rest of the path is returned as it is.
fn rename_data(segments: &[&str]) -> (Vec<String>, bool) {
    let rename = |segment: &str| -> &'static str {
        match segment {
            "recipes" => "recipe",
            "loot_tables" => "loot_table",
            "advancements" => "advancement",
            "predicates" => "predicate",
            "item_modifiers" => "item_modifier",
            "structures" => "structure",
            "functions" => "function",
            _ => "",
        }
    };
    let rename_tag = |segment: &str| -> &'static str {
        match segment {
            "blocks" => "block",
            "items" => "item",
            "fluids" => "fluid",
            "entity_types" => "entity_type",
            "game_events" => "game_event",
            "functions" => "function",
            _ => "",
        }
    };
    let mut renamed = false;
    let out: Vec<String> = segments
        .iter()
        .enumerate()
        .map(|(i, segment)| {
            // data/<ns>/<kind>/…, and data/<ns>/tags/<kind>/…
            let new = match i {
                2 => rename(segment),
                3 if segments.get(2) == Some(&"tags") => rename_tag(segment),
                _ => "",
            };
            if new.is_empty() {
                (*segment).to_owned()
            } else {
                renamed = true;
                new.to_owned()
            }
        })
        .collect();
    (out, renamed)
}

/// Writes the declarations the converted content needs: one per block the blockstates name and
/// item the item models name that the jar did not declare itself, and the client item
/// definitions 26.3 items need.
fn declare(project: &Path, scan: &Scan, report: &mut Report) -> Result<(), String> {
    for (ns, name) in &scan.blocks {
        if ns == "minecraft" {
            continue;
        }
        let has_item = if scan.declared_blocks.contains(&(ns.clone(), name.clone())) {
            // The declaration is the jar's own: it says whether the block has an item (and it
            // has one unless it says it does not).
            let path = project
                .join("data")
                .join(ns)
                .join("horizon")
                .join("block")
                .join(format!("{name}.json"));
            fs::read_to_string(&path)
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                .and_then(|json| json.get("item").and_then(|item| item.as_bool()))
                .unwrap_or(true)
        } else {
            let state = project
                .join("assets")
                .join(ns)
                .join("blockstates")
                .join(format!("{name}.json"));
            let model = fs::read_to_string(&state)
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                .and_then(|state| first_model(&state));
            let template = model.as_deref().map_or_else(
                || DEFAULT_BLOCK.to_owned(),
                |model| template_for(project, ns, model),
            );
            let declaration = serde_json::json!({ "template": template, "item": true });
            write_json(
                &project
                    .join("data")
                    .join(ns)
                    .join("horizon")
                    .join("block")
                    .join(format!("{name}.json")),
                &declaration,
            )?;
            report.blocks.push((ns.clone(), name.clone(), template));
            true
        };
        // The client item definition 26.3 needs: the jar's item model when it has one, else the
        // blockstate's own model.
        if has_item && !scan.item_definitions.contains(&(ns.clone(), name.clone())) {
            let model = if scan.item_models.contains(&(ns.clone(), name.clone())) {
                Some(format!("{ns}:item/{name}"))
            } else {
                fs::read_to_string(
                    project
                        .join("assets")
                        .join(ns)
                        .join("blockstates")
                        .join(format!("{name}.json")),
                )
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                .and_then(|state| first_model(&state))
                .map(|model| qualify(&model, ns))
            };
            if let Some(model) = model {
                write_item_definition(project, ns, name, &model)?;
                report.item_definitions += 1;
            }
        }
    }
    for (ns, name) in &scan.item_models {
        if ns == "minecraft" || scan.blocks.contains(&(ns.clone(), name.clone())) {
            continue;
        }
        if !scan.declared_items.contains(&(ns.clone(), name.clone())) {
            let declaration = serde_json::json!({ "template": DEFAULT_ITEM });
            write_json(
                &project
                    .join("data")
                    .join(ns)
                    .join("horizon")
                    .join("item")
                    .join(format!("{name}.json")),
                &declaration,
            )?;
            report.items.push((ns.clone(), name.clone()));
        }
        if !scan.item_definitions.contains(&(ns.clone(), name.clone())) {
            write_item_definition(project, ns, name, &format!("{ns}:item/{name}"))?;
            report.item_definitions += 1;
        }
    }
    Ok(())
}

/// An `assets/<ns>/items/<name>.json` drawing `model`.
fn write_item_definition(project: &Path, ns: &str, name: &str, model: &str) -> Result<(), String> {
    write_json(
        &project
            .join("assets")
            .join(ns)
            .join("items")
            .join(format!("{name}.json")),
        &serde_json::json!({
            "model": { "type": "minecraft:model", "model": model }
        }),
    )
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    fs::create_dir_all(path.parent().expect("a file in the project"))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    fs::write(path, serde_json::to_vec_pretty(value).expect("json writes"))
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// The model a blockstate draws: its first variant's, or its first multipart case's. Which
/// variant it is does not matter for the shape family the template is chosen by.
fn first_model(state: &Value) -> Option<String> {
    fn model_of(apply: &Value) -> Option<String> {
        if let Some(model) = apply.get("model").and_then(|m| m.as_str()) {
            return Some(model.to_owned());
        }
        apply.as_array().and_then(|cases| {
            cases.iter().find_map(|case| {
                case.get("model")
                    .and_then(|m| m.as_str())
                    .map(str::to_owned)
            })
        })
    }
    if let Some(variants) = state.get("variants").and_then(|v| v.as_object()) {
        if let Some(model) = variants.values().find_map(model_of) {
            return Some(model);
        }
    }
    state
        .get("multipart")
        .and_then(|v| v.as_array())?
        .iter()
        .find_map(|case| case.get("apply").and_then(model_of))
}

/// A model id with its namespace: `ns:path`, or `path` in `default`.
fn split_model(id: &str, default: &str) -> (String, String) {
    match id.split_once(':') {
        Some((ns, path)) => (ns.to_owned(), path.to_owned()),
        None => (default.to_owned(), id.to_owned()),
    }
}

/// A model id as `ns:path`, whichever form the blockstate used.
fn qualify(id: &str, default: &str) -> String {
    let (ns, path) = split_model(id, default);
    format!("{ns}:{path}")
}

/// The template for a block that draws `model`: the vanilla parent its model chain ends in,
/// classified by shape family. A chain that ends in the mod's own model (no vanilla parent)
/// gets the default.
fn template_for(project: &Path, default_ns: &str, model: &str) -> String {
    let (mut ns, mut path) = split_model(model, default_ns);
    for _ in 0..MAX_PARENT_DEPTH {
        if ns == "minecraft" {
            return classify(&path);
        }
        let file = project
            .join("assets")
            .join(&ns)
            .join("models")
            .join(format!("{path}.json"));
        let Ok(text) = fs::read_to_string(&file) else {
            return DEFAULT_BLOCK.to_owned();
        };
        let Ok(json) = serde_json::from_str::<Value>(&text) else {
            return DEFAULT_BLOCK.to_owned();
        };
        let Some(parent) = json.get("parent").and_then(|p| p.as_str()) else {
            return DEFAULT_BLOCK.to_owned();
        };
        (ns, path) = split_model(parent, &ns);
    }
    DEFAULT_BLOCK.to_owned()
}

/// The vanilla block for a vanilla model's base name: the shape family it draws. The order
/// matters (`fence_gate` before `fence`, `trapdoor` before `door`, `wall_torch` is a torch).
fn classify(parent_path: &str) -> String {
    let base = parent_path
        .rsplit('/')
        .next()
        .unwrap_or(parent_path)
        .to_ascii_lowercase();
    let has = |part: &str| base.contains(part);
    let template = if has("stairs") {
        "minecraft:oak_stairs"
    } else if has("slab") {
        "minecraft:oak_slab"
    } else if has("fence_gate") {
        "minecraft:oak_fence_gate"
    } else if has("fence") {
        "minecraft:oak_fence"
    } else if has("trapdoor") {
        "minecraft:oak_trapdoor"
    } else if has("door") {
        "minecraft:oak_door"
    } else if has("button") {
        "minecraft:oak_button"
    } else if has("pressure_plate") {
        "minecraft:stone_pressure_plate"
    } else if has("carpet") {
        "minecraft:white_carpet"
    } else if has("pane") {
        "minecraft:glass_pane"
    } else if has("bars") {
        "minecraft:iron_bars"
    } else if has("lantern") {
        "minecraft:lantern"
    } else if has("torch") {
        "minecraft:torch"
    } else if has("leaves") {
        "minecraft:oak_leaves"
    } else if has("sapling") {
        "minecraft:oak_sapling"
    } else if has("crop") || has("stem") {
        "minecraft:wheat"
    } else if has("sign") {
        "minecraft:oak_sign"
    } else if has("wall") {
        "minecraft:cobblestone_wall"
    } else if has("ladder") {
        "minecraft:ladder"
    } else if has("candle") {
        "minecraft:candle"
    } else if has("chain") {
        "minecraft:chain"
    } else if has("vine") {
        "minecraft:vine"
    } else if has("lily_pad") {
        "minecraft:lily_pad"
    } else if has("flower_pot") {
        "minecraft:flower_pot"
    } else if has("potted") {
        "minecraft:potted_poppy"
    } else if has("mushroom") {
        "minecraft:red_mushroom"
    } else if has("coral") && has("fan") {
        "minecraft:fire_coral_fan"
    } else if has("coral") {
        "minecraft:fire_coral"
    } else if has("cactus") {
        "minecraft:cactus"
    } else if has("ice") {
        "minecraft:ice"
    } else if has("glass") {
        "minecraft:glass"
    } else if has("tall") {
        "minecraft:large_fern"
    } else if has("cross") || has("flower") || has("plant") || has("bush") {
        "minecraft:poppy"
    } else if has("log") || has("column") || has("pillar") {
        "minecraft:oak_log"
    } else {
        DEFAULT_BLOCK
    };
    template.to_owned()
}

/// Writes the `horizon-mod.toml` of the converted project, and validates it as the build would.
fn write_manifest(
    project: &Path,
    info: &horizon_port_pack::jar::JarInfo,
    extra: &Extra,
    scan: &Scan,
    report: &mut Report,
) -> Result<Manifest, String> {
    let mut text = String::new();
    text.push_str(&format!("id = {}\n", toml_string(&report.id)));
    text.push_str(&format!("name = {}\n", toml_string(&report.name)));
    text.push_str(&format!("version = {}\n", toml_string(&report.version)));
    let description = one_line(&extra.description);
    if !description.is_empty() {
        text.push_str(&format!("description = {}\n", toml_string(&description)));
    }
    if !extra.authors.is_empty() {
        let authors: Vec<String> = extra
            .authors
            .iter()
            .map(|a| toml_string(&one_line(a)))
            .collect();
        text.push_str(&format!("authors = [{}]\n", authors.join(", ")));
    }
    let license = one_line(&info.license);
    if !license.is_empty() {
        text.push_str(&format!("license = {}\n", toml_string(&license)));
    }
    if let Some(homepage) = &extra.homepage {
        text.push_str(&format!("homepage = {}\n", toml_string(homepage)));
    }
    if let Some(issues) = &extra.issues {
        text.push_str(&format!("issues = {}\n", toml_string(issues)));
    }
    let credits = format!(
        "Converted from the Java mod {} {} with `mfre-mod-convert`: its assets and \
         data, with the block and item declarations generated from their models; no Java code \
         is part of this package.",
        info.mod_id, info.version
    );
    text.push_str(&format!("credits = {}\n", toml_string(&credits)));
    text.push_str("api = \"0.2\"\nside = \"both\"\ncapabilities = [\"registries\"]\n");
    // The namespaces the content uses besides the id's, minus the ones no mod may claim and the
    // ones the manifest would refuse.
    let namespaces: Vec<String> = scan
        .namespaces
        .iter()
        .filter(|ns| {
            ns.as_str() != report.id
                && !["minecraft", "c", "horizon"].contains(&ns.as_str())
                && (1..=64).contains(&ns.len())
                && ns
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        })
        .cloned()
        .collect();
    if !namespaces.is_empty() {
        let listed: Vec<String> = namespaces.iter().map(|n| toml_string(n)).collect();
        text.push_str(&format!("namespaces = [{}]\n", listed.join(", ")));
    }
    let manifest = Manifest::parse(&text)?;
    fs::write(project.join(MANIFEST), &text)
        .map_err(|e| format!("{}: {e}", project.join(MANIFEST).display()))?;
    Ok(manifest)
}

/// A TOML basic string of any text a jar's metadata carried.
fn toml_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Metadata text as one line: multiline descriptions keep their words, not their breaks.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    fn jar(files: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write as _;
        let mut zip = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        for (name, bytes) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    fn write_jar(dir: &Path, name: &str, files: &[(&str, &[u8])]) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, jar(files)).unwrap();
        path
    }

    /// A 1×1 transparent PNG, the smallest icon the package rules accept.
    fn png_1x1() -> Vec<u8> {
        base64::engine::general_purpose::STANDARD
            .decode(
                "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==",
            )
            .unwrap()
    }

    fn json(path: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn fabric_content_jar_converts_to_a_project() {
        let tmp = tempfile::tempdir().unwrap();
        let icon = png_1x1();
        let jar = write_jar(
            tmp.path(),
            "test-mod-1.2.3.jar",
            &[
                ("fabric.mod.json", br#"{"schemaVersion":1,"id":"test-mod","version":"1.2.3","name":"Test Mod","description":"A test mod.\nWith two lines.","authors":["Ana","Beto"],"license":"MIT","contact":{"homepage":"https://example.com","issues":"https://example.com/issues"},"icon":"assets/testmod/icon.png"}"#),
                ("assets/testmod/blockstates/ruby_block.json", br#"{"variants":{"":{"model":"testmod:block/ruby_block"}}}"#),
                ("assets/testmod/models/block/ruby_block.json", br#"{"parent":"minecraft:block/cube_all","textures":{"all":"testmod:block/ruby"}}"#),
                ("assets/testmod/models/item/ruby.json", br#"{"parent":"minecraft:item/generated","textures":{"layer0":"testmod:item/ruby"}}"#),
                ("assets/testmod/textures/block/ruby.png", b"not really a png"),
                ("assets/testmod/icon.png", &icon),
                ("data/testmod/recipes/ruby_block.json", br#"{"type":"minecraft:crafting_shaped"}"#),
                ("data/testmod/loot_tables/blocks/ruby_block.json", br#"{"type":"minecraft:block"}"#),
                ("data/testmod/tags/blocks/gems.json", br#"{"values":["testmod:ruby_block"]}"#),
                ("com/example/TestMod.class", b"\xca\xfe\xba\xbe"),
                ("META-INF/jar-in-jar/dep.jar", b"PK\x03\x04"),
            ],
        );
        let out = tmp.path().join("out");
        let report = convert(&jar, Some(&out), None, None).unwrap();
        assert_eq!(report.id, "test_mod");
        assert_eq!(report.version, "1.2.3");
        assert_eq!(report.name, "Test Mod");
        assert!(report.icon);
        assert_eq!(report.nested_jars, 1);
        assert!(report.classes >= 1);
        assert_eq!(report.renamed, 3);
        assert_eq!(
            report.blocks,
            [(
                "testmod".to_owned(),
                "ruby_block".to_owned(),
                "minecraft:stone".to_owned()
            )]
        );
        assert_eq!(report.items, [("testmod".to_owned(), "ruby".to_owned())]);
        assert_eq!(report.item_definitions, 2);
        // The code and the nested jars stay out; the content lands with the 26.3 names.
        assert!(!out.join("com").exists());
        assert!(out.join("data/testmod/recipe/ruby_block.json").exists());
        assert!(
            out.join("data/testmod/loot_table/blocks/ruby_block.json")
                .exists()
        );
        assert!(out.join("data/testmod/tags/block/gems.json").exists());
        assert!(out.join("icon.png").exists());
        let block = json(&out.join("data/testmod/horizon/block/ruby_block.json"));
        assert_eq!(block["template"], "minecraft:stone");
        assert_eq!(block["item"], true);
        let item = json(&out.join("data/testmod/horizon/item/ruby.json"));
        assert_eq!(item["template"], "minecraft:diamond");
        let definition = json(&out.join("assets/testmod/items/ruby_block.json"));
        assert_eq!(definition["model"]["model"], "testmod:block/ruby_block");
        let definition = json(&out.join("assets/testmod/items/ruby.json"));
        assert_eq!(definition["model"]["model"], "testmod:item/ruby");
        let manifest = Manifest::parse(&fs::read_to_string(out.join(MANIFEST)).unwrap()).unwrap();
        assert_eq!(manifest.id, "test_mod");
        assert_eq!(manifest.authors, ["Ana", "Beto"]);
        assert_eq!(manifest.license, "MIT");
        assert_eq!(manifest.description, "A test mod. With two lines.");
        assert_eq!(manifest.homepage.as_deref(), Some("https://example.com"));
        assert_eq!(
            manifest.issues.as_deref(),
            Some("https://example.com/issues")
        );
        assert_eq!(manifest.namespaces, ["testmod"]);
        assert!(manifest.credits.contains("test-mod 1.2.3"), "{manifest:?}");
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.contains("class files not converted"))
        );
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.contains("nested jars not converted"))
        );
    }

    #[test]
    fn neoforge_toml_identity_and_model_parent_chains() {
        let tmp = tempfile::tempdir().unwrap();
        let jar = write_jar(
            tmp.path(),
            "stairs-mod-2.0.0.jar",
            &[
                ("META-INF/neoforge.mods.toml", b"modLoader=\"javafml\"\nloaderVersion=\"[4,)\"\nlicense=\"Apache-2.0\"\n[[mods]]\nmodId=\"stairs_mod\"\nversion=\"2.0.0\"\ndisplayName=\"Stairs Mod\"\ndescription='''Stairs.\nMore stairs.'''\nauthors=\"Ana, Beto\"\n"),
                ("assets/stairs_mod/blockstates/brick_stairs.json", br#"{"variants":{"facing=north,half=bottom":{"model":"stairs_mod:block/brick_stairs"}}}"#),
                ("assets/stairs_mod/models/block/brick_stairs.json", br#"{"parent":"stairs_mod:block/base_stairs"}"#),
                ("assets/stairs_mod/models/block/base_stairs.json", br#"{"parent":"minecraft:block/stairs"}"#),
            ],
        );
        let out = tmp.path().join("out");
        let report = convert(&jar, Some(&out), None, None).unwrap();
        assert_eq!(report.id, "stairs_mod");
        assert_eq!(report.version, "2.0.0");
        assert_eq!(
            report.blocks,
            [(
                "stairs_mod".to_owned(),
                "brick_stairs".to_owned(),
                "minecraft:oak_stairs".to_owned()
            )]
        );
        let manifest = Manifest::parse(&fs::read_to_string(out.join(MANIFEST)).unwrap()).unwrap();
        assert_eq!(manifest.name, "Stairs Mod");
        assert_eq!(manifest.license, "Apache-2.0");
        assert_eq!(manifest.authors, ["Ana", "Beto"]);
        assert_eq!(manifest.description, "Stairs. More stairs.");
    }

    #[test]
    fn declarations_the_jar_already_ships_are_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let jar = write_jar(
            tmp.path(),
            "test-mod-1.0.0.jar",
            &[
                ("fabric.mod.json", br#"{"id":"test-mod","version":"1.0.0"}"#),
                (
                    "data/testmod/horizon/block/ruby_block.json",
                    br#"{"template":"minecraft:iron_block","item":false}"#,
                ),
                (
                    "assets/testmod/blockstates/ruby_block.json",
                    br#"{"variants":{"":{"model":"testmod:block/ruby_block"}}}"#,
                ),
                (
                    "assets/testmod/models/block/ruby_block.json",
                    br#"{"parent":"minecraft:block/cube_all"}"#,
                ),
            ],
        );
        let out = tmp.path().join("out");
        let report = convert(&jar, Some(&out), None, None).unwrap();
        assert!(report.blocks.is_empty());
        // The declaration is the jar's, and it says the block has no item: no definition.
        let block = json(&out.join("data/testmod/horizon/block/ruby_block.json"));
        assert_eq!(block["template"], "minecraft:iron_block");
        assert_eq!(report.item_definitions, 0);
        assert!(!out.join("assets/testmod/items/ruby_block.json").exists());
    }

    #[test]
    fn a_code_only_jar_has_nothing_to_convert() {
        let tmp = tempfile::tempdir().unwrap();
        let jar = write_jar(
            tmp.path(),
            "code-mod-1.0.0.jar",
            &[
                ("fabric.mod.json", br#"{"id":"code-mod","version":"1.0.0"}"#),
                ("com/example/CodeMod.class", b"\xca\xfe\xba\xbe"),
            ],
        );
        let out = tmp.path().join("out");
        let error = convert(&jar, Some(&out), None, None).unwrap_err();
        assert!(error.contains("no assets/ or data/ content"), "{error}");
        assert!(!out.exists());
    }

    #[test]
    fn a_jar_with_no_version_needs_one() {
        let tmp = tempfile::tempdir().unwrap();
        let jar = write_jar(
            tmp.path(),
            "test-mod.jar",
            &[
                ("fabric.mod.json", br#"{"id":"test-mod"}"#),
                (
                    "assets/testmod/blockstates/ruby_block.json",
                    br#"{"variants":{"":{"model":"testmod:block/ruby_block"}}}"#,
                ),
                (
                    "assets/testmod/models/block/ruby_block.json",
                    br#"{"parent":"minecraft:block/cube_all"}"#,
                ),
            ],
        );
        let out = tmp.path().join("out");
        let error = convert(&jar, Some(&out), None, None).unwrap_err();
        assert!(error.contains("--version"), "{error}");
        let report = convert(&jar, Some(&out), None, Some("1.0.0")).unwrap();
        assert_eq!(report.version, "1.0.0");
    }

    #[test]
    fn templates_follow_the_model_shape_family() {
        assert_eq!(classify("block/cube_all"), "minecraft:stone");
        assert_eq!(classify("block/stairs"), "minecraft:oak_stairs");
        assert_eq!(classify("block/slab_top"), "minecraft:oak_slab");
        assert_eq!(classify("block/fence_gate"), "minecraft:oak_fence_gate");
        assert_eq!(classify("block/fence_post"), "minecraft:oak_fence");
        assert_eq!(classify("block/trapdoor_bottom"), "minecraft:oak_trapdoor");
        assert_eq!(classify("block/door_bottom_left"), "minecraft:oak_door");
        assert_eq!(classify("block/wall_torch"), "minecraft:torch");
        assert_eq!(classify("block/wall_sign"), "minecraft:oak_sign");
        assert_eq!(classify("block/wall_post"), "minecraft:cobblestone_wall");
        assert_eq!(classify("block/crop"), "minecraft:wheat");
        assert_eq!(classify("block/cross"), "minecraft:poppy");
        assert_eq!(classify("block/cube_column"), "minecraft:oak_log");
        assert_eq!(classify("block/unknown_shape"), "minecraft:stone");
    }

    #[test]
    fn ids_lose_what_horizon_refuses() {
        assert_eq!(sanitize_id("My-Mod").unwrap(), "my_mod");
        assert_eq!(sanitize_id("t_and_t").unwrap(), "t_and_t");
        assert_eq!(sanitize_id("__weird__").unwrap(), "weird");
        assert!(sanitize_id("--").is_err());
    }
}
