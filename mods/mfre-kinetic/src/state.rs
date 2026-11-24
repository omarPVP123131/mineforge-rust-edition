//! El estado cinético sobre el nivel real: el attachment persistido, el Part del block
//! entity (tick de servidor, tag de sync y render giratorio de cliente) y el adaptador de
//! sólo lectura que el resolvedor puro (`kinetic`) usa.
use crate::kinetic::{self, Axis, Kind, KineticWorld, Node, add, degrees_per_tick};
use mod_api::*;
use std::sync::OnceLock;

/// Claves de registro fijadas una vez en `Hooks::register`.
pub static SHAFT: OnceLock<BlockKey> = OnceLock::new();
pub static WHEEL: OnceLock<BlockKey> = OnceLock::new();
pub static COG: OnceLock<BlockKey> = OnceLock::new();
pub static LARGE_COG: OnceLock<BlockKey> = OnceLock::new();
pub static WATER: OnceLock<BlockKey> = OnceLock::new();

/// Las RPM de una rueda hidráulica con agua (caudal y más fuentes: incrementos siguientes).
pub const WHEEL_RPM: f32 = 8.0;

/// Claves del espejo en el nbt guardado: lo que `block_entity_update_tag` envía al cliente
/// y el render lee. El attachment sigue siendo la verdad del servidor; el espejo existe
/// porque `update_tag` no recibe host y no puede leer el attachment. La velocidad viaja
/// como los bits IEEE de su f32 (`Tag::Float` los guarda así).
pub const NBT_SPEED: &str = "mfre_speed";
pub const NBT_CONNS: &str = "mfre_conns";

/// El estado cinético que cada nodo guarda: sus conexiones axiales y su velocidad. Se
/// persiste con el chunk a través del códec del attachment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Wire)]
pub struct KineticData {
    /// El bitmask de conexiones axiales de la última resolución.
    pub conns: u8,
    /// Su velocidad con signo (RPM alrededor de su axis); 0 = detenida.
    pub speed: f32,
}

/// El attachment por block entity cinético (`BlockEntityPartState::DATA`).
pub static KINETIC: AttachmentType<KineticData> = AttachmentType {
    namespace: "mfre_kinetic",
    name: "kinetic",
    default: || KineticData {
        conns: 0,
        speed: 0.0,
    },
    codec: Some(AttachmentCodec {
        encode: |data| {
            Ok([
                ("c".into(), Tag::Int(i32::from(data.conns))),
                ("s".into(), Tag::Float(data.speed.to_bits())),
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
            Ok(KineticData { conns, speed })
        },
    }),
    copy_on_death: false,
    sync: false,
};

/// El Part de los bloques cinéticos: los hace ticar, mantiene la red resuelta y la dibuja
/// girando en el cliente.
pub struct Kinetic;

/// Los ids de los bloques con block entity cinético.
pub const KINETIC_BLOCKS: [&str; 4] = [
    "mfre_kinetic:shaft",
    "mfre_kinetic:water_wheel",
    "mfre_kinetic:cogwheel",
    "mfre_kinetic:large_cogwheel",
];

#[mod_api::horizon_part]
impl BlockPart for Kinetic {}

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
    /// tick sin escrituras cruzadas ni transacciones entre nodos.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        // Sin cambio no se ensucia el chunk: `Unchanged` descarta el payload del tick.
        *event.reply = BlockEntityTickReply::Unchanged;
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        let members = {
            let world = LevelWorld {
                host: &*host,
                level,
            };
            match kinetic::solve(&world, event.pos) {
                Some(members) => members,
                None => return,
            }
        };
        let Some(me) = members.iter().find(|m| m.pos == event.pos).copied() else {
            return;
        };
        let expected = KineticData {
            conns: me.conns,
            speed: me.speed,
        };

        // ¿El attachment ya tiene lo esperado?
        let entity = host.block_entity_identity(level, event.pos);
        let stored_ok = match entity {
            Some(entity) => Kinetic::state(host, entity).unwrap_or_default() == expected,
            None => true, // sin identidad no hay attachment que corregir
        };
        // ¿El espejo nbt ya tiene lo esperado?
        let mirror_ok = nbt_speed(&event.entity.nbt) == Some(expected.speed)
            && nbt_int(&event.entity.nbt, NBT_CONNS) == Some(i32::from(expected.conns));
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
            // Envía el tag a quien ve el chunk (con el nbt espejo recién escrito); los
            // cambios hechos en el tick salen al inicio del tick siguiente, como en vanilla.
            host.block_entity_sync(event.pos);
        }
    }

    /// El tag que el cliente recibe al cargar el chunk o tras `block_entity_sync`: el
    /// espejo nbt (velocidad y conexiones). Sin host no se puede leer el attachment.
    fn block_entity_update_tag(
        &self,
        event: &mut BlockEntityUpdateTagEvent<'_>,
    ) -> Option<Compound> {
        let mut tag = Compound::new();
        for key in [NBT_SPEED, NBT_CONNS] {
            if let Some(value) = event.entity.nbt.get(&key.into()) {
                tag.insert(key.into(), value.clone());
            }
        }
        (!tag.is_empty()).then_some(tag)
    }

    /// El render de cliente: la geometría gira alrededor del eje del bloque a la velocidad
    /// del espejo nbt. `spin` la deja girando en el reloj de animación incluso con la
    /// geometría en caché (`keep`), que se rehace cuando el estado, el nbt o la luz cambian.
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
        draw.spin(axis.vector(), [0.5, 0.5, 0.5], degrees_per_tick(speed), 0.0);
        match event.block {
            "mfre_kinetic:water_wheel" => draw_wheel(draw, axis),
            "mfre_kinetic:cogwheel" => draw_cog(draw, axis, false),
            "mfre_kinetic:large_cogwheel" => draw_cog(draw, axis, true),
            _ => draw_shaft(draw, axis),
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

/// Dibuja el eje: brazo pasante de 4/16 con el cubo de 10/16 en el centro, todo girando
/// (lo llama `block_entity_submit` bajo el `spin` del eje del bloque).
fn draw_shaft(draw: &mut dyn BlockEntityDrawContext, axis: Axis) {
    let arm = Some(CubeFace::new("mfre_kinetic:block/shaft_arm"));
    let hub = Some(CubeFace::new("mfre_kinetic:block/shaft_hub"));
    // Orden de caras: down, up, north, south, west, east.
    let faces = match axis {
        Axis::X => [arm, arm, arm, arm, hub, hub],
        Axis::Y => [hub, hub, arm, arm, arm, arm],
        Axis::Z => [arm, arm, hub, hub, arm, arm],
    };
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
    // Orden de caras: down, up, north, south, west, east.
    let faces = match axis {
        Axis::X => [side, side, side, side, end, end],
        Axis::Y => [end, end, side, side, side, side],
        Axis::Z => [side, side, end, end, side, side],
    };
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
    // Orden de caras: down, up, north, south, west, east.
    let faces = match axis {
        Axis::X => [side, side, side, side, end, end],
        Axis::Y => [end, end, side, side, side, side],
        Axis::Z => [side, side, end, end, side, side],
    };
    // Chico: 12/16 de diámetro (2..14); grande: 16/16. Grosor 6/16 (5..11).
    let (lo, hi) = if large { (0.0, 16.0) } else { (2.0, 14.0) };
    let (from, to) = match axis {
        Axis::X => ([5.0, lo, lo], [11.0, hi, hi]),
        Axis::Y => ([lo, 5.0, lo], [hi, 11.0, hi]),
        Axis::Z => ([lo, lo, 5.0], [hi, hi, 11.0]),
    };
    draw.cube(from, to, faces, TINT);
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
        let block = self.level.block(self.host, pos)?;
        let kind = if block == shaft {
            Kind::Shaft
        } else if block == wheel {
            Kind::Wheel
        } else if block == cog {
            Kind::Cog
        } else if block == large_cog {
            Kind::LargeCog
        } else {
            return None;
        };
        let axis = Axis::parse(&self.level.block_property(self.host, pos, "axis")?)?;
        let source_speed = if kind == Kind::Wheel && self.has_water(pos, axis) {
            WHEEL_RPM
        } else {
            0.0
        };
        Some(Node {
            axis,
            kind,
            source_speed,
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
}
