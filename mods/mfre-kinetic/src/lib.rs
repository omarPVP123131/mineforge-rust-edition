//! Cinética MFRE — mod nativo (D21/D23): potencia rotacional al estilo Create, reescrita
//! clean-room sobre Mod API 0.2. El launcher compila esta crate contra el juego fijado.
//!
//! F42, incremento 4: engranajes y recetas — el resolvedor (`kinetic`) generaliza a
//! nodos con tipo (eje/rueda/engranaje/engranaje grande): conexiones axiales vía eje,
//! mallas de engranajes perpendiculares al desplazamiento con inversión de sentido y
//! relación 2:1/1:2, velocidades f32 por miembro y BFS multi-fuente en orden de posición.
//! `state` lo conecta al nivel y al cliente (attachment + espejo nbt + render giratorio
//! con `spin`+`keep`; los modelos de mundo están vacíos y los de item quedan estáticos).
//! Recetas de crafteo para los cuatro bloques (data/mfre_kinetic/recipe/).
mod kinetic;
mod state;

use mod_api::{
    self, Block, BlockShapes, BlockStateProperty, BlocksEvent, Hooks, PlayerLoggedInEvent,
    RegistrationContext, ServerHost, Text,
};
use state::Kinetic;
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

/// La propiedad `axis` propia de los bloques cinéticos, con su valor por defecto.
fn with_axis(block: &mut Block, default: &str) {
    block.state_properties.insert(
        "axis".to_string(),
        BlockStateProperty::Enum {
            values: vec!["x".to_string(), "y".to_string(), "z".to_string()],
        },
    );
    block
        .default_state
        .insert("axis".to_string(), default.to_string());
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
    fn blocks(&self, _: &RegistrationContext, event: &mut BlocksEvent<'_>) {
        // El eje: transmisión recta a lo largo de su `axis`.
        let mut shaft = Block::new("mfre_kinetic:shaft", "minecraft:iron_block");
        shaft.properties.hardness = Some(1.0);
        // No es un cubo macizo: los vecinos dibujan sus caras y la luz pasa.
        shaft.properties.occludes = Some(false);
        with_axis(&mut shaft, "y");
        let shapes = shaft_shapes();
        shaft.collision_shapes = Some(shapes.clone());
        shaft.shapes = Some(shapes);
        event.blocks.push(shaft);

        // La rueda hidráulica: fuente de 8 RPM con agua en sus vecinos perpendiculares.
        // Su geometría la dibuja el block entity girando: el modelo de mundo está vacío
        // y no ocluye, como el eje.
        let mut wheel = Block::new("mfre_kinetic:water_wheel", "minecraft:iron_block");
        wheel.properties.hardness = Some(1.5);
        wheel.properties.occludes = Some(false);
        with_axis(&mut wheel, "x");
        event.blocks.push(wheel);

        // Engranaje y engranaje grande: mallan entre sí (perpendicular al desplazamiento,
        // invierten el sentido; grande↔chico cambia la velocidad ×2 / ×0.5) y aceptan un
        // eje coaxial. Todo los dibuja el block entity girando.
        let mut cog = Block::new("mfre_kinetic:cogwheel", "minecraft:iron_block");
        cog.properties.hardness = Some(1.0);
        cog.properties.occludes = Some(false);
        with_axis(&mut cog, "y");
        event.blocks.push(cog);

        let mut large_cog = Block::new("mfre_kinetic:large_cogwheel", "minecraft:iron_block");
        large_cog.properties.hardness = Some(1.5);
        large_cog.properties.occludes = Some(false);
        with_axis(&mut large_cog, "y");
        event.blocks.push(large_cog);
    }

    /// Una vez al arrancar: claves de registro, el tipo de block entity cinético y su Part.
    fn register(&self, r: &mut dyn mod_api::Registrar, _: &RegistrationContext) {
        let _ = state::SHAFT.set(r.block_key("mfre_kinetic:shaft"));
        let _ = state::WHEEL.set(r.block_key("mfre_kinetic:water_wheel"));
        let _ = state::COG.set(r.block_key("mfre_kinetic:cogwheel"));
        let _ = state::LARGE_COG.set(r.block_key("mfre_kinetic:large_cogwheel"));
        let _ = state::WATER.set(r.block_key("minecraft:water"));
        let kind = r
            .block_entity_type("kinetic")
            .blocks(&["shaft", "water_wheel", "cogwheel", "large_cogwheel"])
            .build();
        r.block_entity_part_with_state(&kind, Kinetic)
            .expect("native block entity type");
    }
}
