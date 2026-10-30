use serde::{Deserialize, Serialize};

use crate::beacon::State;

/// Idioma activo de la interfaz. Se guarda en `Config.ui.language` y se puede
/// cambiar en caliente desde el overlay de configuración, sin reiniciar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    Es,
    En,
}

impl Lang {
    pub fn toggle(self) -> Self {
        match self {
            Lang::Es => Lang::En,
            Lang::En => Lang::Es,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Lang::Es => "ES",
            Lang::En => "EN",
        }
    }
}

/// Catálogo de textos de la interfaz. Cada variante es un punto de texto visible
/// en la TUI; `t()` devuelve la cadena (o plantilla `{}`) en el idioma activo.
/// Deliberadamente plano (sin namespaces): un enum único es más fácil de grepear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    // -- común, reusado entre modales --
    Cancel,
    Save,
    Field,
    Edit,

    // -- header / sidebar --
    HeaderTitle,
    HeaderListening,
    HeaderServerDown,
    HeaderSessionsTemplate,
    HeaderEventsTemplate,
    SidebarConfig,
    SidebarHelp,
    ModeLive,
    ModeDemo,

    // -- tabs --
    TabNow,
    TabSessions,
    TabStats,
    TabActivity,
    TabSystemCheck,

    // -- help bar --
    HbNotice,
    HbView,
    HbJump,
    HbNavigate,
    HbNextSession,
    HbAuto,
    HbDemo,
    HbMascot,
    HbSound,
    HbPin,
    HbRecheck,
    HbConfig,
    HbQuit,

    // -- help modal --
    HelpTitle,
    HelpSectionGlobal,
    HelpSectionSessions,
    HelpSectionActivity,
    HelpSectionSystemCheck,
    HelpTopBottom,
    HelpScroll,
    HelpClose,

    // -- ahora --
    NowStatusTitle,
    NowNoSessions,
    NowWaitingTemplate,
    NowTurnRunning,
    NowLastTurn,
    NowProject,
    NowModel,
    NowCost,
    NowPrompt,
    NowFollowAuto,
    NowFollowPinned,
    NowTasksTitle,
    NowTasksEmpty,
    NowTasksDoneTemplate,
    NowAgentsTitle,
    NowAgentsEmpty,
    NowAgentsActiveTemplate,
    NowRecentTitle,
    GaugeFiveHour,
    GaugeSevenDay,
    GaugeContext,

    // -- sesiones --
    SessionsEmpty,
    SessionsCountTemplate,
    SessionsDetailTitle,
    ColProject,
    ColState,
    ColCost,
    ColSeen,
    FieldSession,
    FieldCwd,
    FieldState,
    FieldSince,
    FieldTurnCost,
    FieldTasks,
    FieldAgents,

    // -- estadísticas --
    StatsLimitsTitle,
    StatsUpdatedTemplate,
    StatsCostTitle,
    StatsCostApiNote,
    StatsCostSession,
    StatsCostTurn,
    StatsCostToday,
    StatsCostEarlier,
    StatsTodayTitle,
    StatsTurns,
    StatsAbortedTemplate,
    StatsAvgTurn,
    StatsActive,
    StatsWaiting,
    StatsTokens,
    StatsTokensTemplate,
    StatsCache,
    StatsCacheTemplate,
    StatsFiles,
    StatsFilesTemplate,
    StatsCommands,
    StatsErrors,
    StatsQuestions,
    StatsQuestionsTemplate,
    StatsAgents,
    StatsTasks,
    StatsTasksTemplate,
    StatsToolsTitle,
    StatsToolsEmpty,
    StatsActivityTitle,
    StatsActivityPeakTemplate,

    // -- actividad --
    ActivityEmpty,
    ActivityCountTemplate,
    NoteSessionStartedTemplate,
    NoteSessionEndedTemplate,
    NotePromptTemplate,
    NoteNewTurn,
    NoteQuestionTemplate,
    NotePlanMode,
    NotePlanReady,
    NoteTaskDoneTemplate,
    NoteTaskStartedTemplate,
    NoteAgentStartedTemplate,
    NoteAgentDoneTemplate,
    NoteTurnDoneTemplate,
    NoteTurnAbortedTemplate,

    // -- tiempo --
    AgoNow,
    AgoTemplate,

    // -- config form --
    ConfigFormTitle,
    ConfigFieldHostLabel,
    ConfigFieldHostHint,
    ConfigFieldPortLabel,
    ConfigFieldPortHint,
    ConfigFieldTokenLabel,
    ConfigFieldTokenHint,
    ConfigFieldLanguageLabel,
    ConfigFieldLanguageHint,
    ConfigFieldMascotLabel,
    ConfigFieldMascotHint,
    ConfigFieldSoundLabel,
    ConfigFieldSoundHint,
    SoundOn,
    SoundOff,
    ConfigErrInvalidTemplate,
    ConfigErrCreateDirTemplate,
    ConfigErrSerialize,
    ConfigErrWriteTemplate,
    ConfigErrNoUserDir,

    // -- main / app --
    MainConfigLoadErrTemplate,
    MainTokenGeneratedTemplate,
    MainTokenErrTemplate,
    MainConfigSavedTemplate,
    MainConfigSaveErrTemplate,
    MainDemoStarted,
    MainDemoRunning,
    MainFocusTemplate,
    MainAutoFollow,
    MainNoSessions,
    MainMascotTemplate,
    MainSoundOn,
    MainSoundOff,

    // -- system check --
    SetupGateTitle,
    SetupPanelTitle,
    SetupStatusAllOk,
    SetupStatusActionRequired,
    SetupStripeAllOk,
    SetupStripeGateBlocking,
    SetupStripePanelBlocking,
    SetupKeyChoose,
    SetupKeyRecheck,
    SetupKeyContinueAnyway,
    SetupKeyExit,

    // -- deps --
    DepServerLabel,
    DepServerOkTemplate,
    DepServerErrTemplate,
    DepTokenLabel,
    DepTokenConfig,
    DepTokenLegacy,
    DepTokenNone,
    DepStatsLabel,
    DepStatsErrTemplate,
    DepStatsNoDir,
    DepModLabel,
    DepModTemplate,
    DepModIpUnknown,
    DepEventsLabel,
    DepEventsOkTemplate,
    DepEventsNone,
    DepSoundLabel,
    DepSoundOkTemplate,
    DepSoundMuted,
    DepSoundNone,
    DepDemoLabel,
    DepDemoOn,
    DepDemoOff,
}

/// Rellena los `{}` de una plantilla en orden. A diferencia de encadenar
/// `replacen`, un `{}` dentro de un valor ya insertado no se vuelve a rellenar.
pub fn fill(template: &str, args: &[&str]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut args = args.iter();
    let mut parts = template.split("{}");
    if let Some(first) = parts.next() {
        out.push_str(first);
    }
    for part in parts {
        out.push_str(args.next().copied().unwrap_or("{}"));
        out.push_str(part);
    }
    out
}

pub fn t(lang: Lang, key: Key) -> &'static str {
    let (es, en) = pair(key);
    match lang {
        Lang::Es => es,
        Lang::En => en,
    }
}

fn pair(key: Key) -> (&'static str, &'static str) {
    use Key::*;
    match key {
        Cancel => ("cancelar", "cancel"),
        Save => ("guardar", "save"),
        Field => ("campo", "field"),
        Edit => ("editar", "edit"),

        HeaderTitle => ("voxclaude · claude watch", "voxclaude · claude watch"),
        HeaderListening => ("ESCUCHANDO ", "LISTENING "),
        HeaderServerDown => ("SERVIDOR CAÍDO ", "SERVER DOWN "),
        HeaderSessionsTemplate => ("{} SESIONES · {} TRABAJANDO  ", "{} SESSIONS · {} WORKING  "),
        HeaderEventsTemplate => ("{} EVENTOS  ", "{} EVENTS  "),
        SidebarConfig => ("config", "config"),
        SidebarHelp => ("ayuda", "help"),
        ModeLive => ("EN VIVO", "LIVE"),
        ModeDemo => ("DEMO", "DEMO"),

        TabNow => ("Ahora", "Now"),
        TabSessions => ("Sesiones", "Sessions"),
        TabStats => ("Estadísticas", "Stats"),
        TabActivity => ("Actividad", "Activity"),
        TabSystemCheck => ("Verificación", "System Check"),

        HbNotice => (" AVISO ", " NOTICE "),
        HbView => ("ver", "view"),
        HbJump => ("saltar", "jump"),
        HbNavigate => ("navegar", "navigate"),
        HbNextSession => ("sesión", "session"),
        HbAuto => ("auto", "auto"),
        HbDemo => ("demo", "demo"),
        HbMascot => ("mascota", "mascot"),
        HbSound => ("sonido", "sound"),
        HbPin => ("fijar", "pin"),
        HbRecheck => ("re-verificar", "re-check"),
        HbConfig => ("config", "config"),
        HbQuit => ("salir", "quit"),

        HelpTitle => ("ayuda", "help"),
        HelpSectionGlobal => ("global", "global"),
        HelpSectionSessions => ("sesiones", "sessions"),
        HelpSectionActivity => ("actividad", "activity"),
        HelpSectionSystemCheck => ("verificación", "system check"),
        HelpTopBottom => ("ir al inicio / fin", "go to top / bottom"),
        HelpScroll => ("desplazar", "scroll"),
        HelpClose => ("cualquier tecla cierra", "any key closes"),

        NowStatusTitle => ("estado", "status"),
        NowNoSessions => ("Sin sesiones activas", "No active sessions"),
        NowWaitingTemplate => (
            "Esperando eventos del mod activity-beacon en el puerto {}. Pulsá D para ver una demo.",
            "Waiting for events from the activity-beacon mod on port {}. Press D for a demo.",
        ),
        NowTurnRunning => ("Turno en curso", "Turn running"),
        NowLastTurn => ("Último turno", "Last turn"),
        NowProject => ("Proyecto", "Project"),
        NowModel => ("Modelo", "Model"),
        NowCost => ("Coste", "Cost"),
        NowPrompt => ("Prompt", "Prompt"),
        NowFollowAuto => ("auto", "auto"),
        NowFollowPinned => ("fija", "pinned"),
        NowTasksTitle => ("tareas", "tasks"),
        NowTasksEmpty => ("Sin lista de tareas en esta sesión", "No task list in this session"),
        NowTasksDoneTemplate => ("{}/{} terminadas", "{}/{} done"),
        NowAgentsTitle => ("subagentes", "subagents"),
        NowAgentsEmpty => ("Ninguno", "None"),
        NowAgentsActiveTemplate => ("{} activos", "{} active"),
        NowRecentTitle => ("actividad reciente", "recent activity"),
        GaugeFiveHour => ("límite 5 h", "5 h limit"),
        GaugeSevenDay => ("límite 7 días", "7 day limit"),
        GaugeContext => ("contexto", "context"),

        SessionsEmpty => (
            "Ninguna sesión activa. Las sesiones aparecen al llegar su primer evento.",
            "No active sessions. Sessions show up when their first event arrives.",
        ),
        SessionsCountTemplate => ("{} vivas", "{} live"),
        SessionsDetailTitle => ("detalle", "detail"),
        ColProject => ("PROYECTO", "PROJECT"),
        ColState => ("ESTADO", "STATE"),
        ColCost => ("COSTE", "COST"),
        ColSeen => ("VISTA", "SEEN"),
        FieldSession => ("Sesión", "Session"),
        FieldCwd => ("Directorio", "Directory"),
        FieldState => ("Estado", "State"),
        FieldSince => ("Desde", "Since"),
        FieldTurnCost => ("Coste turno", "Turn cost"),
        FieldTasks => ("Tareas", "Tasks"),
        FieldAgents => ("Subagentes", "Subagents"),

        StatsLimitsTitle => ("límites", "limits"),
        StatsUpdatedTemplate => ("actualizado {}", "updated {}"),
        StatsCostTitle => ("coste", "cost"),
        StatsCostApiNote => ("equivalente API", "API equivalent"),
        StatsCostSession => ("Sesión", "Session"),
        StatsCostTurn => ("Último turno", "Last turn"),
        StatsCostToday => ("Hoy", "Today"),
        StatsCostEarlier => ("anteriores", "earlier"),
        StatsTodayTitle => ("hoy", "today"),
        StatsTurns => ("Turnos", "Turns"),
        StatsAbortedTemplate => ("{} cancelados", "{} aborted"),
        StatsAvgTurn => ("Media/turno", "Avg/turn"),
        StatsActive => ("Activo", "Active"),
        StatsWaiting => ("Esperándote", "Waiting on you"),
        StatsTokens => ("Tokens", "Tokens"),
        StatsTokensTemplate => ("{} in · {} out", "{} in · {} out"),
        StatsCache => ("Caché", "Cache"),
        StatsCacheTemplate => ("{}% acierto", "{}% hit"),
        StatsFiles => ("Archivos", "Files"),
        StatsFilesTemplate => ("{} leídos · {} editados", "{} read · {} edited"),
        StatsCommands => ("Comandos", "Commands"),
        StatsErrors => ("Errores", "Errors"),
        StatsQuestions => ("Preguntas", "Questions"),
        StatsQuestionsTemplate => ("{} · {} planes", "{} · {} plans"),
        StatsAgents => ("Subagentes", "Subagents"),
        StatsTasks => ("Tareas", "Tasks"),
        StatsTasksTemplate => ("{} terminadas", "{} done"),
        StatsToolsTitle => ("herramientas", "tools"),
        StatsToolsEmpty => ("Ninguna todavía", "None yet"),
        StatsActivityTitle => ("actividad · 60 min", "activity · 60 min"),
        StatsActivityPeakTemplate => ("pico {}/min", "peak {}/min"),

        ActivityEmpty => ("Sin actividad todavía", "No activity yet"),
        ActivityCountTemplate => ("{} entradas", "{} entries"),
        NoteSessionStartedTemplate => ("Sesión iniciada · {}", "Session started · {}"),
        NoteSessionEndedTemplate => ("Sesión terminada ({})", "Session ended ({})"),
        NotePromptTemplate => ("Prompt: {}", "Prompt: {}"),
        NoteNewTurn => ("Nuevo turno", "New turn"),
        NoteQuestionTemplate => ("Pregunta: {}", "Question: {}"),
        NotePlanMode => ("Entra en modo plan", "Enters plan mode"),
        NotePlanReady => ("Plan listo para aprobar", "Plan ready for approval"),
        NoteTaskDoneTemplate => ("Tarea: {}", "Task: {}"),
        NoteTaskStartedTemplate => ("Tarea en curso: {}", "Task in progress: {}"),
        NoteAgentStartedTemplate => ("Subagente {}: {}", "Subagent {}: {}"),
        NoteAgentDoneTemplate => ("Subagente terminado: {}", "Subagent finished: {}"),
        NoteTurnDoneTemplate => ("Turno terminado en {}", "Turn finished in {}"),
        NoteTurnAbortedTemplate => ("Turno cancelado tras {}", "Turn aborted after {}"),

        AgoNow => ("ahora", "now"),
        AgoTemplate => ("hace {}", "{} ago"),

        ConfigFormTitle => ("configuración", "configuration"),
        ConfigFieldHostLabel => ("Host", "Host"),
        ConfigFieldHostHint => (
            "Interfaz donde escucha el servidor. 0.0.0.0 acepta eventos de toda la red.",
            "Interface the server listens on. 0.0.0.0 accepts events from the whole network.",
        ),
        ConfigFieldPortLabel => ("Puerto", "Port"),
        ConfigFieldPortHint => (
            "Debe coincidir con el de CLAUDE_WATCH_URL en el mod. Se re-abre al guardar.",
            "Must match the port in the mod's CLAUDE_WATCH_URL. Re-opened on save.",
        ),
        ConfigFieldTokenLabel => ("Token", "Token"),
        ConfigFieldTokenHint => (
            "Valor de CLAUDE_WATCH_TOKEN. Vacío: se usa ~/.config/claude-watch/token si existe.",
            "Value of CLAUDE_WATCH_TOKEN. Empty: ~/.config/claude-watch/token is used if present.",
        ),
        ConfigFieldLanguageLabel => ("Idioma", "Language"),
        ConfigFieldLanguageHint => ("Idioma de la interfaz (← → para cambiar).", "Interface language (← → to toggle)."),
        ConfigFieldSoundLabel => ("Sonido", "Sound"),
        ConfigFieldSoundHint => (
            "Jingles al empezar, terminar, fallar o cuando Claude te pregunta (← → para cambiar, V en el tablero).",
            "Jingles when Claude starts, finishes, fails or asks you something (← → to toggle, V on the dashboard).",
        ),
        SoundOn => ("activado", "on"),
        SoundOff => ("silenciado", "muted"),
        ConfigFieldMascotLabel => ("Mascota", "Mascot"),
        ConfigFieldMascotHint => (
            "Quién actúa lo que hace Claude: Pixo, Byte o Michi (← → para elegir, M en el tablero).",
            "Who acts out what Claude is doing: Pixo, Byte or Michi (← → to choose, M on the dashboard).",
        ),
        ConfigErrInvalidTemplate => ("Valor inválido en «{}».", "Invalid value in “{}”."),
        ConfigErrCreateDirTemplate => ("no se pudo crear {}", "could not create {}"),
        ConfigErrSerialize => ("no se pudo serializar la configuración", "could not serialize the configuration"),
        ConfigErrWriteTemplate => ("no se pudo escribir {}", "could not write {}"),
        ConfigErrNoUserDir => (
            "no se encontró el directorio de config del usuario",
            "could not find the user config directory",
        ),

        MainConfigLoadErrTemplate => (
            "Error al leer la config, se usan valores por defecto: {}",
            "Error reading the config, using defaults: {}",
        ),
        MainTokenGeneratedTemplate => (
            "Token generado y guardado en {}. Pasalo al mod (lo ves con C o --token)",
            "Token generated and saved to {}. Give it to the mod (see it with C or --token)",
        ),
        MainTokenErrTemplate => (
            "No se pudo generar el token, el servidor acepta eventos de cualquiera: {}",
            "Could not generate the token, the server accepts events from anyone: {}",
        ),
        MainConfigSavedTemplate => ("Configuración guardada en {}", "Configuration saved to {}"),
        MainConfigSaveErrTemplate => ("No se pudo guardar la configuración: {}", "Could not save the configuration: {}"),
        MainDemoStarted => ("Demo iniciada: sesión simulada «demo-app»", "Demo started: simulated session “demo-app”"),
        MainDemoRunning => ("La demo ya está corriendo", "The demo is already running"),
        MainFocusTemplate => ("Foco fijo en {} (A vuelve a auto)", "Focus pinned to {} (A returns to auto)"),
        MainAutoFollow => ("Siguiendo a la sesión más reciente", "Following the most recent session"),
        MainNoSessions => ("No hay sesiones vivas", "There are no live sessions"),
        MainMascotTemplate => ("Mascota: {}", "Mascot: {}"),
        MainSoundOn => ("Sonido activado", "Sound on"),
        MainSoundOff => ("Sonido silenciado", "Sound muted"),

        SetupGateTitle => ("verificación del sistema", "system check"),
        SetupPanelTitle => ("verificación", "system check"),
        SetupStatusAllOk => ("todo en orden", "all good"),
        SetupStatusActionRequired => ("requiere atención", "action required"),
        SetupStripeAllOk => ("Listo para recibir eventos", "Ready to receive events"),
        SetupStripeGateBlocking => (
            "El servidor no pudo abrir su puerto: no llegará ningún evento",
            "The server could not open its port: no events will arrive",
        ),
        SetupStripePanelBlocking => (
            "El servidor no está escuchando. Cambiá host o puerto con C",
            "The server is not listening. Change host or port with C",
        ),
        SetupKeyChoose => ("elegir", "choose"),
        SetupKeyRecheck => ("re-verificar", "re-check"),
        SetupKeyContinueAnyway => ("continuar igual", "continue anyway"),
        SetupKeyExit => ("salir", "exit"),

        DepServerLabel => ("Servidor", "Server"),
        DepServerOkTemplate => ("escuchando en {}", "listening on {}"),
        DepServerErrTemplate => ("no se pudo abrir {}", "could not open {}"),
        DepTokenLabel => ("Token", "Token"),
        DepTokenConfig => ("configurado en la config", "set in the config"),
        DepTokenLegacy => ("tomado de ~/.config/claude-watch/token", "taken from ~/.config/claude-watch/token"),
        DepTokenNone => (
            "sin token: cualquiera en la red puede enviar eventos (generalo con --token)",
            "no token: anyone on the network can send events (create one with --token)",
        ),
        DepStatsLabel => ("Estadísticas", "Statistics"),
        DepStatsErrTemplate => ("no se puede escribir en {}", "cannot write to {}"),
        DepStatsNoDir => ("sin directorio de datos: no se guardan", "no data directory: not saved"),
        DepModLabel => ("Mod", "Mod"),
        DepModTemplate => ("en Claude Code: CLAUDE_WATCH_URL={}", "in Claude Code: CLAUDE_WATCH_URL={}"),
        DepModIpUnknown => ("<ip-de-este-equipo>", "<this-machine-ip>"),
        DepEventsLabel => ("Eventos", "Events"),
        DepEventsOkTemplate => ("{} recibidos", "{} received"),
        DepEventsNone => ("ninguno todavía", "none yet"),
        DepSoundLabel => ("Sonido", "Sound"),
        DepSoundOkTemplate => ("activado, se reproduce con {}", "on, played with {}"),
        DepSoundMuted => ("silenciado (V para activarlo)", "muted (V to turn it on)"),
        DepSoundNone => (
            "sin reproductor: instalá pipewire, pulseaudio-utils o alsa-utils",
            "no player found: install pipewire, pulseaudio-utils or alsa-utils",
        ),
        DepDemoLabel => ("Demo", "Demo"),
        DepDemoOn => ("activa: hay una sesión simulada", "on: a simulated session is running"),
        DepDemoOff => ("apagada (D o --demo para verla)", "off (D or --demo to see it)"),
    }
}

/// Nombre visible de cada estado de Claude.
pub fn state_label(lang: Lang, state: State) -> &'static str {
    use State::*;
    let (es, en) = match state {
        Idle => ("En espera", "Idle"),
        Sleeping => ("Durmiendo", "Sleeping"),
        Offline => ("Sin sesiones", "No sessions"),
        Working => ("Trabajando", "Working"),
        Thinking => ("Pensando", "Thinking"),
        Answering => ("Respondiendo", "Answering"),
        Reading => ("Leyendo", "Reading"),
        Searching => ("Buscando", "Searching"),
        Editing => ("Editando", "Editing"),
        Running => ("Ejecutando", "Running"),
        Web => ("En la web", "On the web"),
        Delegating => ("Delegando a subagentes", "Delegating to subagents"),
        Asking => ("Te está preguntando", "Asking you"),
        Planning => ("Planeando", "Planning"),
        PlanReady => ("Plan listo, esperando aprobación", "Plan ready, awaiting approval"),
        Organizing => ("Organizando tareas", "Organizing tasks"),
        Mcp => ("Usando MCP", "Using MCP"),
        Tool => ("Usando herramienta", "Using a tool"),
        Done => ("¡Terminado!", "Done!"),
        Error => ("Algo falló", "Something failed"),
    };
    match lang {
        Lang::Es => es,
        Lang::En => en,
    }
}

/// Abreviatura del día de la semana (0 = lunes).
pub fn weekday(lang: Lang, index: u32) -> &'static str {
    const ES: [&str; 7] = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"];
    const EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let i = (index as usize).min(6);
    match lang {
        Lang::Es => ES[i],
        Lang::En => EN[i],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_no_vuelve_a_rellenar_lo_insertado() {
        assert_eq!(fill("Subagente {}: {}", &["raro{}", "revisar"]), "Subagente raro{}: revisar");
        assert_eq!(fill("{} y {}", &["uno"]), "uno y {}");
        assert_eq!(fill("sin huecos", &["x"]), "sin huecos");
    }
}
