use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::event::AppEvent;

use super::now;

/// Pasos de un turno simulado: (estado, detalle, herramienta que lo cierra).
const STEPS: [(&str, &str, Option<&str>); 15] = [
    ("thinking", "", None),
    ("planning", "", Some("EnterPlanMode")),
    ("organizing", "", Some("TaskCreate")),
    ("reading", "src/server/config.ts", Some("Read")),
    ("searching", "handleRequest", Some("Grep")),
    ("web", "https://docs.example.com/api", Some("WebFetch")),
    ("thinking", "", None),
    ("editing", "src/server/index.ts", Some("Edit")),
    ("delegating", "Explore: revisar tests", Some("Agent")),
    ("running", "npm test", Some("Bash")),
    ("asking", "¿Despliego a producción?", Some("AskUserQuestion")),
    ("plan-ready", "", Some("ExitPlanMode")),
    ("mcp", "mcp__github__create_pr", Some("mcp__github__create_pr")),
    ("error", "Bash falló", None),
    ("answering", "", None),
];

const TASKS: [&str; 3] = ["Leer la configuración", "Editar el servidor", "Ejecutar los tests"];

/// Generador xorshift: la demo sólo necesita variar las pausas, no azar de calidad.
struct Jitter(u64);

impl Jitter {
    fn new() -> Self {
        Self((now() * 1e6) as u64 | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Valor uniforme en `[lo, hi)`.
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (self.next() % 10_000) as f64 / 10_000.0 * (hi - lo)
    }
}

/// Simula una sesión de Claude Code en bucle, con los mismos eventos que
/// enviaría el mod, para ver todos los estados de Pixo sin un Claude real.
pub fn spawn(tx: mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        let _ = run(tx).await;
    });
}

async fn run(tx: mpsc::Sender<AppEvent>) -> Result<(), mpsc::error::SendError<AppEvent>> {
    let send = |kind: &str, extra: Value| {
        let mut ev = json!({"sid": "demo", "project": "demo-app", "cwd": "/home/demo/demo-app", "kind": kind, "ts": now() * 1000.0});
        if let (Some(ev), Some(extra)) = (ev.as_object_mut(), extra.as_object()) {
            ev.extend(extra.clone());
        }
        tx.send(AppEvent::Beacon(vec![ev]))
    };
    let tasks = |current: Option<usize>| -> Value {
        let list: Vec<Value> = TASKS
            .iter()
            .enumerate()
            .map(|(j, subject)| {
                let status = match current {
                    None => "completed",
                    Some(k) if j < k => "completed",
                    Some(k) if j == k => "in_progress",
                    Some(_) => "pending",
                };
                json!({"id": (j + 1).to_string(), "subject": subject, "status": status})
            })
            .collect();
        json!({"tasks": list})
    };

    let mut jitter = Jitter::new();
    let reset_5h = (now() + 7200.0) * 1000.0;
    let reset_7d = (now() + 4.0 * 86400.0) * 1000.0;
    send(
        "session_start",
        json!({
            "model": "claude-opus-5-5",
            "state": "idle",
            "measure": {
                "fiveHour": {"percent": 42, "resetsAt": reset_5h},
                "sevenDay": {"percent": 18, "resetsAt": reset_7d},
                "context": {"percent": 23, "tokens": 46000, "window": 200000},
                "usd": 0.4,
            },
        }),
    )
    .await?;

    let mut usd = 0.4;
    loop {
        send("turn_start", json!({"prompt": "Arregla el bug del servidor y añade tests", "model": "claude-opus-5-5"}))
            .await?;
        for (i, (state, detail, tool)) in STEPS.iter().enumerate() {
            send("state", json!({"state": state, "detail": detail})).await?;
            if *state == "delegating" {
                send("agent_start", json!({"agentId": "ag1", "type": "Explore", "description": "revisar tests"}))
                    .await?;
                send("agent_tool", json!({"agentId": "ag1", "tool": "Grep", "detail": "describe("})).await?;
            }
            tokio::time::sleep(Duration::from_secs_f64(jitter.range(2.5, 4.0))).await;
            if let Some(tool) = tool {
                let ms = jitter.range(80.0, 4000.0) as u64;
                send("tool_done", json!({"tool": tool, "detail": detail, "ok": *state != "error", "ms": ms})).await?;
            }
            if *state == "delegating" {
                send("agent_done", json!({"agentId": "ag1", "ms": 3000})).await?;
            }
            if i % 4 == 2 {
                send("tasks", tasks(Some(i / 4))).await?;
            }
            usd += 0.07;
            let n = i as f64;
            send(
                "measure",
                json!({"measure": {
                    "fiveHour": {"percent": 42.0 + n, "resetsAt": reset_5h},
                    "sevenDay": {"percent": 18, "resetsAt": reset_7d},
                    "context": {"percent": 23.0 + n * 3.0, "tokens": 46000.0 + n * 6000.0, "window": 200000},
                    "usd": usd,
                }}),
            )
            .await?;
        }
        send("tasks", tasks(None)).await?;
        send(
            "turn_done",
            json!({"ms": 48000, "usage": {
                "input_tokens": 3200,
                "output_tokens": 1800,
                "cache_read_input_tokens": 41000,
                "cache_creation_input_tokens": 2200,
            }}),
        )
        .await?;
        send("state", json!({"state": "done"})).await?;
        tokio::time::sleep(Duration::from_secs(8)).await;
    }
}
