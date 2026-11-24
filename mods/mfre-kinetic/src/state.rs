//! El estado cinético sobre el nivel real: el attachment persistido, el Part del block
//! entity (tick de servidor, tag de sync y render de cliente), el Part del bloque de la
//! manivela (clic derecho) y el adaptador de sólo lectura que el resolvedor puro
//! (`kinetic`) usa.
use crate::kinetic::{self, Axis, Kind, KineticWorld, Node, add, degrees_per_tick};
use mod_api::*;
use std::sync::OnceLock;

/// Claves de registro fijadas una vez en `Hooks::register`.
pub static SHAFT: OnceLock<BlockKey> = OnceLock::new();
pub static WHEEL: OnceLock<BlockKey> = OnceLock::new();
pub static COG: OnceLock<BlockKey> = OnceLock::new();
pub static LARGE_COG: OnceLock<BlockKey> = OnceLock::new();
pub static CRANK: OnceLock<BlockKey> = OnceLock::new();
pub static SPEEDOMETER: OnceLock<BlockKey> = OnceLock::new();
pub static STRESSOMETER: OnceLock<BlockKey> = OnceLock::new();
pub static WATER: OnceLock<BlockKey> = OnceLock::new();

/// Las RPM de una rueda hidráulica con agua y su capacidad de estrés.
pub const WHEEL_RPM: f32 = 8.0;
pub const WHEEL_CAPACITY: i32 = 16;
/// Las RPM de una manivela activa, su capacidad y los ticks que dura cada giro.
pub const CRANK_RPM: f32 = 8.0;
pub const CRANK_CAPACITY: i32 = 8;
pub const CRANK_TICKS: i32 = 20;

/// Claves del espejo en el nbt guardado: lo que `block_entity_update_tag` envía al cliente
/// y el render lee. El attachment sigue siendo la verdad del servidor; el espejo existe
/// porque `update_tag` no recibe host y no puede leer el attachment. La velocidad viaja
/// como los bits IEEE de su f32 (`Tag::Float` los guarda así).
pub const NBT_SPEED: &str = "mfre_speed";
pub const NBT_CONNS: &str = "mfre_conns";
pub const NBT_CAP: &str = "mfre_cap";
pub const NBT_LOAD: &str = "mfre_load";

/// El estado cinético que cada nodo guarda: sus conexiones axiales, su velocidad y los
/// totales de estrés de su red; la manivela añade su cuenta regresiva y su sentido. Se
/// persiste con el chunk a través del códec del attachment.
#[derive(Clone, Copy, Debug, PartialEq, Wire)]
pub struct KineticData {
    /// El bitmask de conexiones axiales de la última resolución.
    pub conns: u8,
    /// Su velocidad con signo (RPM alrededor de su axis); 0 = detenida.
    pub speed: f32,
    /// La capacidad de estrés de su red (última resolución).
    pub capacity: i32,
    /// La carga de estrés de su red (última resolución).
    pub load: i32,
    /// Manivela: los ticks que le quedan activos; 0 = en reposo.
    pub crank: i32,
    /// Manivela: el sentido del próximo giro (+1/-1); alterna con cada clic.
    pub dir: i32,
}

impl Default for KineticData {
    fn default() -> Self {
        Self {
            conns: 0,
            speed: 0.0,
            capacity: 0,
            load: 0,
            crank: 0,
            dir: 1,
        }
    }
}

/// El attachment por block entity cinético (`BlockEntityPartState::DATA`).
pub static KINETIC: AttachmentType<KineticData> = AttachmentType {
    namespace: "mfre_kinetic",
    name: "kinetic",
    default: KineticData::default,
    codec: Some(AttachmentCodec {
        encode: |data| {
            Ok([
                ("c".into(), Tag::Int(i32::from(data.conns))),
                ("s".into(), Tag::Float(data.speed.to_bits())),
                ("k".into(), Tag::Int(data.capacity)),
                ("l".into(), Tag::Int(data.load)),
                ("t".into(), Tag::Int(data.crank)),
                ("d".into(), Tag::Int(data.dir)),
            ]
            .into())
        },
        decode: |saved: &Compound| {
            let conns = match saved.get(&"c".into()) {
                Some(Tag::Int(n)) if (0..=255).contains(n) => *n as u8,
                _ => return Err(AttachmentError::Decode),
            };
            let speed = match saved.get(&"s".into()) {
                Some(Tag::Float(bits)) => f32::from_bits(*bits),
                _ => return Err(AttachmentError::Decode),
            };
            // Los campos nuevos (inc.5) faltan en los chunks guardados por inc.4:
            // se leen como su valor por defecto en vez de fallar.
            let int_or = |key: &str, default: i32| match saved.get(&key.into()) {
                Some(Tag::Int(n)) => *n,
                _ => default,
            };
            Ok(KineticData {
                conns,
                speed,
                capacity: int_or("k", 0),
                load: int_or("l", 0),
                crank: int_or("t", 0),
                dir: int_or("d", 1),
            })
        },
    }),
    copy_on_death: false,
    sync: false,
};

/// El Part de los bloques cinéticos: los hace ticar, mantiene la red resuelta y la dibuja
/// en el cliente.
pub struct Kinetic;

/// Los ids de los bloques con block entity cinético.
pub const KINETIC_BLOCKS: [&str; 7] = [
    "mfre_kinetic:shaft",
    "mfre_kinetic:water_wheel",
    "mfre_kinetic:cogwheel",
    "mfre_kinetic:large_cogwheel",
    "mfre_kinetic:hand_crank",
    "mfre_kinetic:speedometer",
    "mfre_kinetic:stressometer",
];

#[mod_api::horizon_part]
impl BlockPart for Kinetic {
    /// Clic derecho con la mano vacía en la manivela (servidor, mano principal): la activa
    /// por `CRANK_TICKS` ticks y alterna el sentido, como al dar manivelazos reales. El
    /// tick siguiente resuelve la red con la fuente activa y sincroniza el espejo.
    fn use_without_item(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseWithoutItemEvent<'_>,
    ) -> Option<InteractionResult> {
        let pos = event.click.pos;
        let level = host.level(host.dimension())?;
        let entity = host.block_entity_identity(level, pos)?;
        let _ = Kinetic::edit_state(host, entity, |_, data| {
            data.crank = CRANK_TICKS;
            data.dir = -data.dir;
        });
        Some(InteractionResult::SuccessServer)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for Kinetic {
    /// Los block entities cinéticos tican en el servidor (`EntityBlock.getTicker`);
    /// nunca limpia un valor que otro mod o mixin fijó.
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if KINETIC_BLOCKS.contains(&event.block) {
            event.ticks = true;
        }
    }

    /// Un tick de servidor: resuelve la red desde este nodo y se auto-corrige — cada nodo
    /// actualiza SU attachment (verdad del servidor) y SU espejo nbt (lo que el cliente
    /// ve), y pide `block_entity_sync` cuando el espejo cambió. Así la red converge en un
    /// tick sin escrituras cruzadas ni transacciones entre nodos. La manivela además
    /// descuenta su cuenta regresiva (el resolvedor lee el valor previo, así el último
    /// tick activo todavía genera).
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        // Sin cambio no se ensucia el chunk: `Unchanged` descarta el payload del tick.
        *event.reply = BlockEntityTickReply::Unchanged;
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        let entity = host.block_entity_identity(level, event.pos);
        let stored = match entity {
            Some(entity) => Kinetic::state(host, entity).unwrap_or_default(),
            None => KineticData::default(),
        };
        let solution = {
            let world = LevelWorld {
                host: &*host,
                level,
            };
            match kinetic::solve(&world, event.pos) {
                Some(solution) => solution,
                None => return,
            }
        };
        let Some(me) = solution
            .members
            .iter()
            .find(|m| m.pos == event.pos)
            .copied()
        else {
            return;
        };
        let expected = KineticData {
            conns: me.conns,
            speed: me.speed,
            capacity: solution.capacity,
            load: solution.load,
            crank: stored.crank.saturating_sub(1),
            dir: stored.dir,
        };

        // ¿El attachment ya tiene lo esperado? (sin identidad no hay attachment que corregir)
        let stored_ok = entity.is_none() || stored == expected;
        // ¿El espejo nbt ya tiene lo esperado?
        let mirror_ok = nbt_speed(&event.entity.nbt) == Some(expected.speed)
            && nbt_int(&event.entity.nbt, NBT_CONNS) == Some(i32::from(expected.conns))
            && nbt_int(&event.entity.nbt, NBT_CAP) == Some(expected.capacity)
            && nbt_int(&event.entity.nbt, NBT_LOAD) == Some(expected.load);
        if stored_ok && mirror_ok {
            return;
        }

        // Algo cambió: el tick queda `Changed` y se guarda.
        *event.reply = BlockEntityTickReply::Changed;
        if !stored_ok && let Some(entity) = entity {
            let _ = Kinetic::edit_state(host, entity, |_, data| *data = expected);
        }
        if !mirror_ok {
            event
                .entity
                .nbt
                .insert(NBT_SPEED.into(), Tag::Float(expected.speed.to_bits()));
            event
                .entity
                .nbt
                .insert(NBT_CONNS.into(), Tag::Int(i32::from(expected.conns)));
            event
                .entity
                .nbt
                .insert(NBT_CAP.into(), Tag::Int(expected.capacity));
            event
                .entity
                .nbt
                .insert(NBT_LOAD.into(), Tag::Int(expected.load));
            // Envía el tag a quien ve el chunk (con el nbt espejo recién escrito); los
            // cambios hechos en el tick salen al inicio del tick siguiente, como en vanilla.
            host.block_entity_sync(event.pos);
        }
    }

    /// El tag que el cliente recibe al cargar el chunk o tras `block_entity_sync`: el
    /// espejo nbt (velocidad, conexiones y estrés). Sin host no se puede leer el attachment.
    fn block_entity_update_tag(
        &self,
        event: &mut BlockEntityUpdateTagEvent<'_>,
    ) -> Option<Compound> {
        let mut tag = Compound::new();
        for key in [NBT_SPEED, NBT_CONNS, NBT_CAP, NBT_LOAD] {
            if let Some(value) = event.entity.nbt.get(&key.into()) {
                tag.insert(key.into(), value.clone());
            }
        }
        (!tag.is_empty()).then_some(tag)
    }

    /// El render de cliente: ejes/ruedas/engranajes/manivela giran alrededor del eje del
    /// bloque a la velocidad del espejo nbt (`spin` + geometría en caché con `keep`); los
    /// medidores son estáticos con una aguja rotada según su lectura (`rotate_about`).
    fn block_entity_submit(&self, host: &dyn ClientHost, event: &mut BlockEntitySubmitEvent<'_>) {
        let speed = nbt_speed(&event.entity.nbt).unwrap_or(0.0);
        let facts = host.block_states();
        let axis = event
            .state
            .property(&facts, "axis")
            .ok()
            .flatten()
            .and_then(Axis::parse)
            .unwrap_or(Axis::Y);
        let draw = &mut *event.draw;
        match event.block {
            "mfre_kinetic:speedometer" => {
                // Aguja: ±90° para ±32 RPM (tope del dial).
                let degrees = speed.clamp(-32.0, 32.0) / 32.0 * 90.0;
                draw_meter(draw, axis, degrees, false, TINT, "speedometer_dial");
            }
            "mfre_kinetic:stressometer" => {
                // Aguja: -90° vacío, +90° al doble de la capacidad; roja en sobrecarga.
                let capacity = nbt_int(&event.entity.nbt, NBT_CAP).unwrap_or(0);
                let load = nbt_int(&event.entity.nbt, NBT_LOAD).unwrap_or(0);
                let ratio = if capacity > 0 {
                    load as f32 / capacity as f32
                } else {
                    0.0
                };
                let degrees = ratio.clamp(0.0, 2.0) * 90.0 - 90.0;
                let tint = if load > capacity { 0xFF_40_40 } else { TINT };
                draw_meter(draw, axis, degrees, true, tint, "stressometer_dial");
            }
            _ => {
                draw.spin(axis.vector(), [0.5, 0.5, 0.5], degrees_per_tick(speed), 0.0);
                match event.block {
                    "mfre_kinetic:water_wheel" => draw_wheel(draw, axis),
                    "mfre_kinetic:cogwheel" => draw_cog(draw, axis, false),
                    "mfre_kinetic:large_cogwheel" => draw_cog(draw, axis, true),
                    "mfre_kinetic:hand_crank" => draw_crank(draw, axis),
                    _ => draw_shaft(draw, axis),
                }
            }
        }
        draw.keep();
    }
}

impl BlockEntityPartState for Kinetic {
    type Data = KineticData;
    const DATA: &'static AttachmentType<KineticData> = &KINETIC;
}

/// Lee un entero del espejo nbt.
fn nbt_int(nbt: &Compound, key: &str) -> Option<i32> {
    match nbt.get(&key.into()) {
        Some(Tag::Int(n)) => Some(*n),
        _ => None,
    }
}

/// Lee la velocidad del espejo nbt (bits IEEE en `Tag::Float`).
fn nbt_speed(nbt: &Compound) -> Option<f32> {
    match nbt.get(&NBT_SPEED.into()) {
        Some(Tag::Float(bits)) => Some(f32::from_bits(*bits)),
        _ => None,
    }
}

/// Sin tinta: 0xFFFFFF.
const TINT: u32 = 0xFF_FFFF;

/// La caja de un bloque cilíndrico a lo largo del `axis`: diámetro `lo..hi`, grosor 5..11.
fn cylinder(axis: Axis, lo: f32, hi: f32) -> ([f32; 3], [f32; 3]) {
    match axis {
        Axis::X => ([5.0, lo, lo], [11.0, hi, hi]),
        Axis::Y => ([lo, 5.0, lo], [hi, 11.0, hi]),
        Axis::Z => ([lo, lo, 5.0], [hi, hi, 11.0]),
    }
}

/// Las caras de un cilindro a lo largo del `axis`: `end` en las tapas, `side` alrededor.
/// Orden de caras: down, up, north, south, west, east.
fn cylinder_faces<'a>(
    axis: Axis,
    side: Option<CubeFace<'a>>,
    end: Option<CubeFace<'a>>,
) -> [Option<CubeFace<'a>>; 6] {
    match axis {
        Axis::X => [side, side, side, side, end, end],
        Axis::Y => [end, end, side, side, side, side],
        Axis::Z => [side, side, end, end, side, side],
    }
}

/// La dirección de referencia perpendicular al axis (hacia dónde apunta una aguja/manivela
/// en reposo): +Y para los axes X y Z, +X para el axis Y.
fn reference_dir(axis: Axis) -> [f32; 3] {
    match axis {
        Axis::Y => [1.0, 0.0, 0.0],
        _ => [0.0, 1.0, 0.0],
    }
}

/// Dibuja el eje: brazo pasante de 4/16 con el cubo de 10/16 en el centro, todo girando
/// (lo llama `block_entity_submit` bajo el `spin` del eje del bloque).
fn draw_shaft(draw: &mut dyn BlockEntityDrawContext, axis: Axis) {
    let arm = Some(CubeFace::new("mfre_kinetic:block/shaft_arm"));
    let hub = Some(CubeFace::new("mfre_kinetic:block/shaft_hub"));
    let faces = cylinder_faces(axis, arm, hub);
    let (from, to) = match axis {
        Axis::X => ([0.0, 6.0, 6.0], [16.0, 10.0, 10.0]),
        Axis::Y => ([6.0, 0.0, 6.0], [10.0, 16.0, 10.0]),
        Axis::Z => ([6.0, 6.0, 0.0], [10.0, 10.0, 16.0]),
    };
    draw.cube(from, to, faces, TINT);
    // El cubo central (acoplamiento), íntegro de hub.
    draw.cube([5.0; 3], [11.0; 3], [hub; 6], TINT);
}

/// Dibuja la rueda hidráulica: un cilindro de 10/16 de diámetro a lo largo del eje, con
/// las tapas de acero y la banda de paletas ember girando.
fn draw_wheel(draw: &mut dyn BlockEntityDrawContext, axis: Axis) {
    let side = Some(CubeFace::new("mfre_kinetic:block/water_wheel_side"));
    let end = Some(CubeFace::new("mfre_kinetic:block/water_wheel_end"));
    let faces = cylinder_faces(axis, side, end);
    let (from, to) = match axis {
        Axis::X => ([0.0, 3.0, 3.0], [16.0, 13.0, 13.0]),
        Axis::Y => ([3.0, 0.0, 3.0], [13.0, 16.0, 13.0]),
        Axis::Z => ([3.0, 3.0, 0.0], [13.0, 13.0, 16.0]),
    };
    draw.cube(from, to, faces, TINT);
}

/// Dibuja un engranaje: un disco dentado a lo largo del eje — el chico de 12/16 de
/// diámetro y 6/16 de grosor, el grande ocupa los 16/16. Las tapas muestran el buje, los
/// lados los dientes.
fn draw_cog(draw: &mut dyn BlockEntityDrawContext, axis: Axis, large: bool) {
    let side = Some(CubeFace::new(if large {
        "mfre_kinetic:block/large_cogwheel_side"
    } else {
        "mfre_kinetic:block/cogwheel_side"
    }));
    let end = Some(CubeFace::new(if large {
        "mfre_kinetic:block/large_cogwheel_end"
    } else {
        "mfre_kinetic:block/cogwheel_end"
    }));
    let faces = cylinder_faces(axis, side, end);
    // Chico: 12/16 de diámetro (2..14); grande: 16/16. Grosor 6/16 (5..11).
    let (lo, hi) = if large { (0.0, 16.0) } else { (2.0, 14.0) };
    let (from, to) = cylinder(axis, lo, hi);
    draw.cube(from, to, faces, TINT);
}

/// Dibuja la manivela: un buje de acero con un brazo y su empuñadura ember hacia la
/// dirección de referencia; todo gira bajo el `spin` cuando está activa.
fn draw_crank(draw: &mut dyn BlockEntityDrawContext, axis: Axis) {
    let base = Some(CubeFace::new("mfre_kinetic:block/hand_crank_base"));
    let handle = Some(CubeFace::new("mfre_kinetic:block/hand_crank_handle"));
    // El buje central.
    draw.cube([5.0; 3], [11.0; 3], [base; 6], TINT);
    // El brazo desde el buje hacia la dirección de referencia, y la empuñadura al final.
    let dir = reference_dir(axis);
    let arm_from = [8.0 - dir[0], 8.0 - dir[1], 8.0 - dir[2]];
    let arm_to = [
        8.0 + dir[0] * 6.0 + 1.0 * (1.0 - dir[0]),
        8.0 + dir[1] * 6.0 + 1.0 * (1.0 - dir[1]),
        8.0 + dir[2] * 6.0 + 1.0 * (1.0 - dir[2]),
    ];
    let arm_from = [
        arm_from[0] - 1.0 * (1.0 - dir[0]),
        arm_from[1] - 1.0 * (1.0 - dir[1]),
        arm_from[2] - 1.0 * (1.0 - dir[2]),
    ];
    draw.cube(arm_from, arm_to, [base; 6], TINT);
    let knob_from = [
        arm_to[0] - 2.0 * dir[0] - 2.0 * (1.0 - dir[0]),
        arm_to[1] - 2.0 * dir[1] - 2.0 * (1.0 - dir[1]),
        arm_to[2] - 2.0 * dir[2] - 2.0 * (1.0 - dir[2]),
    ];
    draw.cube(knob_from, arm_to, [handle; 6], TINT);
}

/// Dibuja un medidor: un disco estático con el dial en las tapas y una aguja delgada
/// rotada `degrees` alrededor del axis (push/rotate_about/pop para no mover el disco).
/// `tint` tiñe la aguja (roja en sobrecarga del stressometer).
fn draw_meter(
    draw: &mut dyn BlockEntityDrawContext,
    axis: Axis,
    degrees: f32,
    _is_stress: bool,
    tint: u32,
    dial: &str,
) {
    let texture = format!("mfre_kinetic:block/{dial}");
    let side = Some(CubeFace::new("mfre_kinetic:block/meter_side"));
    let end = Some(CubeFace::new(&texture));
    let faces = cylinder_faces(axis, side, end);
    // El cuerpo: disco de 12/16 de diámetro.
    let (from, to) = cylinder(axis, 2.0, 14.0);
    draw.cube(from, to, faces, TINT);
    // La aguja: una vara delgada del centro hacia la dirección de referencia, rotada.
    let dir = reference_dir(axis);
    draw.push();
    draw.rotate_about(axis.vector(), degrees, [0.5, 0.5, 0.5]);
    let needle = Some(CubeFace::new("mfre_kinetic:block/meter_needle"));
    let from = [
        8.0 - dir[0] - 0.5 * (1.0 - dir[0]),
        8.0 - dir[1] - 0.5 * (1.0 - dir[1]),
        8.0 - dir[2] - 0.5 * (1.0 - dir[2]),
    ];
    let to = [
        8.0 + dir[0] * 5.0 + 0.5 * (1.0 - dir[0]),
        8.0 + dir[1] * 5.0 + 0.5 * (1.0 - dir[1]),
        8.0 + dir[2] * 5.0 + 0.5 * (1.0 - dir[2]),
    ];
    draw.cube(from, to, [needle; 6], tint);
    draw.pop();
}

/// El adaptador de sólo lectura del resolvedor sobre el nivel real.
struct LevelWorld<'h> {
    host: &'h dyn Host,
    level: LevelRef,
}

impl KineticWorld for LevelWorld<'_> {
    fn node(&self, pos: [i32; 3]) -> Option<Node> {
        let shaft = SHAFT.get().copied()?;
        let wheel = WHEEL.get().copied()?;
        let cog = COG.get().copied()?;
        let large_cog = LARGE_COG.get().copied()?;
        let crank = CRANK.get().copied()?;
        let speedometer = SPEEDOMETER.get().copied()?;
        let stressometer = STRESSOMETER.get().copied()?;
        let block = self.level.block(self.host, pos)?;
        let kind = if block == shaft {
            Kind::Shaft
        } else if block == wheel {
            Kind::Wheel
        } else if block == cog {
            Kind::Cog
        } else if block == large_cog {
            Kind::LargeCog
        } else if block == crank {
            Kind::HandCrank
        } else if block == speedometer {
            Kind::Speedometer
        } else if block == stressometer {
            Kind::Stressometer
        } else {
            return None;
        };
        let axis = Axis::parse(&self.level.block_property(self.host, pos, "axis")?)?;
        // Las fuentes: la rueda con agua perpendicular, la manivela con cuenta activa.
        let (source_speed, capacity) = match kind {
            Kind::Wheel if self.has_water(pos, axis) => (WHEEL_RPM, WHEEL_CAPACITY),
            Kind::HandCrank if self.crank_active(pos) => {
                let dir = self.crank_dir(pos);
                (CRANK_RPM * dir, CRANK_CAPACITY)
            }
            _ => (0.0, 0),
        };
        Some(Node {
            axis,
            kind,
            source_speed,
            capacity,
        })
    }
}

impl LevelWorld<'_> {
    /// Una rueda hidráulica tiene agua en alguno de sus cuatro vecinos perpendiculares.
    fn has_water(&self, pos: [i32; 3], axis: Axis) -> bool {
        let Some(water) = WATER.get().copied() else {
            return false;
        };
        axis.perpendicular().iter().any(|offset| {
            self.level
                .block(self.host, add(pos, *offset))
                .is_some_and(|block| block == water)
        })
    }

    /// El estado guardado de la manivela en `pos` (su attachment).
    fn crank_data(&self, pos: [i32; 3]) -> Option<KineticData> {
        let entity = self.host.block_entity_identity(self.level, pos)?;
        Kinetic::state(self.host, entity).ok()
    }

    /// ¿La manivela está activa (le quedan ticks de giro)?
    fn crank_active(&self, pos: [i32; 3]) -> bool {
        self.crank_data(pos).is_some_and(|data| data.crank > 0)
    }

    /// El sentido del giro activo de la manivela (+1/-1 como f32).
    fn crank_dir(&self, pos: [i32; 3]) -> f32 {
        match self.crank_data(pos) {
            Some(data) if data.dir < 0 => -1.0,
            _ => 1.0,
        }
    }
}
