use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    Quit,
    NextTab,
    PrevTab,
    /// Salto directo a una sección del menú (`1`..`6`).
    JumpTab(usize),
    Up,
    Down,
    Top,
    Bottom,
    OpenConfig,
    /// Re-verificación del sistema (SYSTEM CHECK).
    Refresh,
    Help,
    /// Pasa el foco a la siguiente sesión viva y deja de seguir a la más reciente.
    NextSession,
    /// Vuelve a seguir automáticamente a la sesión más reciente.
    Auto,
    /// Arranca la sesión simulada.
    Demo,
    /// Cambia a la siguiente mascota y lo guarda en la config.
    NextMascot,
    /// Activa o silencia los sonidos y lo guarda en la config.
    ToggleSound,
    /// Activa o desactiva las notificaciones de escritorio.
    ToggleNotify,
    /// Enter: en SESIONES fija el foco en la sesión seleccionada.
    Enter,
    /// Esc: cierra/vuelve; en ACTIVIDAD limpia los filtros.
    Back,
    /// `/`: buscar en ACTIVIDAD.
    Search,
    /// `f`: siguiente tipo de entrada en ACTIVIDAD.
    CycleKind,
    /// `p`: siguiente proyecto en ACTIVIDAD.
    CycleProject,
    /// Espacio: congela o reanuda el registro en ACTIVIDAD.
    TogglePause,
}

pub fn action_for(key: KeyEvent) -> Option<AppAction> {
    match key.code {
        KeyCode::Char('q') => Some(AppAction::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(AppAction::Quit),
        KeyCode::Char('c') => Some(AppAction::OpenConfig),
        KeyCode::F(1) => Some(AppAction::OpenConfig),
        KeyCode::Tab => Some(AppAction::NextTab),
        KeyCode::BackTab => Some(AppAction::PrevTab),
        KeyCode::Char(d @ '1'..='6') => {
            Some(AppAction::JumpTab(d.to_digit(10).unwrap() as usize - 1))
        }
        KeyCode::Up | KeyCode::Char('k') => Some(AppAction::Up),
        KeyCode::Down | KeyCode::Char('j') => Some(AppAction::Down),
        KeyCode::Home | KeyCode::Char('g') => Some(AppAction::Top),
        KeyCode::End | KeyCode::Char('G') => Some(AppAction::Bottom),
        KeyCode::Char('r') | KeyCode::Char('R') => Some(AppAction::Refresh),
        KeyCode::Char('h') | KeyCode::Char('H') => Some(AppAction::Help),
        KeyCode::Char('s') | KeyCode::Char('S') => Some(AppAction::NextSession),
        KeyCode::Char('a') | KeyCode::Char('A') => Some(AppAction::Auto),
        KeyCode::Char('d') | KeyCode::Char('D') => Some(AppAction::Demo),
        KeyCode::Char('m') | KeyCode::Char('M') => Some(AppAction::NextMascot),
        KeyCode::Char('v') | KeyCode::Char('V') => Some(AppAction::ToggleSound),
        KeyCode::Char('n') | KeyCode::Char('N') => Some(AppAction::ToggleNotify),
        KeyCode::Enter => Some(AppAction::Enter),
        KeyCode::Esc => Some(AppAction::Back),
        KeyCode::Char('/') => Some(AppAction::Search),
        KeyCode::Char('f') | KeyCode::Char('F') => Some(AppAction::CycleKind),
        KeyCode::Char('p') | KeyCode::Char('P') => Some(AppAction::CycleProject),
        KeyCode::Char(' ') => Some(AppAction::TogglePause),
        _ => None,
    }
}
