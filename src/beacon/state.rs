use serde::{Deserialize, Serialize};

/// Lo que está haciendo Claude en una sesión. Los estados vienen del mod
/// `activity-beacon` como texto; `Idle`, `Sleeping` y `Offline` además los
/// deriva la TUI según el tiempo transcurrido.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    #[default]
    Idle,
    Sleeping,
    Offline,
    Working,
    Thinking,
    Answering,
    Reading,
    Searching,
    Editing,
    Running,
    Web,
    Delegating,
    Asking,
    Planning,
    PlanReady,
    Organizing,
    Mcp,
    Tool,
    Done,
    Error,
}

impl State {
    #[cfg(test)]
    pub const ALL: [State; 20] = [
        State::Idle,
        State::Sleeping,
        State::Offline,
        State::Working,
        State::Thinking,
        State::Answering,
        State::Reading,
        State::Searching,
        State::Editing,
        State::Running,
        State::Web,
        State::Delegating,
        State::Asking,
        State::Planning,
        State::PlanReady,
        State::Organizing,
        State::Mcp,
        State::Tool,
        State::Done,
        State::Error,
    ];

    /// Un estado desconocido (versión nueva del mod) se muestra como trabajando.
    pub fn parse(raw: &str) -> Self {
        match raw {
            "idle" => State::Idle,
            "sleeping" => State::Sleeping,
            "offline" => State::Offline,
            "thinking" => State::Thinking,
            "answering" => State::Answering,
            "reading" => State::Reading,
            "searching" => State::Searching,
            "editing" => State::Editing,
            "running" => State::Running,
            "web" => State::Web,
            "delegating" => State::Delegating,
            "asking" => State::Asking,
            "planning" => State::Planning,
            "plan-ready" => State::PlanReady,
            "organizing" => State::Organizing,
            "mcp" => State::Mcp,
            "tool" => State::Tool,
            "done" => State::Done,
            "error" => State::Error,
            _ => State::Working,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            State::Idle => "idle",
            State::Sleeping => "sleeping",
            State::Offline => "offline",
            State::Working => "working",
            State::Thinking => "thinking",
            State::Answering => "answering",
            State::Reading => "reading",
            State::Searching => "searching",
            State::Editing => "editing",
            State::Running => "running",
            State::Web => "web",
            State::Delegating => "delegating",
            State::Asking => "asking",
            State::Planning => "planning",
            State::PlanReady => "plan-ready",
            State::Organizing => "organizing",
            State::Mcp => "mcp",
            State::Tool => "tool",
            State::Done => "done",
            State::Error => "error",
        }
    }

    /// Claude está parado esperando una decisión del usuario.
    pub fn is_waiting(self) -> bool {
        matches!(self, State::Asking | State::PlanReady)
    }

    /// Cuenta como "trabajando" en la cabecera y en el LED de la sidebar.
    pub fn is_busy(self) -> bool {
        !matches!(self, State::Idle | State::Sleeping | State::Offline | State::Done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_y_as_str_son_inversos() {
        for state in State::ALL {
            assert_eq!(State::parse(state.as_str()), state);
        }
    }

    #[test]
    fn estado_desconocido_es_trabajando() {
        assert_eq!(State::parse("algo-nuevo"), State::Working);
    }
}
