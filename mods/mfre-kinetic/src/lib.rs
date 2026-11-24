//! Cinética MFRE — mod nativo (D21/D23): potencia rotacional al estilo Create, reescrita
//! clean-room sobre Mod API 0.2. El launcher compila esta crate contra el juego fijado.
//!
//! F42, incremento 1: el **eje** (`mfre_kinetic:shaft`) — primer bloque cinético, con su
//! propiedad `axis` propia, formas de colisión/outline por eje y assets completos. La red
//! rotacional (block entities, grafo velocidad/estrés, render giratorio) llega en los
//! próximos incrementos.
use mod_api::{
    self, Block, BlockShapes, BlockStateProperty, BlocksEvent, Hooks, PlayerLoggedInEvent,
    ServerHost, Text,
};
use std::collections::BTreeMap;

#[derive(Default)]
struct Mod;

/// El cubo central del eje (10/16), común a las tres orientaciones.
const HUB: [f64; 6] = [0.3125, 0.3125, 0.3125, 0.6875, 0.6875, 0.6875];

/// Las cajas del eje por valor de `axis`: brazo pasante (4/16 de grosor) + cubo.
fn shaft_shapes() -> BlockShapes {
    let mut values = BTreeMap::new();
    values.insert(
        "x".to_string(),
        vec![[0.0, 0.375, 0.375, 1.0, 0.625, 0.625], HUB],
    );
    values.insert(
        "y".to_string(),
        vec![[0.375, 0.0, 0.375, 0.625, 1.0, 0.625], HUB],
    );
    values.insert(
        "z".to_string(),
        vec![[0.375, 0.375, 0.0, 0.625, 0.625, 1.0], HUB],
    );
    BlockShapes {
        properties: vec!["axis".to_string()],
        values,
        default: None,
    }
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Un jugador entró al servidor: saludo que confirma que el mod nativo cargó (QA §5.13).
    fn player_logged_in(&self, host: &mut dyn ServerHost, event: &mut PlayerLoggedInEvent<'_>) {
        let line = format!(
            "Cinética MFRE (spike nativo F42) dice hola, {}!",
            event.name
        );
        if let Some(player) = host.player(event.player) {
            let _ = player.tell(host, Text::literal(line));
        }
    }

    /// Bloques del mod declarados en código (misma declaración que el JSON de contenido).
    fn blocks(&self, _: &mod_api::RegistrationContext, event: &mut BlocksEvent<'_>) {
        let mut shaft = Block::new("mfre_kinetic:shaft", "minecraft:iron_block");
        shaft.properties.hardness = Some(1.0);
        // No es un cubo macizo: los vecinos dibujan sus caras y la luz pasa.
        shaft.properties.occludes = Some(false);
        shaft.state_properties.insert(
            "axis".to_string(),
            BlockStateProperty::Enum {
                values: vec!["x".to_string(), "y".to_string(), "z".to_string()],
            },
        );
        shaft
            .default_state
            .insert("axis".to_string(), "y".to_string());
        let shapes = shaft_shapes();
        shaft.collision_shapes = Some(shapes.clone());
        shaft.shapes = Some(shapes);
        event.blocks.push(shaft);
    }
}
