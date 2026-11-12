//! Notificaciones de escritorio para los avisos que te esperan: Claude te
//! pregunta, hay un plan listo, terminó el turno o hubo un error. Se envían
//! con el notificador del sistema (`osascript` en macOS, `notify-send` en el
//! resto), sin shell: el texto viaja siempre como argumento.

use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::beacon::{Alert, Cue, clip};
use crate::i18n::{self, Key, Lang};
use crate::sound::in_path;

/// El mismo aviso no se notifica dos veces antes de esto.
const SAME_CUE_GAP: Duration = Duration::from_secs(5);
const DETAIL_MAX: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backend {
    Osascript,
    NotifySend,
}

impl Backend {
    fn program(self) -> &'static str {
        match self {
            Backend::Osascript => "osascript",
            Backend::NotifySend => "notify-send",
        }
    }
}

pub struct Notifier {
    backend: Option<Backend>,
    last: HashMap<Cue, Instant>,
}

impl Notifier {
    pub fn detect() -> Self {
        let candidate = if cfg!(target_os = "macos") { Backend::Osascript } else { Backend::NotifySend };
        Self { backend: in_path(candidate.program()).then_some(candidate), last: HashMap::new() }
    }

    /// Nombre del notificador, o `None` si no hay forma de notificar.
    pub fn name(&self) -> Option<&'static str> {
        self.backend.map(Backend::program)
    }

    /// Decide si `cue` se notifica ahora y, si es así, lo anota.
    fn admit(&mut self, cue: Cue, now: Instant) -> bool {
        if !notifies(cue) || self.last.get(&cue).is_some_and(|t| now - *t < SAME_CUE_GAP) {
            return false;
        }
        self.last.insert(cue, now);
        true
    }

    pub fn notify(&mut self, lang: Lang, alert: &Alert) {
        let Some(backend) = self.backend else {
            return;
        };
        if !self.admit(alert.cue, Instant::now()) {
            return;
        }
        let Some((title, body)) = message(lang, alert) else {
            return;
        };
        let mut cmd = Command::new(backend.program());
        match backend {
            Backend::Osascript => cmd.args([
                "-e",
                "on run argv",
                "-e",
                "display notification (item 2 of argv) with title (item 1 of argv)",
                "-e",
                "end run",
                &title,
                &body,
            ]),
            Backend::NotifySend => cmd.args(["--app-name=VOXClaudeWatch", &title, &body]),
        };
        match cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            // Se espera en otro hilo para no dejar procesos zombis.
            Ok(mut child) => {
                std::thread::spawn(move || child.wait());
            }
            Err(e) => tracing::warn!(error = %e, program = backend.program(), "no se pudo enviar la notificación"),
        }
    }
}

/// Sólo lo que te espera o cierra un turno merece interrumpirte.
fn notifies(cue: Cue) -> bool {
    matches!(cue, Cue::Asking | Cue::PlanReady | Cue::Done | Cue::Error)
}

/// Título y cuerpo de la notificación, o `None` si el aviso no se notifica.
fn message(lang: Lang, alert: &Alert) -> Option<(String, String)> {
    let t = |k: Key| i18n::t(lang, k);
    let detail = clip(&alert.detail, DETAIL_MAX);
    let with_detail = |plain: Key, tpl: Key| {
        if detail.is_empty() { t(plain).to_string() } else { t(tpl).replacen("{}", &detail, 1) }
    };
    let body = match alert.cue {
        Cue::Asking => with_detail(Key::NotifyAsking, Key::NotifyAskingTemplate),
        Cue::PlanReady => t(Key::NotifyPlanReady).to_string(),
        Cue::Done => t(Key::NotifyDone).to_string(),
        Cue::Error => with_detail(Key::NotifyError, Key::NotifyErrorTemplate),
        _ => return None,
    };
    let title = if alert.project.is_empty() { "Claude".to_string() } else { format!("Claude · {}", alert.project) };
    Some((title, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alert(cue: Cue, detail: &str) -> Alert {
        Alert { cue, project: "demo".into(), detail: detail.into() }
    }

    #[test]
    fn no_repite_el_mismo_aviso_en_rafaga() {
        let mut n = Notifier { backend: None, last: HashMap::new() };
        let t0 = Instant::now();
        assert!(n.admit(Cue::Asking, t0));
        assert!(!n.admit(Cue::Asking, t0 + Duration::from_secs(2)));
        assert!(n.admit(Cue::Done, t0 + Duration::from_secs(2)));
        assert!(n.admit(Cue::Asking, t0 + Duration::from_secs(6)));
        // Los avisos de fondo nunca notifican.
        assert!(!n.admit(Cue::Prompt, t0));
        assert!(!n.admit(Cue::Delegating, t0));
    }

    #[test]
    fn cada_aviso_tiene_texto_en_ambos_idiomas() {
        for lang in [Lang::Es, Lang::En] {
            for cue in Cue::ALL {
                for detail in ["", "algo"] {
                    match message(lang, &alert(cue, detail)) {
                        Some((title, body)) => {
                            assert_eq!(title, "Claude · demo");
                            assert!(!body.is_empty() && !body.contains("{}"), "{cue:?} {lang:?}");
                            if !detail.is_empty() && matches!(cue, Cue::Asking | Cue::Error) {
                                assert!(body.contains(detail));
                            }
                        }
                        None => assert!(!notifies(cue)),
                    }
                }
            }
        }
    }
}
