//! The fork's identity strings (MFRE Fase 13): what About and `--version` show.
//!
//! `RELEASE_DATE` is the fork revision's date, bumped when the base or the patch stack moves;
//! `BASE_REVISION` is the upstream Horizon commit this tree descends from — the revision the
//! sparse snapshot root (`e906a31`, "snapshot: horizon-rust/horizon @ 0949a05") records.
//! The attribution and trademark disclaimers that ship beside them (TRAYECTORIA's brand notice,
//! mitigations 1–3) live where they are shown: the About rows and the README.

/// The fork revision's date (UTC), bumped when the base or the patch stack moves.
pub const RELEASE_DATE: &str = "2026-11-23";
/// The upstream Horizon commit this tree descends from.
pub const BASE_REVISION: &str = "0949a05";
/// `MFRE <fecha> (base Horizon <rev>)`: About's version row and `horizon-launcher --version`.
/// A literal (`concat!` takes no consts); the test below keeps it in step with the two parts.
pub const VERSION_STRING: &str = "MFRE 2026-11-23 (base Horizon 0949a05)";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_string_names_the_fork_its_date_and_its_base() {
        let rest = VERSION_STRING
            .strip_prefix("MFRE ")
            .expect("the string starts with the fork's name");
        let (date, base) = rest
            .split_once(" (base Horizon ")
            .expect("the string carries its base");
        assert_eq!(base.strip_suffix(')'), Some(BASE_REVISION));
        let mut ymd = date.split('-');
        let num = |s: Option<&str>| s.and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        let (y, m, d) = (num(ymd.next()), num(ymd.next()), num(ymd.next()));
        assert!((2026..=2100).contains(&y) && (1..=12).contains(&m) && (1..=31).contains(&d));
        assert_eq!(ymd.next(), None);
    }

    #[test]
    fn the_base_revision_is_the_snapshot_s_upstream_commit() {
        // The snapshot root's message records it; keep both in step when the base moves.
        assert_eq!(BASE_REVISION, "0949a05");
        assert_eq!(BASE_REVISION.len(), 7);
    }

    /// F14: the upstream license notices ship unchanged, beside the fork's own notice.
    #[test]
    fn the_license_notices_ship() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let license = std::fs::read_to_string(root.join("LICENSE")).expect("LICENSE ships");
        assert!(license.contains("GNU LESSER GENERAL PUBLIC LICENSE"));
        for name in ["LICENSES/GPL-3.0.txt", "LICENSES/MIT.txt", "NOTICE-MFRE.md"] {
            assert!(root.join(name).is_file(), "{name} ships");
        }
    }

    /// F14 (D2): no Azure client ID is compiled into the fork's binaries; sign-in is
    /// configuration the fork never sets, and its flow is offline profiles.
    #[test]
    fn no_azure_client_id_ships() {
        assert_eq!(option_env!("HORIZON_MSA_CLIENT_ID"), None);
    }

    /// F14 (D6): a fork binary never replaces itself, so Horizon's signed self-update never
    /// runs; the engine-release signatures verify only upstream's own artifacts.
    #[test]
    fn the_fork_never_self_updates() {
        assert!(!crate::backend::self_update_allowed());
    }

    /// F14: no Minecraft asset ships in the tree; preparation copies the player's own copy and
    /// the fork's title background is its own art.
    #[test]
    fn no_mojang_assets_ship() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        assert!(!root.join("assets/minecraft").exists());
    }

    #[test]
    fn the_version_string_is_its_two_parts() {
        assert_eq!(
            VERSION_STRING,
            format!("MFRE {RELEASE_DATE} (base Horizon {BASE_REVISION})")
        );
    }
}
