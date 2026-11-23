//! The fork's own title panorama (MFRE Fase 12): six faces and the overlay, drawn in code by
//! `tools/mfre_panorama.py` (a forge night: gradient, ember horizon, sparks, the M mark as a
//! faint watermark on face 0) and embedded here. Preparation writes them over the classic
//! background that comes from the player's Mojang assets, so what the fork shows is its own
//! art (TRAYECTORIA's brand notice: zero Mojang assets in the fork's branding).
use std::fs;
use std::path::Path;

use crate::Log;

const FACES: [&[u8]; 6] = [
    include_bytes!("mfre_panorama/panorama_0.png"),
    include_bytes!("mfre_panorama/panorama_1.png"),
    include_bytes!("mfre_panorama/panorama_2.png"),
    include_bytes!("mfre_panorama/panorama_3.png"),
    include_bytes!("mfre_panorama/panorama_4.png"),
    include_bytes!("mfre_panorama/panorama_5.png"),
];
const OVERLAY: &[u8] = include_bytes!("mfre_panorama/panorama_overlay.png");

/// Where the title background lives in prepared resources.
fn background_dir(target: &Path) -> std::path::PathBuf {
    target.join("assets/minecraft/textures/gui/title/background")
}

/// Writes the fork's faces over the classic background in the resources at `target`, when
/// there is one (a prepared tree, or one the `panorama` command just repaired); anything
/// else is left alone. Returns whether it wrote.
pub fn write_panorama(target: &Path, log: Log) -> bool {
    let dir = background_dir(target);
    if !dir.is_dir() {
        return false;
    }
    for (face, bytes) in FACES.iter().enumerate() {
        if fs::write(dir.join(format!("panorama_{face}.png")), bytes).is_err() {
            return false;
        }
    }
    if fs::write(dir.join("panorama_overlay.png"), OVERLAY).is_err() {
        return false;
    }
    log("MFRE: the title panorama is the fork's own (tools/mfre_panorama.py).");
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Width and height from a PNG's IHDR chunk.
    fn size(bytes: &[u8]) -> (u32, u32) {
        let w = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
        let h = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
        (w, h)
    }

    #[test]
    fn the_faces_are_equally_sized_squares() {
        let mut sizes = FACES.iter().map(|f| size(f));
        let first = sizes.next().unwrap();
        assert_eq!(first.0, first.1);
        assert!(sizes.all(|s| s == first));
        assert_eq!(size(OVERLAY), first);
    }

    #[test]
    fn preparation_replaces_the_classic_faces_and_only_those() {
        let root = std::env::temp_dir().join(format!("mfre-panorama-{}", std::process::id()));
        let dir = background_dir(&root);
        let _ = fs::remove_dir_all(&root);
        // No background yet: nothing to replace.
        assert!(!write_panorama(&root, &|_| {}));
        fs::create_dir_all(&dir).unwrap();
        let classic = dir.join("panorama_0.png");
        fs::write(&classic, b"classic").unwrap();
        let untouched = dir.join("panorama_overlay.png");
        fs::write(&untouched, b"classic overlay").unwrap();
        assert!(write_panorama(&root, &|_| {}));
        assert_eq!(fs::read(&classic).unwrap(), FACES[0]);
        assert_eq!(fs::read(&untouched).unwrap(), OVERLAY);
        // Again: the same bytes, idempotently.
        assert!(write_panorama(&root, &|_| {}));
        assert_eq!(fs::read(&classic).unwrap(), FACES[0]);
        let _ = fs::remove_dir_all(&root);
    }
}
