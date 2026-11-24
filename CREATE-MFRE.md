# CREATE-MFRE — Port de Create a Horizon: factibilidad y gaps

**Fase 41 (D19) · Sesión 11 · 2026-11-24**
Referencia de diseño: **Create 6.0.8 para mc1.21.1** (Modrinth, 2025-11-02).
API destino: **Horizon Mod API 0.2** (este repo, `docs/modding/`).

## 1. Veredicto

**GO condicionado.** Las fases F42 (MVP rotacional) y F43 (máquinas y logística) tienen
cobertura casi total en Mod API 0.2 — la documentación del motor incluye ejemplos
literales de lo que Create necesita (ejes rotatorios, cintas animadas, deployers,
fluidos en mB, recetas de máquina). F44 (contraptions/trenes) queda **condicionada** al
gap G1 (entidades libres), con un spike exploratorio al inicio de F43.

## 2. Marco legal

- Create es **MIT** (verificado en fuente primaria: `api.github.com/repos/simibubi/Create/license`
  → "MIT License, Copyright (c) 2019 simibubi"; el campo "Custom License" de CurseForge no es
  autoritativo). Portar con atribución es legal.
- Método: **reescritura clean-room** en Rust contra Mod API 0.2. No se traduce código Java
  (mixins/JVM/APIs de Forge no tienen mapeo 1:1); se reimplementan los *comportamientos*
  observables usando Create 6.0.8 como referencia de diseño.
- Atribución: entrada en `NOTICE-MFRE.md` + copia de la licencia MIT en `LICENSES/`
  (patrón F14). Crédito visible: "inspirado en Create, de simibubi y el equipo de Create".
- **Nombre propio**: el mod no se llama "Create". Id `mfre_kinetic`, nombre visible
  **"Cinética MFRE"** (es) / "MFRE Kinetics" (en).
- Assets: **100% propios** (paleta ember de F11 + estilo industrial). MIT permitiría
  reusar los originales con atribución, pero arrancamos propios para identidad del fork;
  si algún día se reusan, van al NOTICE.

## 3. Inventario de sistemas de Create (referencia 6.0.8)

| # | Sistema | Escala aproximada |
|---|---|---|
| 1 | Red rotacional: RPM/estrés, fuentes (rueda hidráulica, molino, manivela, motores), propagación (eje, engranaje, cinta, cadena), embrague/inversor, medidores | núcleo lógico grande |
| 2 | Máquinas de procesamiento: prensa, mezcladora, ruedas de trituración, molino de piedra, sierra, taladro, cosechadora, arado, **deployer**, spout, drenador, crafter mecánico | ~15 tipos |
| 3 | Logística: cintas (transporte de ítems), tolvas (chute), embudos, observador de contenido, interruptor de stock, bóveda, balanza | media |
| 4 | Fluidos: tubería, bomba, tanque multibloque, manguera, spout — **1000 mB = 1 balde** | media |
| 5 | Contraptions: rodamiento mecánico (estructuras giratorias), pistón/portal, ensamblador de vagonetas, asientos, controles, planos + cañón | **la más difícil** |
| 6 | Trenes: vías propias, bogies, señales, horarios, estaciones | grande, depende de 5 |
| 7 | Ponder (tutoriales guiados) | sistema propio |
| 8 | Contenido decorativo: ~500 bloques (paletas, ventanas, lámparas), ítems (llave, goggles, extendo-grip, lanza-papas) | grande pero mecánico |

## 4. Evidencia: qué ofrece Mod API 0.2 (mapeo necesidad → API → doc)

| Necesidad de Create | API disponible | Documento |
|---|---|---|
| Máquina con estado persistente y tick | `block_entity_type` + `BlockEntityPartState` (attachment guardado con el chunk, transaccional) + `block_entity_tick` | `concepts/parts/block-entity-types.md` |
| Animación cliente suave (giro entre updates) | `block_entity_client_tick` (+ opt-in `block_entity_client_ticks`) | `how-to/world/machine-menu-data.md` |
| Render de partes móviles (ejes, ruedas, brazos) | `block_entity_submit` con `BlockEntityDrawContext` (`quad/cube/block/item/model_part/push/pop/translate/rotate_about/scale`), render type `"INVISIBLE"` para que el BE dibuje todo | hooks + `how-to/content-mods.md` |
| Modelos rotados/horneados propios | `StandaloneModelForm::Rotated` (ejemplo doc: `mymod:block/shaft`) y `Combined` | `how-to/client/model-drawing.md` |
| Cinta animada (scroll de textura) | `draw.terrain_model(model, …with_uv_shift(TerrainUvShift))` — ejemplo doc: "machine's **belts, shafts** and casings" | `how-to/client/model-drawing.md` |
| Dibujado masivo de copias | grupos persistentes `create_group` (hasta 16,384 copias de un modelo sólido) | `how-to/client/model-drawing.md` |
| Caja de render grande (estructuras) | `block_entity_render_bounding_box` + `block_entity_view_distance` | hooks |
| Recetas de máquina (prensado/mixing/crushing) | `register_recipe_type` + `register_raw_recipe_serializer` (JSON propio publicado por `recipes_of_type`) | `crafting-recipe-kinds.md` |
| Crafter mecánico (grilla NxM propia) | `CraftingRecipeKind` con `CraftingGrid` (ancho/alto, celdas opcionales) | `crafting-recipe-kinds.md` |
| GUIs de máquina | tipos `menu/` + sync de datos (`machine-menu-data`) | `how-to/screen-room.md`, `how-to/world/machine-menu-data.md` |
| Deployer / sierra / taladro | `host.fake_player` + `PlayerAction::{UseOn, UseItem, InteractOn, Attack, BreakBlock}` — ejemplo doc: "a machine that acts as a player (**a deployer, a block breaker**)" | `how-to/entities/fake-players.md` |
| Fluidos (tuberías/tanques/bomba) | capability `FLUIDS` en **mB (1000 = 1 balde)**, `ModFluidTanks` por lado, `move_resources` transaccional, `ITEM_FLUIDS`, fluidos de contenido con balde | `concepts/capabilities/block-fluids.md`, `how-to/content-mods.md` |
| Interop de energía | capability `ENERGY` (estilo NeoForge) + `move_energy` | `concepts/capabilities/block-energy.md` |
| Sync cliente↔servidor (RPM, fase, redes) | payloads tipados `PayloadCodec` + `play_to_server_typed` / `play_to_client_typed` | `how-to/client/networking.md` |
| Ensamblar/desensamblar estructuras | bulk block edits tipados (paleta de `BlockState`, filtros, "preserve strict block-entity behavior", chunks completos) | `how-to/world/bulk-block-edits.md` |
| Planos (schematics) de otras versiones | `upgrade_block_entity_nbt` / `upgrade_entity_nbt` (datafixers del juego; el doc cita "a schematic") | `how-to/world/bulk-block-edits.md` |
| Lógica por tick de entidad (contraption/tren) | hooks `entity_tick_pre/post`, `entity_move`, `entity_size` (hitbox), `entity_invulnerability_check`, `living_motion_values`, persistencia custom (`requiresCustomPersistence`) | `crates/horizon-mod-vocab/src/hooks/` (756 hooks), `how-to/entities/living-state-flags.md` |
| Montar (asientos, contraptions tripuladas) | árbol pasajero/vehículo de primera clase (`passengers`, `is_passenger`, `is_vehicle`, hooks `vehicle_*`) | `how-to/entities/living-state-flags.md` |
| Renderer propio de entidad | `EntityRenderer` + `RegisterEntityRenderersEvent` (reemplaza el modelo completo; dibuja con el mismo contexto que un BE) | `how-to/client/entity-renderers.md` |
| HUD (goggles) | `hud-drawing` | `how-to/client/hud-drawing.md` |
| Comportamiento de bloques | `BlockPart`: `dynamic_light`, conexiones (vallas), pathfinding, `on_block_state_change`, reacciones a pistón, sticky | `concepts/parts/block-parts.md` |
| Contenido decorativo (~500 bloques) | paquetes de contenido declarativos (patrón F20 ember) | `how-to/content-mods.md` |

**Conclusión de la tabla**: todo lo que F42/F43 necesitan existe y está documentado. Los
BE parts **sí cruzan a wasm** (los *entity* parts no: `r.entity_part` es nativo-only,
"entity parts do not cross to wasm yet").

## 5. Gaps y riesgos

| # | Gap | Severidad | Mitigación |
|---|---|---|---|
| **G1** | **Entidades libres para contraptions/trenes.** Los tipos de entidad declarables copian plantilla vanilla: `minecraft:snowball` (thrown) o living mob. No existe una clase de entidad arbitraria como `AbstractContraptionEntity` de Create. | **ALTA** (solo F44) | Entidad con plantilla living + `EntityRenderer` propio (dibuja la estructura: `block/cube/model_part`), `entity_size` para la hitbox, `entity_tick_pre/post` para la física propia, `entity_invulnerability_check` + `living_motion_values` para anular daño/gravedad no deseada, `requiresCustomPersistence` contra despawn, attachments para el estado, pasajeros para asientos. **Spike mínimo en F43** (entidad-prototipo: renderer + tick + tamaño + pasajero). Si el muro es real: issue a upstream (API activa, 756 hooks) y F44 se re-planifica. |
| **G2** | Bulk edits no representan NBT ("NBT is not representable") → una contraption con cofres/hornos perdería el contenido si solo se usa el bulk edit. | MEDIA | Tras el bulk edit, escribir cada block entity por su handle (los edits "preserve strict block-entity behavior"; la escritura posterior es la ruta documentada para datos). |
| **G3** | La red rotacional (propagación, estrés, RPM, cachés) es código propio puro: no hay un "kinetic system" en la API. | MEDIA (esperado) | Es justamente el trabajo clean-room: grafo en attachments de BE (`network_id`, `speed`, `stress`), BFS en `on_block_state_change`/ticks, sync por payload. Create lo resuelve igual (su `RotationPropagator` es código de mod, no de Forge). |
| **G4** | Entity parts son nativo-only → **Cinética MFRE será mod nativo**, no wasm. El toolchain validado en F18 es wasm. | MEDIA | La ruta nativa es primera clase (13 guías: empaquetado, multiplayer, export de servidor dedicado, trust). F42 arranca con spike "native hello-world" (validar toolchain nativo antes del código cinético). Los mods de contenido decorativo sí pueden ser wasm/content puros. |
| **G5** | Límites de registro: 2^20 block states, 16,384 sprites; ~500 bloques con variantes (ejes por eje, cintas por dirección…) consumen atlas y estados. | BAJA | Cabe de sobra, pero medir en F42 (los bloques cinéticos son pocos; la decoración va en paquetes aparte). |
| **G6** | Ponder (tutoriales) no tiene equivalente directo. | BAJA | Fuera de alcance F42–F44. `screens` + `living-scenes` podrían aproximar; se decide después. |
| **G7** | Trenes: vías propias + señales + horarios sobre la entidad de G1; además el render de vías suaves puede requerir engine (D10). | BAJA→ALTA | F44 queda condicionado a G1; trenes son el último hito y pueden moverse a un F46 si el spike falla. |
| **G8** | Fluidos de mundo propios (chocolate, miel de Create). | BAJA | Fluidos declarables por contenido (`horizon/fluid/`, con balde). El transporte interno de Create no usa fluidos de mundo (tuberías/tanques = datos), que es lo que `FLUIDS` cubre. |

## 6. Arquitectura propuesta

- **Un mod nativo** `mfre_kinetic` ("Cinética MFRE") con `data/` + `assets/` propios
  (los mods nativos llevan árboles de pack normales, `how-to/native-mods/03-data-files.md`),
  más **paquetes de contenido** separados para la decoración masiva (patrón F20).
- **Núcleo cinético**: grafo rotacional en attachments de BE (nodo: `speed`, `stress`,
  `network`, `phase`); propagación por BFS ante cambios de bloque; overload → la red se
  detiene (como Create). Sync al cliente: payload con RPM+fase por red visible +
  interpolación en `block_entity_client_tick`.
- **Render**: modelos standalone por pieza (formas `Rotated` por eje), `block_entity_submit`
  dibuja con `rotate_about(ángulo)`; cintas con `terrain_model` + `TerrainUvShift`.
- **Recetas**: tipos `mfre_kinetic:pressing`, `:mixing`, `:crushing`, `:bulk_smelting`,
  `:mechanical_crafting` (serializer crudo + lectura propia en las máquinas).
- **Máquinas**: un `BlockEntityPart` por familia, estado en attachments, menús con sync.
- **Deployer/sierra**: fake players con `PlayerAction`.
- **Fluidos**: capability `FLUIDS` + `ModFluidTanks`; red de tuberías propia (grafo, como el cinético).

## 7. Plan de fases (aceptación concreta)

### F42 — MVP rotacional
Spike inicial: toolchain **nativo** validado (hello-world nativo en LAN, como F18 hizo con wasm).
Luego: rueda hidráulica + manivela (fuentes), eje, engranaje, engranaje grande,
inversor y embrague, medidor de velocidad y de estrés (con menú), llave (rotar/desmontar),
propagación de red con estrés y overload, render giratorio interpolado, lang es/en.
**Aceptación**: una rueda hidráulica mueve una red de ejes/engranajes visible y suave;
añadir carga por encima del estrés disponible detiene la red; los medidores muestran RPM
y estrés reales; QA en hardware (§5.13).

### F43 — Máquinas y logística
Spike G1 (entidad-prototipo). Basin + prensa + mezcladora, ruedas de trituración,
molino de piedra, sierra mecánica, deployer (fake player), cinta con transporte de ítems
y render animado, chute + embudo, bomba/tubería/tanque/spout, crafter mecánico,
recetas propias en JSON. **Aceptación**: granja automática (sierra+taladro opcional),
línea de trigo→harina→pan mecánico, prensa con receta propia, red de fluidos con tanque
visible lleno/vacío.

### F44 — Contraptions y trenes (condicionada a G1)
Rodamiento mecánico (estructura giratoria como entidad), pistón/portal mecánico,
asientos tripulables, ensamblador de vagonetas, planos + cañón (bulk edits + datafixers).
Trenes (vía/señal/horario) solo si el spike de F43 confirma G1 viable; si no, se mueven
a F46 y se documenta la dependencia (posible issue a upstream / engine D10).
**Aceptación**: puerta giratoria de 5×5 con rodamiento, máquina voladora simple con
pistones, jugador sentado viaja con la contraption sin desync en LAN.

## 8. Decisiones que formaliza este documento

- **D21**: Cinética MFRE = **mod nativo** (`mfre_kinetic`), assets 100% propios, referencia
  Create 6.0.8/mc1.21.1, atribución MIT en NOTICE-MFRE + LICENSES/. La red rotacional y las
  recetas de máquina son código clean-room propio sobre Mod API 0.2.
- **D22**: F44 queda **condicionada** al resultado del spike G1 (entidad libre sobre
  plantilla living). Trenes pueden moverse a F46.
- **D20** (registrada en TRAYECTORIA con esta fase): la conversora de contenido Java→.hmod
  queda añadida al roadmap como **F45** (confirmada por el usuario).

## 9. Riesgo global y esfuerzo

F42 y F43 son de riesgo **bajo-medio** (la API cubre todo; el trabajo es volumen).
F44 es **alto** en su parte de entidades (G1) y es la única que puede fallar.
Orden de ejecución: spike nativo → núcleo cinético → máquinas → spike entidad → contraptions.
Cada fase cierra con su parche, gates y QA (GUÍA-QA §5.13+).
