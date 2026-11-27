# MODS-MFRE — Toolchain de mods validada (Fase 18)

**Actualizado:** 2026-11-24 · **Rama:** `mfre/main` (parche 0022) · **Estado:** validada en
sandbox 🟦 (wasm **y nativo game-tree**, F42); lo que necesita GPU o el motor prebuilt queda
marcado 🖥️ (máquina real, GUÍA-QA §5.10 y §5.13).

La infraestructura de mods es la de upstream y el fork la conserva completa y sin tocar
(`git diff` del fork sobre `crates/horizon-mod-*`, `crates/cargo-horizon-mod` y `docs/modding/`
está vacío): `cargo-horizon-mod` (la herramienta), `docs/modding/` (el libro y sus ejemplos),
`crates/horizon-mod-*` (API 0.2, runtime wasmtime, paquete `.hmod`, registro). Este documento
registra qué se validó en la sandbox del fork, cómo se usa y qué decisiones aplican a MFRE.

## 1. Validación en la sandbox (2026-11-23)

| # | Prueba | Resultado |
|---|--------|-----------|
| 1 | `cargo build -p cargo-horizon-mod` desde la fuente del fork | ✅ ~5 min en frío |
| 2 | Modo game-tree: `build --wasm` en `docs/modding/examples/wasm-hello` (API por path al árbol del repo; sin descarga de SDK) | ✅ `mod.wasm` 12.8 MB + `hello_wasm.hmod` 2.9 MB, ~4 min |
| 3 | `cargo horizon-mod check hello_wasm.hmod` (chequeos del registro: tamaño, SHA-256, manifiesto, contenido) | ✅ |
| 4 | Flujo estándar: `cargo horizon-mod new <mod> --template wasm` → fetch del SDK upstream **firmado** (build `f1bb66f353cf…`, 21 MB; firma y `sdk.json` verificados) | ✅ (requiere red a GitHub) |
| 5 | `build --wasm` del mod nuevo contra el SDK fijado + `check` | ✅ `mfre_new_test.hmod` 2.9 MB |
| 6 | Tests de la herramienta: `cargo test -p cargo-horizon-mod` | ✅ 51/0 (20 ignored) |
| 7 | Tests de empaquetado: `cargo test -p horizon-mod-package` | ✅ 176/0 |
| 8 | `wasm-std` (el miembro std del juego) y el fixture de pruebas, construidos con la herramienta | ✅ |
| 9 | Runtime: `cargo test -p horizon-mod-wasm --test real_member --features listener-fixture` con `HORIZON_WASM_STD`/`HORIZON_WASM_FIXTURE` locales | ⚠️ ver abajo |
| 10 | **Nativo game-tree (F42, 2026-11-24)**: `mods/mfre-kinetic` (hello nativo, D21/D23) — `cargo check` ✅ (2 min, 62 crates), `cargo horizon-mod build` ✅ "Building against the game checkout" → `mfre_kinetic.hmod` 5,480 B sha256 `1d509f80…`, `check` ✅ "native-mod, format native-1/0719b37…/e59da90…, for 26.3" | ✅ paquete+check; compilación/carga vía launcher 🖥️ (§5.13) |

Sobre (9): `dump_members` pasa y `real_member` **ejecuta el miembro wasm real de punta a punta**
(carga del grupo, std compartido, registro, inicialización del guest, dispatch de hooks y ticks
de entidad/block-entity) antes de caer en `resolve_items`: el registro de pruebas del host no
conoce `minecraft:paper`, que el fixture pide. Ese desajuste fixture/host viene del snapshot
upstream (el fork no toca esos crates) y el test está `#[ignore]` en upstream — ningún workflow
adoptado construye sus fixtures (`tools/ci/ignored-tests/wasm-fixture.txt`). El runtime wasm
queda así validado en lo que importa: enlaza, carga y ejecuta mods reales.

**Toolchain**: la herramienta instala sola, vía rustup, el Rust fijado en `tools/ci/rust-version`
(**1.99.1**, commit `58cf8f9c3…`) con `rust-src` y el target `wasm32-unknown-unknown`; std se
compila desde fuente (`-Zbuild-std`) para que todos los mods de un grupo compartan símbolos.
La sección `[wasm]` del manifiesto (sha512 + toolchain) la escribe la herramienta; no editar a mano.

## 2. Cómo se construye un mod WASM en MFRE

```sh
# Herramienta desde la fuente del fork (sin binstall: el fork no publica releases todavía, F26):
cargo build -p cargo-horizon-mod
export PATH="$CARGO_TARGET_DIR/debug:$PATH"   # para que `cargo horizon-mod` la encuentre

# Opción A — mod independiente (flujo estándar, SDK upstream firmado):
cargo horizon-mod new mi-mod --template wasm
cd mi-mod
cargo horizon-mod build --wasm                 # → mod.wasm + target/horizon-mod/mi_mod.hmod
cargo horizon-mod check target/horizon-mod/mi_mod.hmod

# Opción B — dentro del árbol del juego (sin descarga de SDK; así viven los ejemplos del libro):
cd docs/modding/examples/wasm-hello
cargo horizon-mod build --wasm
```

`cargo horizon-mod run` (build + arrancar `horizon-client` con `--dev-mod <id>.hmod`) necesita
GPU → máquina real (GUÍA-QA §5.10). De los mods nativos, el **empaquetado y el check sí caben en
la sandbox** (game-tree: el `Cargo.toml` del mod apunta por path a `crates/horizon-mod-api` del
árbol; probado en F42 con `mods/mfre-kinetic`); la **compilación contra el SDK y la carga**
(`horizon-launcher native-build <ruta> --trust-native-code`, o base developer +
`HORIZON_LAUNCHER_DEVELOPER_SDK`) necesitan el motor instalado → máquina real (GUÍA-QA §5.13).

## 3. Decisiones y limitaciones del fork

- **SDK**: desde F36 inc.A el fork publica su propio SDK: el workflow **mfre sdk** (tag
  `sdk-<sha40>`) sube la release prerelease `sdk-<commit>` firmada, y las releases del juego
  adjuntan su `sdk.json`; `SDK_RELEASES`/`SDK_RELEASE_LIST` (horizon-addresses) ya apuntan al
  fork, así que `new`/`update`/`check`/`build` lo encuentran solos (`HORIZON_SDK_BASE` acepta
  un espejo adicional, URL o carpeta). `mods/mfre-kinetic` pasó del game-tree del spike F42 al
  flujo estándar (dep `horizon-mod-api = { path = ".horizon/sdk/api/crates/horizon-mod-api" }`,
  pin = el build que sirve la release del juego): el `.hmod` del catálogo debe ser
  autocontenido, y el path al árbol (`../../crates/…`) no existe en la máquina del jugador.
  La **opción B** (game-tree) sigue valiendo para spikes locales sin SDK publicado.
- **sha512 no bit-reproducible entre hosts**: reconstruir `wasm-hello` con la misma toolchain
  fijada dio un sha512 distinto al clavado en upstream (rutas/host embebidos). No es fallo: la
  herramienta reescribe `[wasm]` en cada build y el juego verifica `mod.wasm` contra el
  manifiesto de su propio `.hmod`. Los `.hmod` del fork se firman/distribuyen en F26.
- **Fase 8 LOCKED sigue aplicando**: el juego canónico del fork se compila
  `--no-default-features --features audio,controllers`; el runtime de mods (wasmtime) no es una
  feature desconectable y viaja en el build canónico.
- **F20** (primer mod propio MFRE) usará esta toolchain: proyecto en el árbol del fork
  (opción B) con `id` en namespace propio.

## 4. Referencias del repo

- Libro: `docs/modding/src/` (`how-to/content-mods.md`, `how-to/wasm-mods.md`, `how-to/native-mods.md`).
- Ejemplos: `docs/modding/examples/wasm-hello` (hooks de chat/ticks/fluidos) y `wasm-widgets`.
- Herramienta: `crates/cargo-horizon-mod/` (plantillas `template/wasm`, `template/native`).
- Runtime: `crates/horizon-mod-wasm/` (tests `real_member`, `real_library`, grupos, cachés).
- Empaquetado/manifiesto: `crates/horizon-mod-package/`.
