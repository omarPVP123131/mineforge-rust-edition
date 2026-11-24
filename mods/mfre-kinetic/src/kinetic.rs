//! La red rotacional pura: nodos, conexiones, BFS y velocidad — sin tipos de `mod_api`.
//! El adaptador sobre el nivel real vive en `state.rs`; este módulo se prueba con
//! `cargo test` sin juego (validación sandbox del incremento 2 de F42).

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

    /// Las cuatro direcciones perpendiculares (donde la rueda hidráulica busca agua).
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
}

/// Suma de posiciones.
pub fn add(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// El bit de conexión en la dirección del eje: positiva `1 << índice`, negativa `8 << índice`.
pub fn conn_bit(axis: Axis, positive: bool) -> u8 {
    let base = 1u8 << axis.index();
    if positive { base } else { base << 3 }
}

/// Un nodo cinético como lo ve la red.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    /// Su eje de transmisión.
    pub axis: Axis,
    /// Las RPM que aporta como fuente; 0 si no es fuente o su fuente está inactiva.
    pub source_speed: i32,
}

/// La lectura de sólo que el resolvedor necesita; el nivel real la implementa en `state.rs`.
pub trait KineticWorld {
    /// El nodo cinético en `pos`, o `None` si no hay un bloque cinético ahí.
    fn node(&self, pos: [i32; 3]) -> Option<Node>;
}

/// Un miembro de la red ya resuelta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Member {
    /// Su posición.
    pub pos: [i32; 3],
    /// La velocidad de toda la red (RPM con signo; 0 = detenida).
    pub speed: i32,
    /// Su bitmask de conexiones al resolver.
    pub conns: u8,
}

/// BFS desde `start` por conexiones de eje: dos bloques cinéticos se conectan cuando el
/// vecino está a lo largo del eje de ambos y comparten ese eje. La velocidad de la red la
/// da la fuente de menor posición (x,y,z); sin fuentes, 0. `None` sólo si `start` no es un
/// nodo. Los miembros salen ordenados por posición (determinista para pruebas y diffs).
pub fn solve<W: KineticWorld>(world: &W, start: [i32; 3]) -> Option<Vec<Member>> {
    world.node(start)?;
    let mut members: Vec<([i32; 3], Node, u8)> = Vec::new();
    let mut visited: Vec<[i32; 3]> = vec![start];
    let mut queue: VecDeque<[i32; 3]> = VecDeque::from([start]);
    while let Some(pos) = queue.pop_front() {
        let Some(node) = world.node(pos) else {
            continue;
        };
        let delta = node.axis.delta();
        let mut conns = 0u8;
        for (positive, step) in [(true, delta), (false, [-delta[0], -delta[1], -delta[2]])] {
            let next = add(pos, step);
            let Some(other) = world.node(next) else {
                continue;
            };
            if other.axis != node.axis {
                continue;
            }
            conns |= conn_bit(node.axis, positive);
            if !visited.contains(&next) {
                visited.push(next);
                queue.push_back(next);
            }
        }
        members.push((pos, node, conns));
    }
    let speed = members
        .iter()
        .filter(|(_, node, _)| node.source_speed != 0)
        .min_by_key(|(pos, _, _)| (pos[0], pos[1], pos[2]))
        .map_or(0, |(_, node, _)| node.source_speed);
    members.sort_by_key(|(pos, _, _)| (pos[0], pos[1], pos[2]));
    Some(
        members
            .into_iter()
            .map(|(pos, _, conns)| Member { pos, speed, conns })
            .collect(),
    )
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
            source_speed: 0,
        }
    }

    fn wheel(axis: Axis, rpm: i32) -> Node {
        Node {
            axis,
            source_speed: rpm,
        }
    }

    fn member_at(members: &[Member], pos: [i32; 3]) -> Member {
        *members.iter().find(|m| m.pos == pos).expect("miembro")
    }

    #[test]
    fn line_propagates_speed_and_conns() {
        let mut map = HashMap::new();
        map.insert([0, 0, 0], wheel(Axis::X, 8));
        map.insert([1, 0, 0], shaft(Axis::X));
        map.insert([2, 0, 0], shaft(Axis::X));
        let members = solve(&MapWorld(map.clone()), [1, 0, 0]).unwrap();
        assert_eq!(members.len(), 3);
        assert!(members.iter().all(|m| m.speed == 8));
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
        map.insert([0, 0, 0], wheel(Axis::X, 8));
        map.insert([1, 0, 0], shaft(Axis::Y));
        let members = solve(&MapWorld(map.clone()), [0, 0, 0]).unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].conns, 0);
        // Y desde el otro lado tampoco: la red del eje Y existe aparte.
        let other = solve(&MapWorld(map.clone()), [1, 0, 0]).unwrap();
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].speed, 0);
    }

    #[test]
    fn no_source_gives_zero() {
        let mut map = HashMap::new();
        map.insert([0, 0, 0], shaft(Axis::Z));
        map.insert([0, 0, 1], shaft(Axis::Z));
        let members = solve(&MapWorld(map.clone()), [0, 0, 0]).unwrap();
        assert_eq!(members.len(), 2);
        assert!(members.iter().all(|m| m.speed == 0));
    }

    #[test]
    fn gap_splits_network() {
        let mut map = HashMap::new();
        map.insert([0, 0, 0], wheel(Axis::X, 8));
        map.insert([1, 0, 0], shaft(Axis::X));
        // hueco en [2,0,0]
        map.insert([3, 0, 0], shaft(Axis::X));
        let left = solve(&MapWorld(map.clone()), [1, 0, 0]).unwrap();
        assert_eq!(left.len(), 2);
        assert!(left.iter().all(|m| m.speed == 8));
        let right = solve(&MapWorld(map.clone()), [3, 0, 0]).unwrap();
        assert_eq!(right.len(), 1);
        assert_eq!(right[0].speed, 0);
    }

    #[test]
    fn lowest_source_wins() {
        let mut map = HashMap::new();
        map.insert([0, 0, 0], wheel(Axis::X, -8));
        map.insert([1, 0, 0], shaft(Axis::X));
        map.insert([2, 0, 0], wheel(Axis::X, 8));
        let members = solve(&MapWorld(map.clone()), [1, 0, 0]).unwrap();
        assert!(members.iter().all(|m| m.speed == -8));
    }

    #[test]
    fn inactive_wheel_is_not_a_source() {
        let mut map = HashMap::new();
        map.insert([0, 0, 0], wheel(Axis::X, 0));
        map.insert([1, 0, 0], shaft(Axis::X));
        let members = solve(&MapWorld(map.clone()), [1, 0, 0]).unwrap();
        assert_eq!(members.len(), 2);
        assert!(members.iter().all(|m| m.speed == 0));
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
        let members = solve(&MapWorld(map.clone()), [1, 0, 0]).unwrap();
        let positions: Vec<[i32; 3]> = members.iter().map(|m| m.pos).collect();
        assert_eq!(positions, vec![[0, 0, 0], [1, 0, 0], [2, 0, 0]]);
    }
}
