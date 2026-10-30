pub mod demo;
pub mod measure;
pub mod server;
pub mod state;
pub mod stats;

use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use measure::Measure;
pub use state::State;
use stats::Stats;

/// Tras `done`, segundos que se mantiene la celebración antes de pasar a idle.
const DONE_HOLD: f64 = 4.0;
/// Segundos sin cambios para que una sesión idle se duerma.
const SLEEP_AFTER: f64 = 300.0;
/// Segundos sin eventos para dar una sesión por perdida.
const STALE_AFTER: f64 = 7200.0;
/// Tope de sesiones en memoria, por si llegan muchos `sid` distintos en poco tiempo.
const MAX_SESSIONS: usize = 100;
const MAX_TIMELINE: usize = 200;
const MAX_RAW: usize = 50;

/// Hora actual en segundos Unix con decimales.
pub fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub subject: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct Agent {
    pub id: String,
    pub kind: String,
    pub description: String,
    pub started: f64,
    /// Última herramienta que usó el subagente, con su detalle.
    pub tool: String,
    pub finished: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub sid: String,
    pub project: String,
    pub cwd: String,
    pub model: String,
    pub state: State,
    pub detail: String,
    pub state_since: f64,
    pub turn_started: Option<f64>,
    /// Duración del último turno terminado, en segundos.
    pub last_turn: Option<f64>,
    pub prompt: String,
    pub tasks: Vec<Task>,
    /// En orden de creación.
    pub agents: Vec<Agent>,
    pub measure: Measure,
    pub last_seen: f64,
    pub ended: bool,
    pub usd: Option<f64>,
    usd_turn_start: Option<f64>,
    pub turn_cost: Option<f64>,
}

impl Session {
    fn new(sid: &str, now: f64) -> Self {
        Self {
            sid: sid.to_string(),
            project: "?".to_string(),
            cwd: String::new(),
            model: String::new(),
            state: State::Idle,
            detail: String::new(),
            state_since: now,
            turn_started: None,
            last_turn: None,
            prompt: String::new(),
            tasks: Vec::new(),
            agents: Vec::new(),
            measure: Measure::default(),
            last_seen: now,
            ended: false,
            usd: None,
            usd_turn_start: None,
            turn_cost: None,
        }
    }

    /// Estado a mostrar: el último recibido, envejecido según el tiempo.
    pub fn visible_state(&self, now: f64) -> State {
        if self.ended || now - self.last_seen > STALE_AFTER {
            return State::Offline;
        }
        let quiet = now - self.state_since;
        match self.state {
            State::Done if quiet > SLEEP_AFTER => State::Sleeping,
            State::Done if quiet > DONE_HOLD => State::Idle,
            State::Idle if quiet > SLEEP_AFTER => State::Sleeping,
            state => state,
        }
    }

    pub fn agent_mut(&mut self, id: &str) -> Option<&mut Agent> {
        self.agents.iter_mut().find(|a| a.id == id)
    }

    pub fn active_agents(&self, now: f64) -> Vec<&Agent> {
        self.agents.iter().filter(|a| a.finished.is_none() && now - a.started < 3600.0).collect()
    }

    /// Subagentes terminados más recientes primero.
    pub fn recent_agents(&self, n: usize) -> Vec<&Agent> {
        let mut done: Vec<&Agent> = self.agents.iter().filter(|a| a.finished.is_some()).collect();
        done.sort_by(|a, b| b.finished.unwrap_or(0.0).total_cmp(&a.finished.unwrap_or(0.0)));
        done.truncate(n);
        done
    }

    pub fn tasks_done(&self) -> usize {
        self.tasks.iter().filter(|t| t.status == "completed").count()
    }
}

/// Una entrada del registro de actividad. Se guarda estructurada y se traduce
/// al pintar, para que cambiar de idioma también cambie lo ya registrado.
#[derive(Debug, Clone, PartialEq)]
pub enum Note {
    SessionStarted { model: String },
    SessionEnded { reason: String },
    Prompt(String),
    NewTurn,
    Question(String),
    PlanMode,
    PlanReady,
    Tool { agent: Option<String>, tool: String, detail: String, ok: bool },
    TaskDone(String),
    TaskStarted(String),
    AgentStarted { kind: String, description: String },
    AgentDone(String),
    TurnDone { secs: f64, aborted: bool },
}

impl Note {
    pub fn icon(&self) -> &'static str {
        match self {
            Note::SessionStarted { .. } => "◆",
            Note::SessionEnded { .. } => "◇",
            Note::Prompt(_) | Note::NewTurn => "»",
            Note::Question(_) => "?",
            Note::PlanMode | Note::PlanReady => "▤",
            Note::Tool { tool, .. } => tool_icon(tool),
            Note::TaskDone(_) => "✔",
            Note::TaskStarted(_) => "▶",
            Note::AgentStarted { .. } | Note::AgentDone(_) => "⧉",
            Note::TurnDone { aborted: false, .. } => "✓",
            Note::TurnDone { aborted: true, .. } => "■",
        }
    }
}

pub fn tool_icon(tool: &str) -> &'static str {
    match tool {
        "Read" => "◧",
        "Grep" | "Glob" => "⌕",
        "Edit" | "Write" => "✎",
        "Bash" => "▶",
        "WebFetch" | "WebSearch" => "◎",
        "Agent" => "⧉",
        "AskUserQuestion" => "?",
        "EnterPlanMode" | "ExitPlanMode" => "▤",
        "TaskCreate" | "TaskUpdate" | "TodoWrite" => "☐",
        _ => "•",
    }
}

/// Momentos que merecen un sonido: cambios de estado importantes, no cada
/// herramienta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cue {
    SessionStart,
    SessionEnd,
    Prompt,
    Asking,
    PlanReady,
    Delegating,
    Done,
    Error,
}

impl Cue {
    pub const ALL: [Cue; 8] = [
        Cue::SessionStart,
        Cue::SessionEnd,
        Cue::Prompt,
        Cue::Asking,
        Cue::PlanReady,
        Cue::Delegating,
        Cue::Done,
        Cue::Error,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Cue::SessionStart => "session-start",
            Cue::SessionEnd => "session-end",
            Cue::Prompt => "prompt",
            Cue::Asking => "asking",
            Cue::PlanReady => "plan-ready",
            Cue::Delegating => "delegating",
            Cue::Done => "done",
            Cue::Error => "error",
        }
    }

    /// Cuánto importa oírlo: lo que te espera primero, el ruido de fondo al final.
    pub fn priority(self) -> u8 {
        match self {
            Cue::Asking => 7,
            Cue::PlanReady => 6,
            Cue::Error => 5,
            Cue::Done => 4,
            Cue::SessionEnd => 3,
            Cue::SessionStart => 2,
            Cue::Delegating => 1,
            Cue::Prompt => 0,
        }
    }

    /// Estados que suenan al entrar en ellos.
    fn for_state(state: State) -> Option<Cue> {
        match state {
            State::Asking => Some(Cue::Asking),
            State::PlanReady => Some(Cue::PlanReady),
            State::Done => Some(Cue::Done),
            State::Error => Some(Cue::Error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimelineEntry {
    pub ts: f64,
    pub project: String,
    pub note: Note,
}

/// Aplica los eventos del mod al estado en memoria.
pub struct Hub {
    pub sessions: HashMap<String, Session>,
    pub timeline: VecDeque<TimelineEntry>,
    pub stats: Stats,
    pub last_event: Option<f64>,
    pub received: u64,
    raw: VecDeque<Value>,
    /// Sonidos pendientes; los vacía el bucle principal con `take_cues`.
    cues: Vec<Cue>,
}

fn text<'a>(ev: &'a Value, key: &str) -> &'a str {
    ev.get(key).and_then(Value::as_str).unwrap_or("")
}

fn number(ev: &Value, key: &str) -> u64 {
    ev.get(key).and_then(Value::as_f64).map_or(0, |n| n.max(0.0) as u64)
}

fn flag(ev: &Value, key: &str) -> bool {
    ev.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// Recorta a `max` caracteres (no bytes) para que el registro no crezca.
pub fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn parse_tasks(v: Option<&Value>) -> Option<Vec<Task>> {
    let list = v?.as_array()?;
    Some(
        list.iter()
            .map(|t| Task {
                id: t.get("id").map(|id| id.as_str().map_or_else(|| id.to_string(), str::to_string)).unwrap_or_default(),
                subject: text(t, "subject").to_string(),
                status: t.get("status").and_then(Value::as_str).unwrap_or("pending").to_string(),
            })
            .collect(),
    )
}

impl Hub {
    pub fn new(stats: Stats) -> Self {
        Self {
            sessions: HashMap::new(),
            timeline: VecDeque::with_capacity(MAX_TIMELINE),
            stats,
            last_event: None,
            received: 0,
            raw: VecDeque::with_capacity(MAX_RAW),
            cues: Vec::new(),
        }
    }

    pub fn take_cues(&mut self) -> Vec<Cue> {
        std::mem::take(&mut self.cues)
    }

    /// Sesiones no perdidas, la más reciente primero.
    pub fn live(&self, now: f64) -> Vec<&Session> {
        let mut live: Vec<&Session> =
            self.sessions.values().filter(|s| s.visible_state(now) != State::Offline).collect();
        live.sort_by(|a, b| b.last_seen.total_cmp(&a.last_seen).then_with(|| a.sid.cmp(&b.sid)));
        live
    }

    /// La sesión vista más recientemente aunque ya esté perdida.
    pub fn latest(&self) -> Option<&Session> {
        self.sessions.values().max_by(|a, b| a.last_seen.total_cmp(&b.last_seen))
    }

    fn log(&mut self, project: &str, note: Note, now: f64) {
        if self.timeline.len() == MAX_TIMELINE {
            self.timeline.pop_front();
        }
        self.timeline.push_back(TimelineEntry { ts: now, project: project.to_string(), note });
    }

    pub fn handle(&mut self, ev: &Value) {
        self.handle_at(ev, now());
    }

    /// Antes de crear una sesión: olvida las perdidas y, si aún no queda sitio
    /// para la nueva, las más viejas. La más reciente se conserva siempre:
    /// `latest` la sigue mostrando.
    fn prune(&mut self, now: f64) {
        let latest = self.latest().map(|s| s.sid.clone());
        self.sessions.retain(|sid, s| now - s.last_seen <= STALE_AFTER || Some(sid) == latest.as_ref());
        if self.sessions.len() >= MAX_SESSIONS {
            let mut by_age: Vec<(f64, String)> = self.sessions.values().map(|s| (s.last_seen, s.sid.clone())).collect();
            by_age.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, sid) in by_age.into_iter().take(self.sessions.len() + 1 - MAX_SESSIONS) {
                self.sessions.remove(&sid);
            }
        }
    }

    /// Cambio de día de las estadísticas; se llama con cada evento y cada tick,
    /// para que a medianoche se reinicien aunque no llegue nada.
    pub fn roll_day(&mut self) {
        let sessions = &self.sessions;
        self.stats.roll(|sid| sessions.contains_key(sid));
    }

    pub fn handle_at(&mut self, ev: &Value, now: f64) {
        self.roll_day();
        if self.raw.len() == MAX_RAW {
            self.raw.pop_front();
        }
        self.raw.push_back(ev.clone());
        self.last_event = Some(now);
        self.received += 1;

        let sid = match ev.get("sid") {
            Some(Value::String(s)) if !s.is_empty() => s.clone(),
            _ => "?".to_string(),
        };
        if !self.sessions.contains_key(&sid) {
            self.prune(now);
        }
        let s = self.sessions.entry(sid.clone()).or_insert_with(|| Session::new(&sid, now));
        if !text(ev, "project").is_empty() {
            s.project = text(ev, "project").to_string();
        }
        if !text(ev, "cwd").is_empty() {
            s.cwd = text(ev, "cwd").to_string();
        }
        s.last_seen = now;
        s.ended = false;
        let project = s.project.clone();

        let mut notes: Vec<Note> = Vec::new();
        match text(ev, "kind") {
            kind @ ("session_start" | "snapshot") => {
                self.apply_snapshot(&sid, ev, kind == "session_start", now);
                if kind == "session_start" {
                    self.cues.push(Cue::SessionStart);
                    notes.push(Note::SessionStarted { model: self.sessions[&sid].model.clone() });
                }
            }
            "session_end" => {
                self.set_state(&sid, State::Idle, "", now);
                if let Some(s) = self.sessions.get_mut(&sid) {
                    s.ended = true;
                }
                self.cues.push(Cue::SessionEnd);
                notes.push(Note::SessionEnded { reason: text(ev, "reason").to_string() });
            }
            "turn_start" => {
                let s = self.sessions.get_mut(&sid).expect("sesión recién insertada");
                s.turn_started = Some(now);
                s.usd_turn_start = Some(s.usd.unwrap_or(0.0));
                s.prompt = text(ev, "prompt").to_string();
                self.cues.push(Cue::Prompt);
                if !text(ev, "model").is_empty() {
                    s.model = text(ev, "model").to_string();
                }
                notes.push(if s.prompt.is_empty() { Note::NewTurn } else { Note::Prompt(clip(&s.prompt, 70)) });
            }
            "state" => {
                let raw = text(ev, "state");
                let state = State::parse(if raw.is_empty() { "working" } else { raw });
                let detail = text(ev, "detail");
                self.set_state(&sid, state, detail, now);
                match state {
                    State::Asking => {
                        self.stats.day.questions += 1;
                        notes.push(Note::Question(clip(detail, 70)));
                    }
                    State::Planning => {
                        self.stats.day.plans += 1;
                        notes.push(Note::PlanMode);
                    }
                    State::PlanReady => notes.push(Note::PlanReady),
                    _ => {}
                }
            }
            "tool_done" => notes.extend(self.on_tool_done(&sid, ev, now)),
            "agent_tool" => {
                let s = self.sessions.get_mut(&sid).expect("sesión recién insertada");
                if let Some(agent) = s.agent_mut(text(ev, "agentId")) {
                    agent.tool = format!("{} {}", text(ev, "tool"), text(ev, "detail")).trim().to_string();
                }
            }
            "tasks" => {
                let st = &mut self.stats.day;
                let s = self.sessions.get_mut(&sid).expect("sesión recién insertada");
                let before: HashMap<String, String> =
                    s.tasks.iter().map(|t| (t.id.clone(), t.status.clone())).collect();
                s.tasks = parse_tasks(ev.get("tasks")).unwrap_or_default();
                for t in &s.tasks {
                    let prev = before.get(&t.id).map(String::as_str);
                    if t.status == "completed" && prev != Some("completed") {
                        st.tasks_done += 1;
                        notes.push(Note::TaskDone(clip(&t.subject, 60)));
                    } else if t.status == "in_progress" && prev != Some("in_progress") {
                        notes.push(Note::TaskStarted(clip(&t.subject, 60)));
                    }
                }
            }
            "agent_start" => {
                let s = self.sessions.get_mut(&sid).expect("sesión recién insertada");
                let id = match text(ev, "agentId") {
                    "" => format!("a{}", s.agents.len()),
                    id => id.to_string(),
                };
                let kind = match text(ev, "type") {
                    "" => "agent".to_string(),
                    t => t.to_string(),
                };
                let description = text(ev, "description").to_string();
                notes.push(Note::AgentStarted { kind: kind.clone(), description: clip(&description, 50) });
                s.agents.retain(|a| a.id != id);
                s.agents.push(Agent { id, kind, description, started: now, tool: String::new(), finished: None });
                self.stats.day.agents += 1;
                self.cues.push(Cue::Delegating);
            }
            "agent_done" => {
                let s = self.sessions.get_mut(&sid).expect("sesión recién insertada");
                if let Some(agent) = s.agent_mut(text(ev, "agentId"))
                    && agent.finished.is_none()
                {
                    agent.finished = Some(now);
                    notes.push(Note::AgentDone(clip(&agent.description, 50)));
                }
            }
            "turn_done" => {
                let ms = number(ev, "ms");
                let aborted = flag(ev, "aborted");
                let st = &mut self.stats.day;
                st.turns += 1;
                st.turn_ms += ms;
                if aborted {
                    st.aborted += 1;
                }
                if let Some(usage) = ev.get("usage") {
                    st.tokens_in += number(usage, "input_tokens");
                    st.tokens_out += number(usage, "output_tokens");
                    st.cache_read += number(usage, "cache_read_input_tokens");
                    st.cache_write += number(usage, "cache_creation_input_tokens");
                }
                let s = self.sessions.get_mut(&sid).expect("sesión recién insertada");
                s.last_turn = Some(ms as f64 / 1000.0);
                s.turn_started = None;
                if let (Some(usd), Some(start)) = (s.usd, s.usd_turn_start) {
                    s.turn_cost = Some((usd - start).max(0.0));
                }
                notes.push(Note::TurnDone { secs: ms as f64 / 1000.0, aborted });
            }
            "measure" => {
                if let Some(m) = ev.get("measure") {
                    self.apply_measure(&sid, &Measure::from_json(m), false, now);
                }
                let s = self.sessions.get_mut(&sid).expect("sesión recién insertada");
                if !text(ev, "model").is_empty() {
                    s.model = text(ev, "model").to_string();
                }
            }
            _ => {}
        }

        for note in notes {
            self.log(&project, note, now);
        }
    }

    fn on_tool_done(&mut self, sid: &str, ev: &Value, now: f64) -> Option<Note> {
        let tool = match text(ev, "tool") {
            "" => "?",
            t => t,
        };
        let ok = ev.get("ok").and_then(Value::as_bool).unwrap_or(true);
        let detail = text(ev, "detail");

        let st = &mut self.stats.day;
        *st.tools.entry(tool.to_string()).or_default() += 1;
        st.tool_ms += number(ev, "ms");
        if !ok {
            st.tool_errors += 1;
        }
        match tool {
            "Read" => Stats::add_file(&mut st.files_read, detail),
            "Edit" | "Write" | "NotebookEdit" => Stats::add_file(&mut st.files_edited, detail),
            "Bash" => st.bash += 1,
            _ => {}
        }
        self.stats.tick_activity(now);

        let s = self.sessions.get_mut(sid)?;
        let agent = s.agent_mut(text(ev, "agentId")).map(|a| {
            a.tool = tool.to_string();
            a.kind.clone()
        });
        if matches!(tool, "TaskCreate" | "TaskUpdate" | "TodoWrite") {
            return None;
        }
        Some(Note::Tool { agent, tool: tool.to_string(), detail: clip(detail, 60), ok })
    }

    fn set_state(&mut self, sid: &str, state: State, detail: &str, now: f64) {
        let Some(s) = self.sessions.get_mut(sid) else {
            return;
        };
        if s.state.is_waiting() && !state.is_waiting() {
            self.stats.day.waiting_ms += ((now - s.state_since).max(0.0) * 1000.0) as u64;
        }
        if state != s.state {
            s.state_since = now;
            if let Some(cue) = Cue::for_state(state) {
                self.cues.push(cue);
            }
        }
        s.state = state;
        s.detail = detail.to_string();
    }

    fn apply_snapshot(&mut self, sid: &str, ev: &Value, fresh: bool, now: f64) {
        if let Some(s) = self.sessions.get_mut(sid)
            && !text(ev, "model").is_empty()
        {
            s.model = text(ev, "model").to_string();
        }
        if !text(ev, "state").is_empty() {
            self.set_state(sid, State::parse(text(ev, "state")), text(ev, "detail"), now);
        }
        if let (Some(tasks), Some(s)) = (parse_tasks(ev.get("tasks")), self.sessions.get_mut(sid)) {
            s.tasks = tasks;
        }
        if let Some(m) = ev.get("measure").filter(|m| m.is_object()) {
            self.apply_measure(sid, &Measure::from_json(m), fresh, now);
        }
    }

    fn apply_measure(&mut self, sid: &str, m: &Measure, fresh: bool, now: f64) {
        let Some(s) = self.sessions.get_mut(sid) else {
            return;
        };
        s.measure.merge(m);
        let last = &mut self.stats.last;
        if m.five_hour.is_some() {
            last.five_hour = m.five_hour;
            last.limits_at = Some(now);
        }
        if m.seven_day.is_some() {
            last.seven_day = m.seven_day;
            last.limits_at = Some(now);
        }
        if m.context.is_some() {
            last.context = m.context;
            last.context_project = s.project.clone();
            last.context_at = Some(now);
        }
        if let Some(usd) = m.usd {
            s.usd = Some(usd);
            if s.turn_started.is_none()
                && let Some(start) = s.usd_turn_start
            {
                s.turn_cost = Some((usd - start).max(0.0));
            }
            let (sid, project) = (s.sid.clone(), s.project.clone());
            self.stats.record_cost(&sid, &project, usd, fresh);
        }
    }

    /// Cuerpo de `GET /health`.
    pub fn health_json(&self, now: f64) -> String {
        json!({"ok": true, "sessions": self.live(now).len()}).to_string()
    }

    /// Cuerpo de `GET /debug`: estado por sesión y los últimos eventos crudos.
    pub fn debug_json(&self, now: f64) -> String {
        let sessions: serde_json::Map<String, Value> = self
            .sessions
            .values()
            .map(|s| {
                let m = &s.measure;
                (
                    s.sid.clone(),
                    json!({
                        "project": s.project,
                        "state": s.visible_state(now).as_str(),
                        "model": s.model,
                        "measure": {
                            "fiveHour": m.five_hour,
                            "sevenDay": m.seven_day,
                            "context": m.context,
                            "usd": m.usd,
                        },
                    }),
                )
            })
            .collect();
        json!({"sessions": sessions, "events": self.raw}).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn hub() -> Hub {
        Hub::new(Stats::new(None))
    }

    fn ev(kind: &str, extra: Value) -> Value {
        let mut base = json!({"sid": "s1", "project": "demo-app", "cwd": "/tmp/demo-app", "kind": kind});
        base.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        base
    }

    #[test]
    fn ciclo_de_un_turno() {
        let mut h = hub();
        let t = 1000.0;
        h.handle_at(&ev("session_start", json!({"model": "claude-opus-5-5", "state": "idle", "measure": {"usd": 0.0}})), t);
        h.handle_at(&ev("turn_start", json!({"prompt": "arregla el bug"})), t + 1.0);
        h.handle_at(&ev("state", json!({"state": "reading", "detail": "src/main.rs"})), t + 2.0);
        h.handle_at(&ev("tool_done", json!({"tool": "Read", "detail": "src/main.rs", "ok": true, "ms": 40})), t + 3.0);
        h.handle_at(&ev("tool_done", json!({"tool": "Bash", "detail": "cargo test", "ok": false})), t + 4.0);
        h.handle_at(&ev("measure", json!({"measure": {"usd": 0.25, "fiveHour": {"percent": 40}}})), t + 5.0);
        h.handle_at(&ev("turn_done", json!({"ms": 9000, "usage": {"input_tokens": 100, "output_tokens": 50, "cache_read_input_tokens": 300}})), t + 9.0);
        h.handle_at(&ev("state", json!({"state": "done"})), t + 9.0);

        let s = &h.sessions["s1"];
        assert_eq!(s.model, "claude-opus-5-5");
        assert_eq!(s.prompt, "arregla el bug");
        assert_eq!(s.last_turn, Some(9.0));
        assert_eq!(s.turn_cost, Some(0.25));
        assert_eq!(s.visible_state(t + 10.0), State::Done);
        assert_eq!(s.visible_state(t + 20.0), State::Idle);
        assert_eq!(s.visible_state(t + 400.0), State::Sleeping);

        let d = &h.stats.day;
        assert_eq!(d.turns, 1);
        assert_eq!(d.tools["Read"], 1);
        assert_eq!(d.tool_errors, 1);
        assert_eq!(d.bash, 1);
        assert_eq!(d.files_read, vec!["src/main.rs".to_string()]);
        assert_eq!(d.cache_hit().map(|p| p.round()), Some(75.0));
        assert_eq!(h.stats.last.five_hour.unwrap().percent, Some(40.0));
        assert!((h.stats.day_cost() - 0.25).abs() < 1e-9);
        assert_eq!(h.timeline.len(), 5);
    }

    #[test]
    fn tareas_y_subagentes() {
        let mut h = hub();
        let tasks = |a: &str, b: &str| json!({"tasks": [{"id": "1", "subject": "uno", "status": a}, {"id": "2", "subject": "dos", "status": b}]});
        h.handle_at(&ev("tasks", tasks("pending", "pending")), 1.0);
        h.handle_at(&ev("tasks", tasks("in_progress", "pending")), 2.0);
        h.handle_at(&ev("tasks", tasks("completed", "in_progress")), 3.0);
        assert_eq!(h.stats.day.tasks_done, 1);
        assert_eq!(h.sessions["s1"].tasks_done(), 1);

        h.handle_at(&ev("agent_start", json!({"agentId": "ag1", "type": "Explore", "description": "revisar tests"})), 4.0);
        h.handle_at(&ev("agent_tool", json!({"agentId": "ag1", "tool": "Grep", "detail": "fn main"})), 5.0);
        assert_eq!(h.sessions["s1"].agents[0].tool, "Grep fn main");
        assert_eq!(h.sessions["s1"].active_agents(6.0).len(), 1);
        h.handle_at(&ev("agent_done", json!({"agentId": "ag1"})), 6.0);
        assert!(h.sessions["s1"].active_agents(7.0).is_empty());
        assert_eq!(h.sessions["s1"].recent_agents(3).len(), 1);
    }

    #[test]
    fn suena_solo_al_cambiar_a_un_estado_importante() {
        let mut h = hub();
        h.handle_at(&ev("session_start", json!({"state": "idle"})), 1.0);
        h.handle_at(&ev("state", json!({"state": "reading"})), 2.0);
        h.handle_at(&ev("state", json!({"state": "error"})), 3.0);
        h.handle_at(&ev("state", json!({"state": "error"})), 4.0);
        h.handle_at(&ev("state", json!({"state": "done"})), 5.0);
        assert_eq!(h.take_cues(), vec![Cue::SessionStart, Cue::Error, Cue::Done]);
        assert!(h.take_cues().is_empty());
    }

    #[test]
    fn tiempo_esperando_al_usuario() {
        let mut h = hub();
        h.handle_at(&ev("state", json!({"state": "asking", "detail": "¿Despliego?"})), 10.0);
        h.handle_at(&ev("state", json!({"state": "working"})), 25.0);
        assert_eq!(h.stats.day.waiting_ms, 15_000);
        assert_eq!(h.stats.day.questions, 1);
    }

    #[test]
    fn sesiones_terminadas_o_perdidas_no_estan_vivas() {
        let mut h = hub();
        h.handle_at(&ev("snapshot", json!({"state": "working"})), 1.0);
        h.handle_at(&json!({"sid": "s2", "kind": "snapshot", "state": "idle"}), 2.0);
        assert_eq!(h.live(3.0).iter().map(|s| s.sid.as_str()).collect::<Vec<_>>(), vec!["s2", "s1"]);
        h.handle_at(&ev("session_end", json!({"reason": "exit"})), 4.0);
        assert_eq!(h.live(5.0).len(), 1);
        assert!(h.live(2.0 + STALE_AFTER + 1.0).is_empty());
    }

    #[test]
    fn olvida_las_sesiones_perdidas_menos_la_ultima() {
        let mut h = hub();
        h.handle_at(&json!({"sid": "vieja", "kind": "snapshot"}), 1.0);
        h.handle_at(&json!({"sid": "media", "kind": "snapshot"}), 2.0);
        // Una sesión nueva, mucho después: las perdidas se purgan.
        h.handle_at(&json!({"sid": "nueva", "kind": "snapshot"}), 3.0 + STALE_AFTER);
        let mut sids: Vec<&str> = h.sessions.keys().map(String::as_str).collect();
        sids.sort();
        assert_eq!(sids, vec!["media", "nueva"]);
    }

    #[test]
    fn no_guarda_mas_de_max_sesiones() {
        let mut h = hub();
        for i in 0..MAX_SESSIONS + 10 {
            h.handle_at(&json!({"sid": format!("s{i}"), "kind": "snapshot"}), i as f64);
        }
        assert_eq!(h.sessions.len(), MAX_SESSIONS);
        assert!(h.sessions.contains_key(&format!("s{}", MAX_SESSIONS + 9)));
        assert!(!h.sessions.contains_key("s0"));
    }

    #[test]
    fn debug_json_es_valido() {
        let mut h = hub();
        h.handle_at(&ev("session_start", json!({"measure": {"fiveHour": {"percent": 1}}})), 1.0);
        let v: Value = serde_json::from_str(&h.debug_json(2.0)).unwrap();
        assert_eq!(v["sessions"]["s1"]["measure"]["fiveHour"]["percent"], json!(1.0));
        assert_eq!(v["events"].as_array().unwrap().len(), 1);
    }
}
