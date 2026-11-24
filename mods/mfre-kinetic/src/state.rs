//! El estado cinético sobre el nivel real: el attachment persistido, el Part del block
//! entity y el adaptador de sólo lectura que el resolvedor puro (`kinetic`) usa.
use crate::kinetic::{self, Axis, KineticWorld, Node, add};
use mod_api::*;
use std::sync::OnceLock;

/// Claves de registro fijadas una vez en `Hooks::register`.
pub static SHAFT: OnceLock<BlockKey> = OnceLock::new();
pub static WHEEL: OnceLock<BlockKey> = OnceLock::new();
pub static WATER: OnceLock<BlockKey> = OnceLock::new();

/// Las RPM de una rueda hidráulica con agua (dirección y caudal: incrementos siguientes).
pub const WHEEL_RPM: i32 = 8;

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

/// El Part de los bloques cinéticos: los hace ticar y mantiene la red resuelta.
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

    /// Un tick de servidor: resuelve la red desde este nodo y, si su velocidad o sus
    /// conexiones cambiaron frente a lo guardado, reescribe la red completa en una
    /// transacción. Cada nodo detecta así los cambios que le tocan (colocar/romper un
    /// vecino, agua que llega o se va de una rueda) y la red converge en un tick.
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
        if let Some(entity) = host.block_entity_identity(level, event.pos) {
            if Kinetic::state(host, entity).unwrap_or_default() == expected {
                return;
            }
        }
        // Algo cambió: la red se reescribe y el tick queda `Changed` (su valor por defecto).
        *event.reply = BlockEntityTickReply::Changed;
        let _ = host.transaction(|host, _| {
            for member in &members {
                let Some(level) = host.level(host.dimension()) else {
                    continue;
                };
                let Some(target) = host.block_entity_identity(level, member.pos) else {
                    continue;
                };
                let _ = edit_part_state(host, target, &KINETIC, |_, data| {
                    data.conns = member.conns;
                    data.speed = member.speed;
                });
            }
            Ok::<(), ()>(())
        });
    }
}

impl BlockEntityPartState for Kinetic {
    type Data = KineticData;
    const DATA: &'static AttachmentType<KineticData> = &KINETIC;
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
