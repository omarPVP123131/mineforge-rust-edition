//! La red rotacional pura: nodos, conexiones, BFS y velocidad — sin tipos de `mod_api`.
//! El adaptador sobre el nivel real vive en `state.rs`; este módulo se prueba con
//! `cargo test` sin juego (validación sandbox de F42).
//!
//! Reglas de conexión (limpias, inspiradas en Create — port clean-room D21):
//! - **Axial** (a lo largo del eje compartido): el eje (Shaft) conecta con cualquier
//!   cinético del mismo axis pegado a sus extremos (eje↔eje, eje↔rueda, eje↔engranaje).
//!   Misma velocidad, mismo sentido. Entre dos no-ejes no hay conexión axial.
//! - **Malla** (engranajes): dos engranajes (Cog/LargeCog) vecinos meshan cuando el
//!   desplazamiento es perpendicular a AMBOS axes (incluye engranajes coplanares del
//!   mismo axis en fila). Cada malla invierte el sentido y aplica la relación de
//!   tamaños: grande→chico ×2, chico→grande ×0.5, iguales ×1.
//!
//! La velocidad la propagan las fuentes (ruedas con agua) por BFS multi-fuente en orden
//! de posición (x,y,z): cada miembro toma la velocidad de la fuente prioritaria que lo
//! alcanza primero; sin fuente, 0. Las relaciones son potencias de 2: exactas en f32.

use std::collections::VecDeque;

/// El eje de un bloque cinético: su propiedad de estado `axis`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    /// El valor de la propiedad de estado.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "x" => Some(Self::X),
            "y" => Some(Self::Y),
            "z" => Some(Self::Z),
            _ => None,
        }
    }

    /// El vector unidad del eje.
    pub fn delta(self) -> [i32; 3] {
        match self {
            Self::X => [1, 0, 0],
            Self::Y => [0, 1, 0],
            Self::Z => [0, 0, 1],
        }
    }

    /// Las cuatro direcciones perpendiculares (vecinos de malla y de agua).
    pub fn perpendicular(self) -> [[i32; 3]; 4] {
        match self {
            Self::X => [[0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]],
            Self::Y => [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]],
            Self::Z => [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0]],
        }
    }

    fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Z => 2,
        }
    }

    /// El vector unidad del eje en f32, para `draw.spin`/`draw.rotate_about`.
    pub fn vector(self) -> [f32; 3] {
        match self {
            Self::X => [1.0, 0.0, 0.0],
            Self::Y => [0.0, 1.0, 0.0],
            Self::Z => [0.0, 0.0, 1.0],
        }
    }
}

/// El tipo de bloque cinético, para las reglas de conexión y las relaciones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Eje: transmisión recta; conector axial universal.
    Shaft,
    /// Rueda hidráulica: fuente cuando tiene agua perpendicular.
    Wheel,
    /// Engranaje chico: malla con otros engranajes.
    Cog,
    /// Engranaje grande: malla como el chico, con relación 2:1.
    LargeCog,
}

impl Kind {
    /// Si mesha con otros engranajes.
    fn is_gear(self) -> bool {
        matches!(self, Self::Cog | Self::LargeCog)
    }

    fn is_large(self) -> bool {
        matches!(self, Self::LargeCog)
    }
}

/// Suma de posiciones.
pub fn add(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// Resta de posiciones (el desplazamiento de `a` a `b`).
pub fn sub(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// El bit de conexión axial en la dirección del eje: positiva `1 << índice`,
/// negativa `8 << índice`. Las mallas de engranajes no usan bits (MVP).
pub fn conn_bit(axis: Axis, positive: bool) -> u8 {
    let base = 1u8 << axis.index();
    if positive { base } else { base << 3 }
}

/// Un nodo cinético como lo ve la red.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node {
    /// Su eje de transmisión.
    pub axis: Axis,
    /// Su tipo (reglas de conexión y relación).
    pub kind: Kind,
    /// Las RPM que aporta como fuente; 0 si no es fuente o su fuente está inactiva.
    pub source_speed: f32,
}

/// La lectura de sólo que el resolvedor necesita; el nivel real la implementa en `state.rs`.
pub trait KineticWorld {
    /// El nodo cinético en `pos`, o `None` si no hay un bloque cinético ahí.
    fn node(&self, pos: [i32; 3]) -> Option<Node>;
}

/// Un miembro de la red ya resuelta.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Member {
    /// Su posición.
    pub pos: [i32; 3],
    /// SU velocidad con signo (RPM alrededor de su propio axis); 0 = detenido.
    pub speed: f32,
    /// Su bitmask de conexiones axiales al resolver.
    pub conns: u8,
}

/// ¿Dos nodos vecinos se conectan? `offset` es el desplazamiento de `a` a `b`.
/// Devuelve también si la conexión es axial (para los bits de `conns`).
fn connects(a: Node, b: Node, offset: [i32; 3]) -> Option<bool> {
    // Axial: mismo axis, desplazamiento a lo largo del axis, y al menos un eje (shaft).
    if a.axis == b.axis {
        let d = a.axis.delta();
        if offset == d || offset == [-d[0], -d[1], -d[2]] {
            if a.kind == Kind::Shaft || b.kind == Kind::Shaft {
                return Some(true);
            }
        }
    }
    // Malla: ambos engranajes y el desplazamiento perpendicular a ambos axes.
    if a.kind.is_gear()
        && b.kind.is_gear()
        && perpendicular_to(a.axis, offset)
        && perpendicular_to(b.axis, offset)
    {
        return Some(false);
    }
    None
}

/// ¿El desplazamiento es perpendicular al axis (y no nulo)?
fn perpendicular_to(axis: Axis, offset: [i32; 3]) -> bool {
    let d = axis.delta();
    let dot = offset[0] * d[0] + offset[1] * d[1] + offset[2] * d[2];
    dot == 0 && offset != [0, 0, 0]
}

/// La relación de velocidad al pasar de `from` a `to` por una malla: grande→chico ×2,
/// chico→grande ×0.5, iguales ×1. La malla invierte el sentido (el signo va aparte).
fn mesh_ratio(from: Node, to: Node) -> f32 {
    match (from.kind.is_large(), to.kind.is_large()) {
        (true, false) => 2.0,
        (false, true) => 0.5,
        _ => 1.0,
    }
}

/// Los 6 vecinos cara a cara, con su desplazamiento.
const NEIGHBORS: [[i32; 3]; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];

/// Resuelve la red que contiene `start`: descubre el componente por BFS (aristas axiales
/// y mallas), propaga las velocidades desde las fuentes en orden de posición (x,y,z) y
/// devuelve los miembros ordenados por posición (determinista para pruebas y diffs).
/// `None` sólo si `start` no es un nodo.
pub fn solve<W: KineticWorld>(world: &W, start: [i32; 3]) -> Option<Vec<Member>> {
    world.node(start)?;
    // 1) El componente: BFS por todas las aristas; guarda nodos y bits axiales.
    let mut nodes: Vec<([i32; 3], Node, u8)> = Vec::new();
    let mut visited: Vec<[i32; 3]> = vec![start];
    let mut queue: VecDeque<[i32; 3]> = VecDeque::from([start]);
    while let Some(pos) = queue.pop_front() {
        let Some(node) = world.node(pos) else {
            continue;
        };
        let mut conns = 0u8;
        for offset in NEIGHBORS {
            let next = add(pos, offset);
            let Some(other) = world.node(next) else {
                continue;
            };
            let Some(axial) = connects(node, other, offset) else {
                continue;
            };
            if axial && node.axis == other.axis {
                let d = node.axis.delta();
                conns |= conn_bit(node.axis, offset == d);
            }
            if !visited.contains(&next) {
                visited.push(next);
                queue.push_back(next);
            }
        }
        nodes.push((pos, node, conns));
    }
    // 2) Las velocidades: BFS multi-fuente en orden de posición; cada miembro toma la
    //    primera asignación (la fuente prioritaria que lo alcanza por el camino más corto).
    let mut speeds: Vec<([i32; 3], f32)> = Vec::new();
    let mut sources: Vec<([i32; 3], Node)> = nodes
        .iter()
        .filter(|(_, node, _)| node.source_speed != 0.0)
        .map(|(pos, node, _)| (*pos, *node))
        .collect();
    sources.sort_by_key(|(pos, _)| (pos[0], pos[1], pos[2]));
    for (source_pos, source_node) in sources {
        if speeds.iter().any(|(pos, _)| *pos == source_pos) {
            continue; // ya la alcanzó otra fuente prioritaria
        }
        let mut queue: VecDeque<([i32; 3], Node, f32)> =
            VecDeque::from([(source_pos, source_node, source_node.source_speed)]);
        speeds.push((source_pos, source_node.source_speed));
        while let Some((pos, node, speed)) = queue.pop_front() {
            for offset in NEIGHBORS {
                let next = add(pos, offset);
                if speeds.iter().any(|(p, _)| *p == next) {
                    continue;
                }
                let Some(other) = world.node(next) else {
                    continue;
                };
                let Some(axial) = connects(node, other, offset) else {
                    continue;
                };
                // La vecina debe ser del componente (lo es: conecta con un miembro).
                let next_speed = if axial {
                    speed
                } else {
                    -speed * mesh_ratio(node, other)
                };
                speeds.push((next, next_speed));
                queue.push_back((next, other, next_speed));
            }
        }
    }
    // 3) Los miembros ordenados por posición, con su velocidad (0 sin fuente que alcance).
    nodes.sort_by_key(|(pos, _, _)| (pos[0], pos[1], pos[2]));
    Some(
        nodes
            .into_iter()
            .map(|(pos, _, conns)| Member {
                pos,
                speed: speeds
                    .iter()
                    .find(|(p, _)| *p == pos)
                    .map_or(0.0, |(_, s)| *s),
                conns,
            })
            .collect(),
    )
}

/// Los grados por tick de un giro a `speed` RPM: 1 RPM = 360° por 1 200 ticks = 0.3°/tick
/// (20 ticks por segundo). El signo es la dirección (negativo = al revés).
pub fn degrees_per_tick(speed: f32) -> f32 {
    speed * 0.3
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MapWorld(HashMap<[i32; 3], Node>);

    impl KineticWorld for MapWorld {
        fn node(&self, pos: [i32; 3]) -> Option<Node> {
            self.0.get(&pos).copied()
        }
    }

    fn shaft(axis: Axis) -> Node {
        Node {
            axis,
            kind: Kind::Shaft,
            source_speed: 0.0,
        }
    }

    fn wheel(axis: Axis, rpm: f32) -> Node {
        Node {
            axis,
            kind: Kind::Wheel,
            source_speed: rpm,
        }
    }

    fn cog(axis: Axis) -> Node {
        Node {
            axis,
            kind: Kind::Cog,
            source_speed: 0.0,
        }
    }

    fn large_cog(axis: Axis) -> Node {
        Node {
            axis,
            kind: Kind::LargeCog,
            source_speed: 0.0,
        }
    }

    fn member_at(members: &[Member], pos: [i32; 3]) -> Member {
        *members.iter().find(|m| m.pos == pos).expect("miembro")
    }

    #[test]
    fn line_propagates_speed_and_conns() {
        let mut map = HashMap::new();
        map.insert([0, 0, 0], wheel(Axis::X, 8.0));
        map.insert([1, 0, 0], shaft(Axis::X));
        map.insert([2, 0, 0], shaft(Axis::X));
        let members = solve(&MapWorld(map.clone()), [1, 0, 0]).unwrap();
        assert_eq!(members.len(), 3);
        assert!(members.iter().all(|m| m.speed == 8.0));
        assert_eq!(
            member_at(&members, [0, 0, 0]).conns,
            conn_bit(Axis::X, true)
        );
        assert_eq!(
            member_at(&members, [1, 0, 0]).conns,
            conn_bit(Axis::X, true) | conn_bit(Axis::X, false)
        );
        assert_eq!(
            member_at(&members, [2, 0, 0]).conns,
            conn_bit(Axis::X, false)
        );
    }

    #[test]
    fn mismatched_axis_does_not_connect() {
        let mut map = HashMap::new();
        map.insert([0, 0, 0], wheel(Axis::X, 8.0));
        map.insert([1, 0, 0], shaft(Axis::Y));
        let members = solve(&MapWorld(map.clone()), [0, 0, 0]).unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].conns, 0);
        let other = solve(&MapWorld(map), [1, 0, 0]).unwrap();
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].speed, 0.0);
    }

    #[test]
    fn no_source_gives_zero() {
        let mut map = HashMap::new();
        map.insert([0, 0, 0], shaft(Axis::Z));
        map.insert([0, 0, 1], shaft(Axis::Z));
        let members = solve(&MapWorld(map), [0, 0, 0]).unwrap();
        assert_eq!(members.len(), 2);
        assert!(members.iter().all(|m| m.speed == 0.0));
    }

    #[test]
    fn gap_splits_network() {
        let mut map = HashMap::new();
        map.insert([0, 0, 0], wheel(Axis::X, 8.0));
        map.insert([1, 0, 0], shaft(Axis::X));
        // hueco en [2,0,0]
        map.insert([3, 0, 0], shaft(Axis::X));
        let left = solve(&MapWorld(map.clone()), [1, 0, 0]).unwrap();
        assert_eq!(left.len(), 2);
        assert!(left.iter().all(|m| m.speed == 8.0));
        let right = solve(&MapWorld(map), [3, 0, 0]).unwrap();
        assert_eq!(right.len(), 1);
        assert_eq!(right[0].speed, 0.0);
    }

    #[test]
    fn lowest_source_wins() {
        let mut map = HashMap::new();
        map.insert([0, 0, 0], wheel(Axis::X, -8.0));
        map.insert([1, 0, 0], shaft(Axis::X));
        map.insert([2, 0, 0], wheel(Axis::X, 8.0));
        let members = solve(&MapWorld(map), [1, 0, 0]).unwrap();
        assert!(members.iter().all(|m| m.speed == -8.0));
    }

    #[test]
    fn inactive_wheel_is_not_a_source() {
        let mut map = HashMap::new();
        map.insert([0, 0, 0], wheel(Axis::X, 0.0));
        map.insert([1, 0, 0], shaft(Axis::X));
        let members = solve(&MapWorld(map), [1, 0, 0]).unwrap();
        assert_eq!(members.len(), 2);
        assert!(members.iter().all(|m| m.speed == 0.0));
    }

    #[test]
    fn start_without_node_is_none() {
        assert!(solve(&MapWorld(HashMap::new()), [0, 0, 0]).is_none());
    }

    #[test]
    fn members_are_sorted_by_position() {
        let mut map = HashMap::new();
        map.insert([2, 0, 0], shaft(Axis::X));
        map.insert([0, 0, 0], shaft(Axis::X));
        map.insert([1, 0, 0], shaft(Axis::X));
        let members = solve(&MapWorld(map), [1, 0, 0]).unwrap();
        let positions: Vec<[i32; 3]> = members.iter().map(|m| m.pos).collect();
        assert_eq!(positions, vec![[0, 0, 0], [1, 0, 0], [2, 0, 0]]);
    }

    #[test]
    fn rpm_to_degrees_per_tick() {
        assert!((degrees_per_tick(8.0) - 2.4).abs() < 1e-6);
        assert!((degrees_per_tick(-8.0) + 2.4).abs() < 1e-6);
        assert_eq!(degrees_per_tick(0.0), 0.0);
    }

    #[test]
    fn axis_vectors() {
        assert_eq!(Axis::X.vector(), [1.0, 0.0, 0.0]);
        assert_eq!(Axis::Y.vector(), [0.0, 1.0, 0.0]);
        assert_eq!(Axis::Z.vector(), [0.0, 0.0, 1.0]);
    }

    // ---- engranajes (incremento 4) ----

    #[test]
    fn shaft_feeds_cog_coaxially() {
        // rueda X → eje X → engranaje X (coaxial, misma velocidad)
        let mut map = HashMap::new();
        map.insert([0, 0, 0], wheel(Axis::X, 8.0));
        map.insert([1, 0, 0], shaft(Axis::X));
        map.insert([2, 0, 0], cog(Axis::X));
        let members = solve(&MapWorld(map), [1, 0, 0]).unwrap();
        assert_eq!(members.len(), 3);
        assert_eq!(member_at(&members, [2, 0, 0]).speed, 8.0);
        // la conexión coaxial con el eje sí cuenta como axial
        assert_eq!(
            member_at(&members, [2, 0, 0]).conns,
            conn_bit(Axis::X, false)
        );
    }

    #[test]
    fn cog_mesh_reverses() {
        // eje Y alimenta un engranaje Y; otro engranaje Y al este (offset X ⊥ Y) meshan
        let mut map = HashMap::new();
        map.insert([0, 0, 0], wheel(Axis::Y, 8.0));
        map.insert([0, 1, 0], shaft(Axis::Y));
        map.insert([0, 2, 0], cog(Axis::Y));
        map.insert([1, 2, 0], cog(Axis::Y));
        let members = solve(&MapWorld(map), [0, 1, 0]).unwrap();
        assert_eq!(members.len(), 4);
        assert_eq!(member_at(&members, [0, 2, 0]).speed, 8.0);
        assert_eq!(member_at(&members, [1, 2, 0]).speed, -8.0);
    }

    #[test]
    fn large_to_small_doubles_and_reverses() {
        let mut map = HashMap::new();
        map.insert([0, 0, 0], wheel(Axis::Y, 8.0));
        map.insert([0, 1, 0], shaft(Axis::Y));
        map.insert([0, 2, 0], large_cog(Axis::Y));
        map.insert([1, 2, 0], cog(Axis::Y));
        let members = solve(&MapWorld(map), [0, 1, 0]).unwrap();
        assert_eq!(member_at(&members, [0, 2, 0]).speed, 8.0);
        assert_eq!(member_at(&members, [1, 2, 0]).speed, -16.0);
    }

    #[test]
    fn small_to_large_halves() {
        let mut map = HashMap::new();
        map.insert([0, 0, 0], wheel(Axis::Y, 8.0));
        map.insert([0, 1, 0], shaft(Axis::Y));
        map.insert([0, 2, 0], cog(Axis::Y));
        map.insert([1, 2, 0], large_cog(Axis::Y));
        let members = solve(&MapWorld(map), [0, 1, 0]).unwrap();
        assert_eq!(member_at(&members, [0, 2, 0]).speed, 8.0);
        assert_eq!(member_at(&members, [1, 2, 0]).speed, -4.0);
    }

    #[test]
    fn perpendicular_axes_mesh() {
        // engranaje X alimentado coaxialmente; un engranaje Y en +z (offset ⊥ a X y a Y)
        let mut map = HashMap::new();
        map.insert([0, 0, 0], wheel(Axis::X, 8.0));
        map.insert([1, 0, 0], shaft(Axis::X));
        map.insert([2, 0, 0], cog(Axis::X));
        map.insert([2, 0, 1], cog(Axis::Y));
        let members = solve(&MapWorld(map), [1, 0, 0]).unwrap();
        assert_eq!(members.len(), 4);
        assert_eq!(member_at(&members, [2, 0, 0]).speed, 8.0);
        assert_eq!(member_at(&members, [2, 0, 1]).speed, -8.0);
    }

    #[test]
    fn offset_parallel_to_axis_does_not_mesh() {
        // dos engranajes Y uno encima del otro: offset Y ∥ axis → no meshan (y sin eje entre
        // ellos, tampoco axial) → componentes separados
        let mut map = HashMap::new();
        map.insert([0, 0, 0], cog(Axis::Y));
        map.insert([0, 1, 0], cog(Axis::Y));
        let members = solve(&MapWorld(map), [0, 0, 0]).unwrap();
        assert_eq!(members.len(), 1);
    }

    #[test]
    fn gear_train_alternates_sign() {
        // fuente vertical (rueda Y + eje Y) y fila de engranajes Y a lo largo de X:
        // 8, -8, 8 (la malla alterna el sentido)
        let mut map = HashMap::new();
        map.insert([0, 0, 0], wheel(Axis::Y, 8.0));
        map.insert([0, 1, 0], shaft(Axis::Y));
        map.insert([0, 2, 0], cog(Axis::Y));
        map.insert([1, 2, 0], cog(Axis::Y));
        map.insert([2, 2, 0], cog(Axis::Y));
        let members = solve(&MapWorld(map), [0, 1, 0]).unwrap();
        assert_eq!(member_at(&members, [0, 2, 0]).speed, 8.0);
        assert_eq!(member_at(&members, [1, 2, 0]).speed, -8.0);
        assert_eq!(member_at(&members, [2, 2, 0]).speed, 8.0);
    }

    #[test]
    fn wheel_does_not_feed_cog_without_shaft() {
        // rueda X y engranaje X pegados: sin eje entre ellos no hay conexión axial
        let mut map = HashMap::new();
        map.insert([0, 0, 0], wheel(Axis::X, 8.0));
        map.insert([1, 0, 0], cog(Axis::X));
        let members = solve(&MapWorld(map), [0, 0, 0]).unwrap();
        assert_eq!(members.len(), 1);
    }
}
