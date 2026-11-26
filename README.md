# MineForge Rust Edition (MFRE)

**Minecraft 26.3, reescrito desde cero en Rust — launcher, cliente y servidor dedicado.**
*A community fork of Horizon: Minecraft 26.3 rewritten from scratch in Rust — launcher, game client and dedicated server.*

[![Release](https://img.shields.io/badge/release-v26.3.0-6f42c1)](../../releases/latest)
[![Licencia](https://img.shields.io/badge/license-LGPL--3.0--or--later-blue)](LICENSE)

---

## Descargar / Download

| Qué | Dónde |
|---|---|
| **Launcher** (Windows x64 / Linux x86-64) | [Releases `launcher@…`](../../releases?q=launcher&expanded=true) — `horizon-launcher-setup.exe` se instala solo |
| **Juego + servidor** (los baja el launcher automáticamente) | [Releases `v…`](../../releases) — instalables desde el launcher, firmados (SHA256SUMS + firma Ed25519) |

## Jugar (3 pasos, sin línea de comandos)

1. Descarga el launcher y ábrelo.
2. Acepta el EULA de Minecraft y pulsa **Instalar y jugar**.
3. Listo: el launcher descarga la versión estable, crea la instancia, prepara los recursos y lanza el juego.

**Necesitas tu propia copia de Minecraft: Java Edition** (versión 26.3, iniciada una vez desde su
launcher oficial). MFRE no distribuye ningún archivo de Mojang: el launcher los toma de tu copia o
los descarga de Mojang con el EULA aceptado, verificando cada SHA-1.

- **Plataformas:** Windows x64, Linux x86-64.

## Requisitos (orientativos, en medición)

- **GPU con Vulkan 1.1, DirectX 12 u OpenGL 3.3+** (integradas tipo Intel HD 4000 o mejores).
  Sin eso el juego cae a **renderizado por software** (la CPU dibuja todo, muy lento) y lo avisa
  con un toast al arrancar.
- **RAM:** 4 GB mínimo, 8 GB recomendado. **Disco:** ~2 GB + las versiones instaladas.
- **PC vieja:** dentro del juego, Opciones → Gráficos → preset **Retro** (distancia de render 2,
  nubes, cielo, clima y texturas animadas apagados, escala de resolución 15 %; el slider llega
  hasta 10 %) es lo que más FPS da; detalle en
  [el sitio del fork](https://omarPVP123131.github.io/mineforge-rust-edition/#requisitos).

## Compilar desde la fuente

```sh
# El launcher (workspace propio):
cd launcher && cargo build --release        # exe en launcher/target/release/

# El juego (desde la raíz del repositorio):
cargo build --profile fast -p horizon-game --bin horizon-client
```

Toolchain: la que fija `tools/ci/rust-version`. Los recursos del juego se preparan con
`horizon-prepare` desde tu propia instalación de Minecraft (`docs/native-client.md`).

## El repositorio

- `main` — la serie de parches del fork sobre la base Horizon `0949a05` (historial completo;
  cada parche MFRE está numerado y firmado en el mensaje de commit).
- Tags `vX.Y.Z` — releases del juego (las publica `.github/workflows/mfre-release.yml`).
- Tags `launcher@X.Y.Z` — releases del launcher (`.github/workflows/mfre-launcher.yml`).
- `docs/` — documentación técnica heredada del upstream, más la propia del fork.
- `mods/mfre-*` — los mods nativos del fork (Kinética, Ascuas).

## Avisos legales / Legal notices

Proyecto fan independiente, sin ánimo comercial. **NO es un producto oficial de Minecraft. No está
aprobado por ni asociado con Mojang o Microsoft.** "Minecraft" es una marca de Mojang Synergies AB;
su código y sus assets pertenecen a Mojang/Microsoft bajo el
[Minecraft EULA](https://www.minecraft.net/en-us/eula). Sin relación con el proyecto Minecraft Forge.

MineForge Rust Edition es un fork comunitario de [Horizon](https://github.com/horizon-rust/horizon)
(© los contribuidores de Horizon), distribuido bajo sus mismos términos:
**GNU LGPL-3.0-or-later** ([LICENSE](LICENSE), [LICENSES/](LICENSES)). Código anterior a la
migración desde MIT sigue disponible bajo [MIT](LICENSES/MIT.txt). Las dependencias conservan sus
propias licencias: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Los cambios y añadidos del
fork: [NOTICE-MFRE.md](NOTICE-MFRE.md). Agradecemos al equipo de Horizon la base.

NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.
Unaffiliated with the Minecraft Forge project.
