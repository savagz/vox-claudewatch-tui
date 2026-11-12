use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tokio::sync::watch;

use crate::beacon::server::{self, Query, ServerStatus};
use crate::beacon::stats::Stats;
use crate::beacon::{self, Hub, NoteKind, Session, TimelineEntry};
use crate::config::{Config, ConfigForm, ConfigOutcome};
use crate::deps::Probe;
use crate::event::AppEvent;
use crate::i18n::{self, Key, Lang};
use crate::keys::{self, AppAction};
use crate::setup::SystemCheckState;
use crate::theme::{Status, Theme};
use crate::ui::tabs::{Tab, activity};

/// Cada cuánto se guardan las estadísticas del día en disco.
const SAVE_EVERY: Duration = Duration::from_secs(30);

/// Canal de recarga en caliente hacia el servidor. Guardar en el overlay de
/// configuración empuja la config nueva por acá.
pub struct Reload {
    pub config: watch::Sender<Config>,
}

/// Filtros de la pestaña Actividad.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ActivityView {
    /// Texto buscado, sin distinguir mayúsculas.
    pub query: String,
    /// Se está escribiendo la búsqueda: las teclas van al texto.
    pub editing: bool,
    pub kind: Option<NoteKind>,
    pub project: Option<String>,
    /// Vista congelada: no entra nada posterior a este instante.
    pub paused_at: Option<f64>,
}

impl ActivityView {
    pub fn is_filtered(&self) -> bool {
        !self.query.is_empty() || self.kind.is_some() || self.project.is_some() || self.paused_at.is_some()
    }

    fn matches(&self, lang: Lang, e: &TimelineEntry) -> bool {
        if self.paused_at.is_some_and(|t| e.ts > t)
            || self.kind.is_some_and(|k| e.note.kind() != k)
            || self.project.as_ref().is_some_and(|p| &e.project != p)
        {
            return false;
        }
        if self.query.is_empty() {
            return true;
        }
        let query = self.query.to_lowercase();
        e.project.to_lowercase().contains(&query) || activity::note_text(lang, &e.note).to_lowercase().contains(&query)
    }
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
    /// Notificador de escritorio detectado al arrancar (lo usa `main`).
    pub notifier: Option<&'static str>,

    /// Sesión elegida a mano; sólo cuenta si `auto` es falso.
    pub focus_sid: Option<String>,
    /// Seguir siempre a la sesión con el evento más reciente.
    pub auto: bool,

    pub sessions_row: usize,
    pub activity_scroll: usize,
    pub activity: ActivityView,

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
            notifier: None,
            focus_sid: None,
            auto: true,
            sessions_row: 0,
            activity_scroll: 0,
            activity: ActivityView::default(),
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
            notifier: self.notifier,
            notify_on: self.config.ui.notify,
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

    /// Entradas del registro que pasan los filtros, lo más nuevo primero.
    pub fn activity_entries(&self) -> Vec<&TimelineEntry> {
        self.hub.timeline.iter().rev().filter(|e| self.activity.matches(self.lang, e)).collect()
    }

    /// Proyectos presentes en el registro, en orden de aparición.
    fn activity_projects(&self) -> Vec<String> {
        let mut projects: Vec<String> = Vec::new();
        for e in &self.hub.timeline {
            if !projects.contains(&e.project) {
                projects.push(e.project.clone());
            }
        }
        projects
    }

    /// Escritura de la búsqueda (`/`): `Enter` la confirma, `Esc` la descarta.
    fn on_search_key(&mut self, key: KeyEvent) {
        let view = &mut self.activity;
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.should_quit = true,
            KeyCode::Esc => {
                view.editing = false;
                view.query.clear();
            }
            KeyCode::Enter => view.editing = false,
            KeyCode::Backspace => {
                view.query.pop();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => view.query.push(c),
            _ => {}
        }
        self.activity_scroll = 0;
    }

    /// Acciones propias de la pestaña Actividad.
    fn on_activity_action(&mut self, action: AppAction) {
        let view = &mut self.activity;
        match action {
            AppAction::Search => view.editing = true,
            AppAction::CycleKind => {
                view.kind = match view.kind.and_then(|k| NoteKind::ALL.iter().position(|x| *x == k)) {
                    None => Some(NoteKind::ALL[0]),
                    Some(i) => NoteKind::ALL.get(i + 1).copied(),
                };
            }
            AppAction::CycleProject => {
                let projects = self.activity_projects();
                let view = &mut self.activity;
                view.project = match view.project.as_ref().and_then(|p| projects.iter().position(|x| x == p)) {
                    None => projects.first().cloned(),
                    Some(i) => projects.get(i + 1).cloned(),
                };
            }
            AppAction::TogglePause => {
                view.paused_at = if view.paused_at.is_some() { None } else { Some(beacon::now()) };
            }
            AppAction::Back => *view = ActivityView::default(),
            _ => return,
        }
        self.activity_scroll = 0;
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
        if self.active_tab == Tab::Activity && self.activity.editing {
            self.on_search_key(key);
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
            AppAction::ToggleNotify => {
                let mut config = self.config.clone();
                config.ui.notify = !config.ui.notify;
                let key = if config.ui.notify { Key::MainNotifyOn } else { Key::MainNotifyOff };
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
            AppAction::Search
            | AppAction::CycleKind
            | AppAction::CycleProject
            | AppAction::TogglePause
            | AppAction::Back
                if self.active_tab == Tab::Activity =>
            {
                self.on_activity_action(action)
            }
            AppAction::Search
            | AppAction::CycleKind
            | AppAction::CycleProject
            | AppAction::TogglePause
            | AppAction::Enter
            | AppAction::Back => {}
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
            Tab::Activity => self.activity_scroll = step(self.activity_scroll, self.activity_entries().len()),
            Tab::SystemCheck => {
                let len = self.system_check.report.checks.len();
                self.system_check.selected = step(self.system_check.selected, len);
            }
            Tab::Now | Tab::Stats | Tab::Office => {}
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
            notifier: None,
            notify_on: false,
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

    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            press(app, KeyCode::Char(c));
        }
    }

    #[test]
    fn la_busqueda_captura_las_teclas() {
        let mut app = app();
        with_sessions(&mut app);
        press(&mut app, KeyCode::Char('4'));
        press(&mut app, KeyCode::Char('/'));
        assert!(app.activity.editing);
        // `q` y `1` se escriben: no salen ni cambian de pestaña.
        type_text(&mut app, "q1");
        assert!(!app.should_quit);
        assert_eq!(app.active_tab, Tab::Activity);
        assert_eq!(app.activity.query, "q1");
        press(&mut app, KeyCode::Esc);
        assert!(!app.activity.editing);
        assert!(app.activity.query.is_empty());

        press(&mut app, KeyCode::Char('/'));
        type_text(&mut app, "LOGIN");
        press(&mut app, KeyCode::Enter);
        assert!(!app.activity.editing);
        let entries = app.activity_entries();
        assert_eq!(entries.len(), 1, "sólo el prompt 'arregla el login'");
        assert!(matches!(entries[0].note, beacon::Note::Prompt(_)));
    }

    #[test]
    fn filtra_por_tipo_y_proyecto() {
        let mut app = app();
        with_sessions(&mut app);
        let total = app.hub.timeline.len();
        press(&mut app, KeyCode::Char('4'));
        // Primer tipo: sesiones.
        press(&mut app, KeyCode::Char('f'));
        assert_eq!(app.activity.kind, Some(NoteKind::Session));
        assert!(app.activity_entries().iter().all(|e| e.note.kind() == NoteKind::Session));
        // Recorre todos los tipos y vuelve a "todos".
        for _ in 1..NoteKind::ALL.len() {
            press(&mut app, KeyCode::Char('f'));
        }
        assert_eq!(app.activity.kind, Some(NoteKind::Plan));
        press(&mut app, KeyCode::Char('f'));
        assert_eq!(app.activity.kind, None);
        assert_eq!(app.activity_entries().len(), total);

        press(&mut app, KeyCode::Char('p'));
        assert_eq!(app.activity.project.as_deref(), Some("web"));
        assert!(app.activity_entries().iter().all(|e| e.project == "web"));
        // Sólo `web` dejó entradas: el siguiente vuelve a "todos".
        press(&mut app, KeyCode::Char('p'));
        assert_eq!(app.activity.project, None);

        press(&mut app, KeyCode::Char('f'));
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.activity, ActivityView::default());
    }

    #[test]
    fn la_pausa_congela_el_registro() {
        let mut app = app();
        with_sessions(&mut app);
        press(&mut app, KeyCode::Char('4'));
        press(&mut app, KeyCode::Char(' '));
        let frozen = app.activity_entries().len();
        app.hub.handle_at(
            &json!({"sid": "a", "project": "web", "kind": "turn_start", "prompt": "otra cosa"}),
            beacon::now() + 10.0,
        );
        assert_eq!(app.activity_entries().len(), frozen);
        press(&mut app, KeyCode::Char(' '));
        assert_eq!(app.activity_entries().len(), frozen + 1);
    }

    #[test]
    fn los_filtros_no_actuan_fuera_de_actividad() {
        let mut app = app();
        press(&mut app, KeyCode::Char('f'));
        press(&mut app, KeyCode::Char('/'));
        assert_eq!(app.activity, ActivityView::default());
    }

    #[test]
    fn el_scroll_se_acota_a_lo_filtrado() {
        let mut app = app();
        with_sessions(&mut app);
        press(&mut app, KeyCode::Char('4'));
        press(&mut app, KeyCode::Char('p'));
        press(&mut app, KeyCode::End);
        assert_eq!(app.activity_scroll, app.activity_entries().len() - 1);
    }

    #[test]
    fn el_servidor_caido_pone_en_rojo_la_verificacion() {
        let mut app = app();
        app.on_event(AppEvent::Server(ServerStatus::Failed("0.0.0.0:8765: en uso".into())));
        assert!(!app.system_check.report.all_ok());
        assert!(!app.server_ok());
    }
}
