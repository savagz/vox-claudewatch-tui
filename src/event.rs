use crossterm::event::KeyEvent;
use serde_json::Value;
use tokio::sync::oneshot;

use crate::beacon::server::{Query, ServerStatus};

#[derive(Debug)]
pub enum AppEvent {
    Tick,
    Key(KeyEvent),
    /// Eventos del mod `activity-beacon` recibidos en un POST (o generados por la demo).
    Beacon(Vec<Value>),
    /// El servidor pide un JSON con el estado (`/health`, `/debug`).
    Query(Query, oneshot::Sender<String>),
    /// El servidor abrió (o no pudo abrir) su socket.
    Server(ServerStatus),
    /// SIGTERM/SIGINT a nivel de proceso: hay que salir ya para restaurar la terminal.
    Shutdown,
}
