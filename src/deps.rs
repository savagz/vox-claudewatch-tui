use std::net::UdpSocket;
use std::path::PathBuf;

use crate::config::Config;
use crate::i18n::{self, Key, Lang};
use crate::theme::Status;

/// Una fila de la verificación del sistema. Sólo el servidor es bloqueante:
/// sin socket no llega ningún evento y el tablero quedaría vacío.
#[derive(Debug, Clone)]
pub struct DepCheck {
    pub label: &'static str,
    pub detail: String,
    pub status: Status,
}

#[derive(Debug, Clone)]
pub struct DepReport {
    pub checks: Vec<DepCheck>,
}

impl DepReport {
    /// Ningún chequeo en rojo: nada reclama atención inmediata.
    pub fn all_ok(&self) -> bool {
        self.checks.iter().all(|c| c.status != Status::Error)
    }

    pub fn get(&self, index: usize) -> Option<&DepCheck> {
        self.checks.get(index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSource {
    Config,
    Legacy,
    None,
}

/// Datos que necesita la verificación. Los arma quien la dispara (el wizard
/// de arranque o la pestaña), porque sólo ellos saben cómo está el servidor.
#[derive(Debug, Clone)]
pub struct Probe {
    /// `Ok(dirección)` si el socket está abierto, `Err(motivo)` si no.
    pub server: Result<String, String>,
    pub token: TokenSource,
    pub port: u16,
    pub stats_path: Option<PathBuf>,
    pub received: u64,
    pub demo: bool,
    /// Reproductor de sonido encontrado.
    pub player: Option<&'static str>,
    pub sound_on: bool,
    /// Notificador de escritorio encontrado.
    pub notifier: Option<&'static str>,
    pub notify_on: bool,
}

impl Probe {
    pub fn token_source(config: &Config) -> TokenSource {
        if !config.server.token.trim().is_empty() {
            TokenSource::Config
        } else if !config.effective_token().is_empty() {
            TokenSource::Legacy
        } else {
            TokenSource::None
        }
    }
}

/// Corre los chequeos. Se dispara sólo por tecla del usuario (abrir el wizard,
/// `r` en la pestaña), nunca en el loop caliente.
pub fn probe(lang: Lang, p: &Probe) -> DepReport {
    let t = |k: Key| i18n::t(lang, k);
    let mut checks = Vec::with_capacity(7);

    checks.push(match &p.server {
        Ok(addr) => DepCheck {
            label: t(Key::DepServerLabel),
            detail: t(Key::DepServerOkTemplate).replacen("{}", addr, 1),
            status: Status::Ok,
        },
        Err(e) => DepCheck {
            label: t(Key::DepServerLabel),
            detail: t(Key::DepServerErrTemplate).replacen("{}", e, 1),
            status: Status::Error,
        },
    });

    checks.push(DepCheck {
        label: t(Key::DepTokenLabel),
        detail: match p.token {
            TokenSource::Config => t(Key::DepTokenConfig).to_string(),
            TokenSource::Legacy => t(Key::DepTokenLegacy).to_string(),
            TokenSource::None => t(Key::DepTokenNone).to_string(),
        },
        status: if p.token == TokenSource::None { Status::Warning } else { Status::Ok },
    });

    checks.push(match &p.stats_path {
        Some(path) if writable(path) => DepCheck {
            label: t(Key::DepStatsLabel),
            detail: path.display().to_string(),
            status: Status::Ok,
        },
        Some(path) => DepCheck {
            label: t(Key::DepStatsLabel),
            detail: t(Key::DepStatsErrTemplate).replacen("{}", &path.display().to_string(), 1),
            status: Status::Warning,
        },
        None => DepCheck {
            label: t(Key::DepStatsLabel),
            detail: t(Key::DepStatsNoDir).to_string(),
            status: Status::Warning,
        },
    });

    let ip = local_ip().unwrap_or_else(|| t(Key::DepModIpUnknown).to_string());
    checks.push(DepCheck {
        label: t(Key::DepModLabel),
        detail: t(Key::DepModTemplate).replacen("{}", &format!("http://{ip}:{}", p.port), 1),
        status: Status::Info,
    });

    checks.push(DepCheck {
        label: t(Key::DepEventsLabel),
        detail: if p.received > 0 {
            t(Key::DepEventsOkTemplate).replacen("{}", &p.received.to_string(), 1)
        } else {
            t(Key::DepEventsNone).to_string()
        },
        status: if p.received > 0 { Status::Ok } else { Status::Info },
    });

    checks.push(DepCheck {
        label: t(Key::DepSoundLabel),
        detail: match (p.player, p.sound_on) {
            (None, _) => t(Key::DepSoundNone).to_string(),
            (Some(_), false) => t(Key::DepSoundMuted).to_string(),
            (Some(player), true) => t(Key::DepSoundOkTemplate).replacen("{}", player, 1),
        },
        status: match (p.player, p.sound_on) {
            (None, true) => Status::Warning,
            (Some(_), true) => Status::Ok,
            (_, false) => Status::Info,
        },
    });

    checks.push(DepCheck {
        label: t(Key::DepNotifyLabel),
        detail: match (p.notifier, p.notify_on) {
            (None, _) => t(Key::DepNotifyNone).to_string(),
            (Some(_), false) => t(Key::DepNotifyOff).to_string(),
            (Some(notifier), true) => t(Key::DepNotifyOkTemplate).replacen("{}", notifier, 1),
        },
        status: match (p.notifier, p.notify_on) {
            (None, true) => Status::Warning,
            (Some(_), true) => Status::Ok,
            (_, false) => Status::Info,
        },
    });

    checks.push(DepCheck {
        label: t(Key::DepDemoLabel),
        detail: if p.demo { t(Key::DepDemoOn) } else { t(Key::DepDemoOff) }.to_string(),
        status: Status::Info,
    });

    DepReport { checks }
}

/// Comprueba que se puede crear el directorio y escribir en él.
fn writable(path: &std::path::Path) -> bool {
    let Some(dir) = path.parent() else {
        return false;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".write-test");
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(probe);
    ok
}

/// IP de la interfaz con ruta por defecto. `connect` en UDP no envía nada:
/// sólo resuelve qué dirección local usaría el sistema.
fn local_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    socket.local_addr().ok().map(|a| a.ip().to_string()).filter(|ip| ip != "0.0.0.0")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Probe {
        Probe {
            server: Ok("0.0.0.0:8765".into()),
            token: TokenSource::Config,
            port: 8765,
            stats_path: None,
            received: 0,
            demo: false,
            player: None,
            sound_on: true,
            notifier: None,
            notify_on: false,
        }
    }

    #[test]
    fn solo_el_servidor_bloquea() {
        assert!(probe(Lang::Es, &base()).all_ok());
        let mut p = base();
        p.token = TokenSource::None;
        assert!(probe(Lang::Es, &p).all_ok());
        p.server = Err("Address already in use".into());
        assert!(!probe(Lang::En, &p).all_ok());
    }
}
