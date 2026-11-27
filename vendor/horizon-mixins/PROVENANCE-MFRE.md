# Provenance of this vendored snapshot (MineForge Rust Edition)

This directory is an unmodified snapshot of the upstream `horizon-mixins` tool at commit
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0` — the revision this repository pins
(`crates/horizon-mod-package/src/mixins_tool.rs`, `REV`). Upstream published it at
`https://github.com/horizon-rust/horizon-mixins`, which now answers 404. The snapshot was
recovered on 2026-11-27 from the public copy at `https://github.com/YKKULTRA/horizon-mixins`
(a standalone copy of `levzzz5154/horizon-mixins`, MIT), via
`https://codeload.github.com/YKKULTRA/horizon-mixins/tar.gz/e59da9094f0c2fdda6c627fe9eff22dec6a906f0`
(SHA-256 `5ee6da27c822fc3c1b8258618b8de68356dbee8df8f7dc0b7493b8d96620710d`). That the tree is
the pinned revision is checked at build time by the fork's carried patch applying to it exactly
(see below); the patch refuses any other text.

License: MIT, `Copyright (c) 2026 levzzz` (see `LICENSE`), compatible with this repository's
LGPL-3.0 and redistributed here unmodified with attribution.

The fork's one carried change, `tools/mixins/narrow-reference-rule.patch`, is not applied in this
tree. Builds apply it with the repository's own applier
(`cargo run -p xtask -- mixins-tool patch vendor/horizon-mixins`), exactly as the launcher does
when it builds the tool from source, so the published prebuilts match the pinned identity
(`rev` + patch hash + target) that launchers check before running a prebuilt.
