use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use tokio::sync::watch;

use crate::beacon::server::{self, Query, ServerStatus};
use crate::beacon::stats::Stats;
use crate::beacon::{self, Hub, Session};
use crate::config::{Config, ConfigForm, ConfigOutcome};
use crate::deps::Probe;
use crate::event::AppEvent;
use crate::i18n::{self, Key, Lang};
use crate::keys::{self, AppAction};
use crate::setup::SystemCheckState;
use crate::theme::{Status, Theme};
use crate::ui::tabs::Tab;

/// Cada cuánto se guardan las estadísticas del día en disco.
const SAVE_EVERY: Duration = Duration::from_secs(30);

/// Canal de recarga en caliente hacia el servidor. Guardar en el overlay de
/// configuración empuja la config nueva por acá.
pub struct Reload {
    pub config: watch::Sender<Config>,
}

/// Acciones que se resuelven fuera de `App` porque necesitan el canal de eventos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingAction {
    StartDemo,
}

pub struct App {
    pub config: Config,
    pub reload: Reload,
    pub lang: Lang,
    pub theme: Theme,
    pub active_tab: Tab,
    pub should_quit: bool,
    /// Contador de ticks: anima la mascota y el spinner.
    pub tick: u64,

    pub system_check: SystemCheckState,
    pub hub: Hub,
    pub server: Option<ServerStatus>,
    pub demo: bool,
    /// Reproductor de sonido detectado al arrancar (lo usa `main`).
    pub player: Option<&'static str>,

    /// Sesión elegida a mano; sólo cuenta si `auto` es falso.
    pub focus_sid: Option<String>,
    /// Seguir siempre a la sesión con el evento más reciente.
    pub auto: bool,

    pub sessions_row: usize,
    pub activity_scroll: usize,

    pub config_form: Option<ConfigForm>,
    pub help_open: bool,
    pub toast: Option<(String, Status)>,
    pending: Option<PendingAction>,
    last_save: Instant,
}

impl App {
    pub fn new(
        config: Config,
        reload: Reload,
        stats: Stats,
        server: Option<ServerStatus>,
        system_check: SystemCheckState,
    ) -> Self {
        Self {
            lang: config.ui.language,
            theme: Theme::default(),
            active_tab: Tab::Now,
            should_quit: false,
            tick: 0,
            system_check,
            hub: Hub::new(stats),
            server,
            demo: false,
            player: None,
            focus_sid: None,
            auto: true,
            sessions_row: 0,
            activity_scroll: 0,
            config_form: None,
            help_open: false,
            toast: None,
            pending: None,
            last_save: Instant::now(),
            config,
            reload,
        }
    }

    pub fn take_pending(&mut self) -> Option<PendingAction> {
        self.pending.take()
    }

    pub fn on_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Tick => {
                self.tick = self.tick.wrapping_add(1);
                self.hub.roll_day();
                if self.last_save.elapsed() >= SAVE_EVERY {
                    self.save_stats();
                }
            }
            AppEvent::Key(key) => {
                if key.kind != KeyEventKind::Release {
                    self.toast = None;
                    self.on_key(key);
                }
            }
            AppEvent::Beacon(events) => {
                for ev in &events {
                    self.hub.handle(ev);
                }
            }
            AppEvent::Query(query, reply) => {
                let now = beacon::now();
                let body = match query {
                    Query::Health => self.hub.health_json(now),
                    Query::Debug => self.hub.debug_json(now),
                };
                let _ = reply.send(body);
            }
            AppEvent::Server(status) => {
                self.server = Some(status);
                self.system_check.recheck(self.probe());
            }
            AppEvent::Shutdown => self.should_quit = true,
        }
    }

    pub fn save_stats(&mut self) {
        self.last_save = Instant::now();
        if let Err(e) = self.hub.stats.save() {
            tracing::warn!(error = %e, "no se pudieron guardar las estadísticas");
        }
    }

    /// Datos para la verificación del sistema, según el estado actual.
    pub fn probe(&self) -> Probe {
        Probe {
            server: match &self.server {
                Some(ServerStatus::Failed(e)) => Err(e.clone()),
                Some(ServerStatus::Listening(addr)) => Ok(addr.clone()),
                None => Ok(server::address(&self.config)),
            },
            token: Probe::token_source(&self.config),
            port: self.config.server.port,
            stats_path: self.hub.stats.path().cloned(),
            received: self.hub.received,
            demo: self.demo,
            player: self.player,
            sound_on: self.config.ui.sound,
        }
    }

    pub fn server_ok(&self) -> bool {
        !matches!(self.server, Some(ServerStatus::Failed(_)))
    }

    // -- sesión en foco --

    /// La sesión que muestran AHORA y el detalle: la fijada a mano si sigue
    /// viva, o la más reciente.
    pub fn current(&self) -> Option<&Session> {
        let live = self.hub.live(beacon::now());
        if !self.auto
            && let Some(sid) = &self.focus_sid
            && let Some(s) = live.iter().find(|s| &s.sid == sid)
        {
            return Some(s);
        }
        live.first().copied()
    }

    fn next_session(&mut self) {
        let live = self.hub.live(beacon::now());
        if live.is_empty() {
            self.toast = Some((i18n::t(self.lang, Key::MainNoSessions).to_string(), Status::Info));
            return;
        }
        let current = self.current().map(|s| s.sid.clone());
        let i = live.iter().position(|s| Some(&s.sid) == current.as_ref()).map_or(0, |i| i + 1);
        let next = live[i % live.len()];
        let project = next.project.clone();
        self.focus_sid = Some(next.sid.clone());
        self.auto = false;
        self.toast = Some((i18n::t(self.lang, Key::MainFocusTemplate).replacen("{}", &project, 1), Status::Info));
    }

    // -- entrada de teclado --

    fn on_key(&mut self, key: KeyEvent) {
        if self.config_form.is_some() {
            self.on_config_key(key);
            return;
        }
        if self.help_open {
            self.help_open = false;
            return;
        }

        let Some(action) = keys::action_for(key) else {
            return;
        };
        self.dispatch(action);
    }

    fn dispatch(&mut self, action: AppAction) {
        match action {
            AppAction::Quit => self.should_quit = true,
            AppAction::NextTab => self.active_tab = self.active_tab.next(),
            AppAction::PrevTab => self.active_tab = self.active_tab.prev(),
            AppAction::JumpTab(i) => {
                if let Some(tab) = Tab::from_index(i) {
                    self.active_tab = tab;
                }
            }
            AppAction::OpenConfig => self.config_form = Some(ConfigForm::new(&self.config)),
            AppAction::Help => self.help_open = true,
            AppAction::Refresh => self.system_check.recheck(self.probe()),
            AppAction::Up => self.move_by(-1),
            AppAction::Down => self.move_by(1),
            AppAction::Top => self.move_by(isize::MIN),
            AppAction::Bottom => self.move_by(isize::MAX),
            AppAction::NextSession => self.next_session(),
            AppAction::Auto => {
                self.auto = true;
                self.toast = Some((i18n::t(self.lang, Key::MainAutoFollow).to_string(), Status::Info));
            }
            AppAction::Demo => {
                if self.demo {
                    self.toast = Some((i18n::t(self.lang, Key::MainDemoRunning).to_string(), Status::Info));
                } else {
                    self.demo = true;
                    self.pending = Some(PendingAction::StartDemo);
                    self.system_check.recheck(self.probe());
                    self.toast = Some((i18n::t(self.lang, Key::MainDemoStarted).to_string(), Status::Info));
                }
            }
            AppAction::NextMascot => {
                let mut config = self.config.clone();
                config.ui.mascot = config.ui.mascot.next();
                let name = config.ui.mascot.label(self.lang);
                self.apply_config(config);
                // El aviso de "guardado en..." se reemplaza por uno más útil.
                if self.toast.as_ref().is_some_and(|(_, s)| *s == Status::Ok) {
                    self.toast =
                        Some((i18n::t(self.lang, Key::MainMascotTemplate).replacen("{}", name, 1), Status::Ok));
                }
            }
            AppAction::ToggleSound => {
                let mut config = self.config.clone();
                config.ui.sound = !config.ui.sound;
                let key = if config.ui.sound { Key::MainSoundOn } else { Key::MainSoundOff };
                self.apply_config(config);
                if self.toast.as_ref().is_some_and(|(_, s)| *s == Status::Ok) {
                    self.toast = Some((i18n::t(self.lang, key).to_string(), Status::Ok));
                }
            }
            AppAction::Enter if self.active_tab == Tab::Sessions => {
                let live = self.hub.live(beacon::now());
                if let Some(s) = live.get(self.sessions_row) {
                    let project = s.project.clone();
                    self.focus_sid = Some(s.sid.clone());
                    self.auto = false;
                    self.toast =
                        Some((i18n::t(self.lang, Key::MainFocusTemplate).replacen("{}", &project, 1), Status::Info));
                }
            }
            AppAction::Enter | AppAction::Back => {}
        }
    }

    /// Mueve la selección o el scroll de la pestaña activa. `isize::MIN`/`MAX`
    /// van al principio/fin.
    fn move_by(&mut self, delta: isize) {
        let step = |value: usize, len: usize| -> usize {
            let max = len.saturating_sub(1);
            match delta {
                isize::MIN => 0,
                isize::MAX => max,
                d if d < 0 => value.saturating_sub(d.unsigned_abs()),
                d => (value + d as usize).min(max),
            }
        };
        match self.active_tab {
            Tab::Sessions => {
                let len = self.hub.live(beacon::now()).len();
                self.sessions_row = step(self.sessions_row, len);
            }
            Tab::Activity => self.activity_scroll = step(self.activity_scroll, self.hub.timeline.len()),
            Tab::SystemCheck => {
                let len = self.system_check.report.checks.len();
                self.system_check.selected = step(self.system_check.selected, len);
            }
            Tab::Now | Tab::Stats => {}
        }
    }

    fn on_config_key(&mut self, key: KeyEvent) {
        let Some(form) = self.config_form.as_mut() else {
            return;
        };
        // `q` se escribe en los campos de texto; sólo Esc cierra el overlay.
        if key.code == KeyCode::Char('c') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }
        match form.on_key(key) {
            ConfigOutcome::None => {}
            ConfigOutcome::Cancel => self.config_form = None,
            ConfigOutcome::Save(config) => {
                self.apply_config(*config);
                self.config_form = None;
            }
        }
    }

    fn apply_config(&mut self, config: Config) {
        self.lang = config.ui.language;
        self.system_check.set_lang(self.lang);
        self.toast = Some(match config.save() {
            Ok(path) => {
                tracing::info!(path = %path.display(), "configuración guardada");
                (
                    i18n::t(self.lang, Key::MainConfigSavedTemplate).replacen("{}", &path.display().to_string(), 1),
                    Status::Ok,
                )
            }
            Err(e) => {
                tracing::error!(error = %format!("{e:#}"), "no se pudo guardar la configuración");
                (
                    i18n::t(self.lang, Key::MainConfigSaveErrTemplate).replacen("{}", &format!("{e:#}"), 1),
                    Status::Error,
                )
            }
        });
        let _ = self.reload.config.send(config.clone());
        self.config = config;
        self.system_check.recheck(self.probe());
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::i18n::Lang;
    use crossterm::event::KeyModifiers;
    use serde_json::json;

    pub fn app() -> App {
        let config = Config::default();
        let (config_tx, _rx) = watch::channel(config.clone());
        let stats = Stats::new(None);
        let probe = Probe {
            server: Ok("0.0.0.0:8765".into()),
            token: crate::deps::TokenSource::None,
            port: 8765,
            stats_path: None,
            received: 0,
            demo: false,
            player: None,
            sound_on: true,
        };
        let system_check = SystemCheckState::new(Lang::default(), probe);
        App::new(config, Reload { config: config_tx }, stats, None, system_check)
    }

    pub fn with_sessions(app: &mut App) {
        let events = vec![
            json!({"sid": "a", "project": "web", "kind": "session_start", "model": "claude-opus-5-5", "state": "idle",
                   "measure": {"fiveHour": {"percent": 42}, "context": {"percent": 30, "tokens": 60000, "window": 200000}, "usd": 1.2}}),
            json!({"sid": "a", "project": "web", "kind": "turn_start", "prompt": "arregla el login"}),
            json!({"sid": "a", "project": "web", "kind": "tasks", "tasks": [{"id": "1", "subject": "leer", "status": "completed"}, {"id": "2", "subject": "editar", "status": "in_progress"}]}),
            json!({"sid": "a", "project": "web", "kind": "agent_start", "agentId": "x", "type": "Explore", "description": "buscar tests"}),
            json!({"sid": "a", "project": "web", "kind": "tool_done", "tool": "Read", "detail": "src/main.rs"}),
            json!({"sid": "b", "project": "api", "kind": "state", "state": "editing", "detail": "src/lib.rs"}),
        ];
        app.on_event(AppEvent::Beacon(events));
    }

    fn press(app: &mut App, code: KeyCode) {
        app.on_event(AppEvent::Key(KeyEvent::new(code, KeyModifiers::NONE)));
    }

    #[test]
    fn s_fija_la_siguiente_sesion_y_a_vuelve_a_auto() {
        let mut app = app();
        with_sessions(&mut app);
        let first = app.current().unwrap().sid.clone();
        press(&mut app, KeyCode::Char('s'));
        assert!(!app.auto);
        assert_ne!(app.current().unwrap().sid, first);
        press(&mut app, KeyCode::Char('a'));
        assert!(app.auto);
        assert_eq!(app.current().unwrap().sid, first);
    }

    #[test]
    fn enter_en_sesiones_fija_la_seleccionada() {
        let mut app = app();
        with_sessions(&mut app);
        press(&mut app, KeyCode::Char('2'));
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        let live = app.hub.live(beacon::now());
        assert_eq!(app.current().unwrap().sid, live[1].sid);
    }

    #[test]
    fn d_pide_arrancar_la_demo_una_sola_vez() {
        let mut app = app();
        press(&mut app, KeyCode::Char('d'));
        assert_eq!(app.take_pending(), Some(PendingAction::StartDemo));
        press(&mut app, KeyCode::Char('d'));
        assert_eq!(app.take_pending(), None);
    }

    #[test]
    fn el_tick_cambia_de_dia_sin_eventos() {
        let mut app = app();
        app.hub.stats.day.turns = 5;
        app.hub.stats.day.date = "2000-01-01".into();
        app.on_event(AppEvent::Tick);
        assert_eq!(app.hub.stats.day.turns, 0);
        assert_ne!(app.hub.stats.day.date, "2000-01-01");
    }

    #[test]
    fn el_servidor_caido_pone_en_rojo_la_verificacion() {
        let mut app = app();
        app.on_event(AppEvent::Server(ServerStatus::Failed("0.0.0.0:8765: en uso".into())));
        assert!(!app.system_check.report.all_ok());
        assert!(!app.server_ok());
    }
}
