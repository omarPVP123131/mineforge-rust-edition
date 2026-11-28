//! `mfre-mod-convert <jar> [--out <dir>] [--id <id>] [--version <version>]`: MineForge Rust
//! Edition's converter of Java content mods (F45). A Fabric, NeoForge or Forge jar whose
//! blocks, items, recipes, loot, textures and translations are files becomes a Horizon content
//! mod's project (`convert.rs`), which `cargo horizon-mod build` then checks and packs into a
//! `.hmod` for the fork's catalog. The Java code is not translated: the declarations the tool
//! writes replace what the code registered, and the report says what the code did that the
//! content cannot.

mod convert;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match convert::run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// The value beside `flag` in `args`, when the flag is there.
fn flag<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1).map(String::as_str)
}
