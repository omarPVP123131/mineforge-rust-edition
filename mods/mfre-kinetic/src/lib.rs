//! Cinética MFRE — mod nativo (D21/D23): el launcher compila esta crate contra el juego
//! fijado y llama a `hooks()`. Spike F42: hello-world nativo que valida la toolchain en el
//! árbol del fork (build + check) antes del núcleo cinético.
use mod_api::{self, Hooks, PlayerLoggedInEvent, ServerHost, Text};

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Un jugador entró al servidor: saludo del spike (QA §5.13 lo sustituirá por contenido).
    fn player_logged_in(&self, host: &mut dyn ServerHost, event: &mut PlayerLoggedInEvent<'_>) {
        let line = format!(
            "Cinética MFRE (spike nativo F42) dice hola, {}!",
            event.name
        );
        if let Some(player) = host.player(event.player) {
            let _ = player.tell(host, Text::literal(line));
        }
    }
}
