# MineForge Rust Edition — Notices and Licenses

MineForge Rust Edition (MFRE) is a community fork of Horizon
(`horizon-rust/horizon`), distributed under the same terms as upstream:
**GNU LGPL-3.0-or-later**. This notice complements — never replaces — the
upstream `LICENSE` file and the `LICENSES/` directory, both of which ship
unchanged with every source and binary distribution of the fork.

## Upstream license and copyright

- Horizon engine, launcher and tooling: © the Horizon contributors,
  LGPL-3.0-or-later (see `LICENSE`, `LICENSES/GPL-3.0.txt`).
- Components upstream carries under MIT terms keep them (`LICENSES/MIT.txt`);
  DirectX shader compiler notice: `packaging/windows/DXC-NOTICE.txt`.
- Every crate of the fork keeps `license = "LGPL-3.0-or-later"` in its
  `Cargo.toml`, as upstream sets it.

## MFRE's own changes and additions

- The fork's patch series (0001–…, see the git history of `mfre/main`) is
  © MineForge Rust Edition, licensed LGPL-3.0-or-later — the same license as
  the work it modifies.
- The fork's brand art (logo, application icons, the title panorama in
  `crates/horizon-prepare/src/mfre_panorama/`) is drawn in code by
  `tools/mfre_brand_icons.py` and `tools/mfre_panorama.py`; © MineForge Rust
  Edition, LGPL-3.0-or-later. It contains no Mojang or Microsoft artwork.

## Third-party assets the fork carries

- Fonts Geist, Geist Mono, Pixelify Sans and Bricolage Grotesque: SIL Open
  Font License 1.1; their license texts ship embedded in
  `crates/horizon-theme/fonts/OFL-*.txt`.
- Icon sets Pixelarticons and Hugeicons: MIT; attributed in
  `docs/design-system.md`.
- Microsoft's four-square sign-in symbol is drawn only where upstream draws
  it, per Microsoft's brand guidance (`launcher/src/microsoft.rs`; see
  upstream `THIRD_PARTY_NOTICES.md`). The fork shows it nowhere by default:
  its account flow is offline profiles.

## Complete corresponding source (LGPL-3.0 §§4, 6)

The complete corresponding source of any distributed MFRE binary is the
fork's repository: the base snapshot plus the patch series (the git bundle
and the numbered patches). Those materials travel with every release
artifact; public publication is scheduled for Fase 26 of the fork's roadmap.

## What MFRE does not carry

- **No Microsoft Azure client ID.** Microsoft sign-in is configuration
  (`HORIZON_MSA_CLIENT_ID`, build- or run-time); the fork ships none and its
  flow is offline profiles. A test asserts no client ID is compiled in.
- **No Mojang or Microsoft files redistributed.** Preparation copies from the
  player's own Minecraft install on their own computer, and the fork's title
  panorama is its own art (Fase 12).
- **No Horizon signed self-update.** A fork binary never replaces itself
  (D6): `self_update_allowed()` is off outside development, and a test
  asserts it. Upstream's release-sign keys verify only upstream's own engine
  release artifacts when the launcher installs an engine build; they are
  never applied to the fork's binary.
