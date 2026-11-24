//! El estado cinético sobre el nivel real: el attachment persistido, el Part del block
//! entity (tick de servidor, tag de sync y render giratorio de cliente) y el adaptador de
//! sólo lectura que el resolvedor puro (`kinetic`) usa.
use crate::kinetic::{self, Axis, KineticWorld, Node, add, degrees_per_tick};
use mod_api::*;
use std::sync::OnceLock;

/// Claves de registro fijadas una vez en `Hooks::register`.
pub static SHAFT: OnceLock<BlockKey> = OnceLock::new();
pub static WHEEL: OnceLock<BlockKey> = OnceLock::new();
pub static WATER: OnceLock<BlockKey> = OnceLock::new();

/// Las RPM de una rueda hidráulica con agua (caudal y más fuentes: incrementos siguientes).
pub const WHEEL_RPM: i32 = 8;

/// Claves del espejo en el nbt guardado: lo que `block_entity_update_tag` envía al cliente
/// y el render lee. El attachment sigue siendo la verdad del servidor; el espejo existe
/// porque `update_tag` no recibe host y no puede leer el attachment.
pub const NBT_SPEED: &str = "mfre_speed";
pub const NBT_CONNS: &str = "mfre_conns";

/// El estado cinético que cada nodo guarda: sus conexiones y su velocidad. Se persiste
/// con el chunk a través del códec del attachment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Wire)]
pub struct KineticData {
    /// El bitmask de conexiones de la última resolución.
    pub conns: u8,
    /// La velocidad con signo (RPM); 0 = detenida.
    pub speed: i32,
}

/// El attachment por block entity cinético (`BlockEntityPartState::DATA`).
pub static KINETIC: AttachmentType<KineticData> = AttachmentType {
    namespace: "mfre_kinetic",
    name: "kinetic",
    default: || KineticData { conns: 0, speed: 0 },
    codec: Some(AttachmentCodec {
        encode: |data| {
            Ok([
                ("c".into(), Tag::Int(i32::from(data.conns))),
                ("s".into(), Tag::Int(data.speed)),
            ]
            .into())
        },
        decode: |saved: &Compound| {
            let conns = match saved.get(&"c".into()) {
                Some(Tag::Int(n)) if (0..=255).contains(n) => *n as u8,
                _ => return Err(AttachmentError::Decode),
            };
            let speed = match saved.get(&"s".into()) {
                Some(Tag::Int(n)) => *n,
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

#[mod_api::horizon_part]
impl BlockPart for Kinetic {}

#[mod_api::horizon_part]
impl BlockEntityPart for Kinetic {
    /// Los block entities cinéticos tican en el servidor (`EntityBlock.getTicker`);
    /// nunca limpia un valor que otro mod o mixin fijó.
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == "mfre_kinetic:shaft" || event.block == "mfre_kinetic:water_wheel" {
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
        let mirror_ok = nbt_int(&event.entity.nbt, NBT_SPEED) == Some(expected.speed)
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
                .insert(NBT_SPEED.into(), Tag::Int(expected.speed));
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
        let speed = nbt_int(&event.entity.nbt, NBT_SPEED).unwrap_or(0);
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
        if event.block == "mfre_kinetic:water_wheel" {
            draw_wheel(draw, axis);
        } else {
            draw_shaft(draw, axis);
        }
        draw.keep();
    }
}

impl BlockEntityPartState for Kinetic {
    type Data = KineticData;
    const DATA: &'static AttachmentType<KineticData> = &KINETIC;
}

/// Lee un entero del nbt (el espejo y el tag del cliente usan `Tag::Int`).
fn nbt_int(nbt: &Compound, key: &str) -> Option<i32> {
    match nbt.get(&key.into()) {
        Some(Tag::Int(n)) => Some(*n),
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
    // El brazo: los extremos son tapa de cubo (hub), los lados son estrías (arm).
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
    // orden: down, up, north, south, west, east
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

/// El adaptador de sólo lectura del resolvedor sobre el nivel real.
struct LevelWorld<'h> {
    host: &'h dyn Host,
    level: LevelRef,
}

impl KineticWorld for LevelWorld<'_> {
    fn node(&self, pos: [i32; 3]) -> Option<Node> {
        let shaft = SHAFT.get().copied()?;
        let wheel = WHEEL.get().copied()?;
        let block = self.level.block(self.host, pos)?;
        let is_wheel = block == wheel;
        if !is_wheel && block != shaft {
            return None;
        }
        let axis = Axis::parse(&self.level.block_property(self.host, pos, "axis")?)?;
        let source_speed = if is_wheel && self.has_water(pos, axis) {
            WHEEL_RPM
        } else {
            0
        };
        Some(Node { axis, source_speed })
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
