# SHADERS-MFRE — Shader API 0.2 validada (Fase 19, parte sandbox)

**Actualizado:** 2026-11-24 · **Rama:** `mfre/main` (parche 0018) · **Estado:** validación
sandbox 🟦 completa; la QA visual es 🖥️ (GUÍA-QA §5.11).

Como con los mods (MODS-MFRE.md), la infraestructura de shaders es la de upstream y el fork
la conserva sin tocar: `crates/horizon-shader-api` (API 0.2, CLI `horizon-shader`, pack de
referencia `packs/golden_hour`), `crates/horizon-shaders` (loader: post/sky/material, biomas,
preproceso estilo Iris/OptiFine) y `crates/horizon-optifine` (compatibilidad de packs).

## Validación en la sandbox (2026-11-24, todo verde)

| # | Prueba | Resultado |
|---|--------|-----------|
| 1 | `cargo build -p horizon-shader-api` (CLI `horizon-shader`) | ✅ |
| 2 | `horizon-shader check crates/horizon-shader-api/packs/golden_hour` | ✅ `golden_hour 2.0.0 (shader API 0.2): ok` — sky + 2 post validados (naga/WGSL), sha256 `a9b1e7df…` |
| 3 | `horizon-shader pack golden_hour --out golden_hour.hshader` | ✅ 3,249 B, sha256 idéntico al de `check` |
| 4 | `cargo horizon-mod check golden_hour.hshader` | ✅ reconocido como "shader, shader API 0.2" |
| 5 | `cargo test -p horizon-shader-api -p horizon-shaders -p horizon-optifine` | ✅ 682/0 (12 ignored) |

## Pendiente en máquina real (GUÍA-QA §5.11)

- `golden_hour` en juego: cielo cálido al amanecer/atardecer, glow alrededor de sol/lava/
  antorchas, grade de color, viñeta; las 5 opciones del pack (`sun_glow`, `glow`, `warmth`,
  `saturation`, `vignette`) mueven el resultado; hot reload al editar un `.wgsl`.
- Loader Iris/OptiFine con un pack real descargado (p. ej. Complementary/BSL): carga o
  degradación con mensaje claro, sin crash.
- Presupuesto GPU del pack: 1.0 ms (`[budget] gpu_ms`) — medir en hardware.

## Notas del fork

- Los packs de shaders son contenido local: cero red, coherente con offline-first (D5/D6).
- Cuando exista el sitio propio (F37), `golden_hour` y packs MFRE se distribuyen desde ahí;
  el formato `.hshader` ya se valida con la misma herramienta que los `.hmod`.
- F21/F38 (catálogo) no aplica a shaders por ahora; el loader los descubre en sus carpetas
  (docs/shaders.md del repo).
