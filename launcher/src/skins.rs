//! MFRE (D4): an offline profile's skin and cape are local PNG files the launcher keeps and
//! the profile's head is drawn from: `skins/<name>.png` and `skins/<name>.cape.png` under the
//! data dir, `<name>` the profile's name in lower case (the same files `Accounts::remove`
//! deletes with the profile). Nothing here touches the network — an offline profile never asks
//! Mojang's services for a look. The game itself draws its own players with the default skins
//! for now; giving these files to the game is the next step (ROADMAP phase 7).

use std::path::{Path, PathBuf};

/// The most a skin or cape PNG may weigh: these are 64×64 pictures; a megabyte is generous.
const MAX_BYTES: u64 = 1024 * 1024;

/// The folder the local skins live in, beside `accounts.json`.
pub fn dir(data: &Path) -> PathBuf {
    data.join("skins")
}

/// Where `name`'s skin (or cape) file lives. `name` is the profile's, any case.
pub fn path(data: &Path, name: &str, cape: bool) -> PathBuf {
    let key = name.to_ascii_lowercase();
    dir(data).join(if cape {
        format!("{key}.cape.png")
    } else {
        format!("{key}.png")
    })
}

/// Which of `name`'s two files are there: (skin, cape).
pub fn has(data: &Path, name: &str) -> (bool, bool) {
    (
        path(data, name, false).is_file(),
        path(data, name, true).is_file(),
    )
}

/// Validate a picked PNG and keep it as `name`'s skin or cape. A skin is 64×64 or the legacy
/// 64×32; a cape is 64×32 or 64×64. The file is copied, so the pick can go anywhere after.
pub fn keep(data: &Path, name: &str, cape: bool, file: &Path) -> Result<PathBuf, String> {
    let shown = file.display();
    let size = std::fs::metadata(file)
        .map_err(|e| format!("{shown}: {e}"))?
        .len();
    if size > MAX_BYTES {
        return Err(format!("{shown} is over 1 MB."));
    }
    let bytes = std::fs::read(file).map_err(|e| format!("{shown}: {e}"))?;
    let (w, h) =
        image::ImageReader::with_format(std::io::Cursor::new(&bytes), image::ImageFormat::Png)
            .into_dimensions()
            .map_err(|_| format!("{shown}: a 64×64 (or 64×32) PNG, please."))?;
    if !(w == 64 && (h == 64 || h == 32)) {
        return Err(format!(
            "{shown} is {w}×{h}: a skin is 64×64 (or the legacy 64×32), a cape 64×32."
        ));
    }
    let to = path(data, name, cape);
    std::fs::create_dir_all(dir(data)).map_err(|e| format!("{}: {e}", dir(data).display()))?;
    std::fs::write(&to, &bytes).map_err(|e| format!("{}: {e}", to.display()))?;
    Ok(to)
}

/// Delete `name`'s skin or cape file; a file that isn't there is no error.
pub fn clear(data: &Path, name: &str, cape: bool) -> Result<(), String> {
    let file = path(data, name, cape);
    match std::fs::remove_file(&file) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{}: {e}", file.display())),
    }
}

/// The 8×8 face of `name`'s local skin, for the profile's head; None when it has no skin file
/// or the file can't be read (the caller falls back to the UUID's default skin).
pub fn face(data: &Path, name: &str) -> Option<Vec<[u8; 4]>> {
    crate::heads::face(&std::fs::read(path(data, name, false)).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A valid PNG of `w`×`h` with one opaque red pixel at (8, 8) — the face's first pixel.
    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([0, 0, 0, 0]));
        img.put_pixel(8, 8, image::Rgba([255, 0, 0, 255]));
        let mut bytes = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
        bytes
    }

    #[test]
    fn a_picked_skin_is_validated_copied_and_found_again() {
        let root = tempfile::tempdir().unwrap();
        let (data, picks) = (root.path().join("data"), root.path().join("picks"));
        std::fs::create_dir_all(&picks).unwrap();
        // 64×64 and the legacy 64×32 both pass; the name's case doesn't matter.
        for (i, (w, h)) in [(64, 64), (64, 32)].into_iter().enumerate() {
            let pick = picks.join(format!("skin{i}.png"));
            std::fs::write(&pick, png(w, h)).unwrap();
            let kept = keep(&data, "Ana_123", false, &pick).unwrap();
            assert_eq!(kept, path(&data, "ana_123", false));
            assert!(kept.is_file());
        }
        assert_eq!(has(&data, "ANA_123"), (true, false));
        // The face comes back: the pixel at (8, 8) is the face's first one.
        let face = face(&data, "ana_123").unwrap();
        assert_eq!(face.len(), 64);
        assert_eq!(face[0], [255, 0, 0, 255]);
        // A cape goes to its own file.
        let cape = picks.join("cape.png");
        std::fs::write(&cape, png(64, 32)).unwrap();
        keep(&data, "ana_123", true, &cape).unwrap();
        assert_eq!(has(&data, "ana_123"), (true, true));
        // Clearing one leaves the other; clearing twice is no error.
        clear(&data, "ana_123", true).unwrap();
        clear(&data, "ana_123", true).unwrap();
        assert_eq!(has(&data, "ana_123"), (true, false));
    }

    #[test]
    fn what_is_not_a_skin_png_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let (data, picks) = (root.path().join("data"), root.path().join("picks"));
        std::fs::create_dir_all(&picks).unwrap();
        // Wrong dimensions.
        let small = picks.join("small.png");
        std::fs::write(&small, png(32, 32)).unwrap();
        assert!(keep(&data, "bob", false, &small).is_err());
        // Not a PNG at all.
        let text = picks.join("skin.png");
        std::fs::write(&text, b"not a png").unwrap();
        assert!(keep(&data, "bob", false, &text).is_err());
        // Nothing was kept.
        assert_eq!(has(&data, "bob"), (false, false));
        assert!(!dir(&data).exists());
        // A file that isn't there says why.
        assert!(keep(&data, "bob", false, &picks.join("missing.png")).is_err());
    }

    #[test]
    fn paths_are_the_names_in_lower_case() {
        let data = Path::new("/tmp/mfre-skins-test");
        assert_eq!(
            path(data, "Player_One", false),
            PathBuf::from("/tmp/mfre-skins-test/skins/player_one.png")
        );
        assert_eq!(
            path(data, "Player_One", true),
            PathBuf::from("/tmp/mfre-skins-test/skins/player_one.cape.png")
        );
        // No profile has a skin where nothing was kept.
        assert_eq!(has(data, "Player_One"), (false, false));
        assert!(face(data, "Player_One").is_none());
    }
}
