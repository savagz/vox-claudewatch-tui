mod app;
mod beacon;
mod config;
mod deps;
mod event;
mod i18n;
mod keys;
mod logging;
mod mascot;
mod setup;
mod sound;
mod theme;
mod ui;

use std::path::PathBuf;
use std::time::Duration;

use app::{App, PendingAction, Reload};
use beacon::server::{self, ServerStatus};
use beacon::stats::Stats;
use color_eyre::Result;
use config::Config;
use crossterm::event::{Event, EventStream};
use deps::Probe;
use event::AppEvent;
use futures_util::StreamExt;
use i18n::Key;
use setup::{SystemCheckMode, SystemCheckOutcome, SystemCheckState};
use sound::Player;
use theme::{Status, Theme};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

const USAGE: &str = "voxclaudewatch-tui: actividad de Claude Code en vivo, enviada por el mod activity-beacon

USO:
    voxclaudewatch-tui [OPCIONES]

OPCIONES:
    --demo           Arranca con una sesión simulada para ver todos los estados
    --token          Muestra el token que debe usar el mod (lo genera si no hay)
    -V, --version    Muestra la versión y sale
    -h, --help       Muestra esta ayuda y sale

ENTORNO:
    VOXCLAUDEWATCH_DEMO=1    Equivale a --demo";

enum Cli {
    Exit,
    Run { demo: bool },
}

fn handle_cli_args() -> Cli {
    let env_demo = std::env::var("VOXCLAUDEWATCH_DEMO").is_ok_and(|v| v == "1");
    match std::env::args().nth(1).as_deref() {
        Some("-V" | "--version") => {
            println!("voxclaudewatch-tui {}", env!("CARGO_PKG_VERSION"));
            Cli::Exit
        }
        Some("-h" | "--help") => {
            println!("{USAGE}");
            Cli::Exit
        }
        Some("--token") => {
            print_token();
            Cli::Exit
        }
        Some("--demo") => Cli::Run { demo: true },
        Some(other) => {
            eprintln!("opción desconocida: {other}\n\n{USAGE}");
            Cli::Exit
        }
        None => Cli::Run { demo: env_demo },
    }
}

/// Imprime el token efectivo. Si no hay ninguno, genera uno y lo guarda en la
/// config para que la TUI y el mod usen el mismo.
fn print_token() {
    let mut config = match Config::load() {
        Ok(c) => c.unwrap_or_default(),
        Err(e) => {
            eprintln!("{e:#}");
            return;
        }
    };
    match config.ensure_token() {
        Ok(saved) => {
            println!("{}", config.effective_token());
            if let Some(path) = saved {
                eprintln!("guardado en {}", path.display());
            }
        }
        Err(e) => eprintln!("{e:#}"),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let demo = match handle_cli_args() {
        Cli::Exit => return Ok(()),
        Cli::Run { demo } => demo,
    };
    color_eyre::install()?;
    let _log_guard = logging::init()?;

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, demo).await;
    ratatui::restore();
    result
}

fn stats_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "voxclaudewatch-tui").map(|d| d.data_dir().join("stats.json"))
}

async fn run(terminal: &mut ratatui::DefaultTerminal, demo: bool) -> Result<()> {
    let (tx, rx) = mpsc::channel::<AppEvent>(100);
    let input = spawn_input_task(tx.clone());
    spawn_signal_task(tx.clone());

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        user = %std::env::var("USER").unwrap_or_default(),
        demo,
        "inicio de sesión"
    );

    let (mut config, notice) = match Config::load() {
        Ok(Some(config)) => (config, None),
        Ok(None) => (Config::default(), None),
        Err(e) => {
            let message = format!("{e:#}");
            tracing::error!(error = %message, "no se pudo cargar la configuración");
            let lang = i18n::Lang::default();
            (Config::default(), Some((i18n::t(lang, Key::MainConfigLoadErrTemplate).replacen("{}", &message, 1), Status::Error)))
        }
    };
    // Sin token cualquiera en la red podría mandar eventos. Con la config rota
    // no se genera: guardarlo pisaría el archivo que el usuario debe arreglar.
    let notice = if notice.is_some() { notice } else { ensure_token(&mut config) };

    let player = Player::detect();
    let stats_path = stats_path();
    let addr = server::address(&config);
    let mut listener = server::bind(&addr);
    let probe = |listener: &Result<std::net::TcpListener, String>, config: &Config| Probe {
        server: listener.as_ref().map(|_| addr.clone()).map_err(|e| format!("{addr}: {e}")),
        token: Probe::token_source(config),
        port: config.server.port,
        stats_path: stats_path.clone(),
        received: 0,
        demo,
        player: player.name(),
        sound_on: config.ui.sound,
    };

    // El wizard sólo aparece si hay algo en rojo: en la práctica, que el
    // puerto esté ocupado. `r` reintenta abrirlo.
    let mut system_check = SystemCheckState::new(config.ui.language, probe(&listener, &config));
    let mut rx = rx;
    if !system_check.report.all_ok() {
        let theme = Theme::default();
        loop {
            terminal.draw(|frame| ui::setup::render_gate(frame, frame.area(), &system_check, &theme))?;
            let Some(event) = rx.recv().await else {
                return Ok(());
            };
            match event {
                AppEvent::Key(key) => match system_check.on_key(key, SystemCheckMode::Gate) {
                    SystemCheckOutcome::None => {}
                    SystemCheckOutcome::Recheck => {
                        listener = server::bind(&addr);
                        system_check.recheck(probe(&listener, &config));
                        if system_check.report.all_ok() {
                            break;
                        }
                    }
                    SystemCheckOutcome::Continue => break,
                    SystemCheckOutcome::Cancel => return Ok(()),
                },
                AppEvent::Shutdown => return Ok(()),
                _ => {}
            }
        }
    }

    let startup = Startup {
        config,
        notice,
        listener: listener.ok(),
        stats: Stats::new(stats_path),
        player,
        demo,
        system_check,
    };
    let result = run_app(terminal, startup, tx, rx).await;
    input.abort();
    tracing::info!("fin de sesión");
    result
}

/// Estado resuelto antes de entrar al panel principal.
struct Startup {
    config: Config,
    /// Aviso de arranque (config ilegible, token generado); va como toast.
    notice: Option<(String, Status)>,
    listener: Option<std::net::TcpListener>,
    stats: Stats,
    player: Player,
    demo: bool,
    system_check: SystemCheckState,
}

async fn run_app(
    terminal: &mut ratatui::DefaultTerminal,
    startup: Startup,
    tx: mpsc::Sender<AppEvent>,
    mut rx: mpsc::Receiver<AppEvent>,
) -> Result<()> {
    let Startup { config, notice, listener, stats, mut player, demo, system_check } = startup;
    let tick_rate = Duration::from_millis(config.ui.tick_rate_ms.max(50));
    let (config_tx, config_rx) = watch::channel(config.clone());

    let initial_status = listener.as_ref().map(|_| ServerStatus::Listening(server::address(&config)));
    spawn_tick_task(tx.clone(), tick_rate);
    server::spawn(listener, config_rx, tx.clone());
    if demo {
        beacon::demo::spawn(tx.clone());
    }

    let mut app = App::new(config, Reload { config: config_tx }, stats, initial_status, system_check);
    app.player = player.name();
    app.demo = demo;
    app.system_check.recheck(app.probe());
    app.toast = notice;

    terminal.draw(|frame| ui::draw(frame, &app))?;

    // Si dibujar falla (p. ej. tras un SIGHUP la terminal ya no existe) se sale
    // del bucle con el error, pero las estadísticas se guardan igual.
    let result: Result<()> = async {
        while let Some(event) = rx.recv().await {
            app.on_event(event);

            if let Some(PendingAction::StartDemo) = app.take_pending() {
                beacon::demo::spawn(tx.clone());
            }
            // De una ráfaga suena sólo el aviso más importante.
            let cue = app.hub.take_cues().into_iter().max_by_key(|c| c.priority());
            if let Some(cue) = cue
                && app.config.ui.sound
            {
                player.play(cue);
            }

            if app.should_quit {
                break;
            }
            terminal.draw(|frame| ui::draw(frame, &app))?;
        }
        Ok(())
    }
    .await;

    app.save_stats();
    result
}

fn ensure_token(config: &mut Config) -> Option<(String, Status)> {
    let lang = config.ui.language;
    match config.ensure_token() {
        Ok(None) => None,
        Ok(Some(path)) => {
            tracing::info!(path = %path.display(), "token generado");
            let path = path.display().to_string();
            Some((i18n::t(lang, Key::MainTokenGeneratedTemplate).replacen("{}", &path, 1), Status::Ok))
        }
        Err(e) => {
            let message = format!("{e:#}");
            tracing::error!(error = %message, "no se pudo generar el token");
            Some((i18n::t(lang, Key::MainTokenErrTemplate).replacen("{}", &message, 1), Status::Warning))
        }
    }
}

fn spawn_input_task(tx: mpsc::Sender<AppEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut stream = EventStream::new();
        while let Some(Ok(evt)) = stream.next().await {
            if let Event::Key(key) = evt
                && tx.send(AppEvent::Key(key)).await.is_err()
            {
                break;
            }
        }
    })
}

fn spawn_tick_task(tx: mpsc::Sender<AppEvent>, tick_rate: Duration) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tick_rate);
        loop {
            interval.tick().await;
            if tx.send(AppEvent::Tick).await.is_err() {
                break;
            }
        }
    });
}

/// SIGTERM, SIGHUP (se cerró la terminal) y un SIGINT enviado con `kill` no
/// llegan como KeyEvent de crossterm: sin este handler el proceso muere sin
/// restaurar la terminal ni guardar las estadísticas.
#[cfg(unix)]
fn spawn_signal_task(tx: mpsc::Sender<AppEvent>) {
    use tokio::signal::unix::{SignalKind, signal};
    tokio::spawn(async move {
        let (Ok(mut term), Ok(mut hup), Ok(mut int)) =
            (signal(SignalKind::terminate()), signal(SignalKind::hangup()), signal(SignalKind::interrupt()))
        else {
            return;
        };
        tokio::select! {
            _ = term.recv() => {}
            _ = hup.recv() => {}
            _ = int.recv() => {}
        }
        let _ = tx.send(AppEvent::Shutdown).await;
    });
}

#[cfg(not(unix))]
fn spawn_signal_task(tx: mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            let _ = tx.send(AppEvent::Shutdown).await;
        }
    });
}
