use crossterm::event::{KeyCode, KeyEvent};

use crate::deps::{self, DepReport, Probe};
use crate::i18n::Lang;

/// `Gate` es el wizard a pantalla completa que corre antes del tablero. El mismo
/// estado se reutiliza dentro de la pestaña `VERIFICACIÓN`, pero ahí la
/// navegación la maneja `App` directamente, sin pasar por `on_key`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemCheckMode {
    Gate,
}

#[derive(Debug, Clone)]
pub enum SystemCheckOutcome {
    None,
    Recheck,
    /// Sólo en modo `Gate`: entrar al tablero igual.
    Continue,
    /// Sólo en modo `Gate`: cancelar y terminar el proceso.
    Cancel,
}

#[derive(Debug, Clone)]
pub struct SystemCheckState {
    pub report: DepReport,
    pub selected: usize,
    pub lang: Lang,
    probe: Probe,
}

impl SystemCheckState {
    pub fn new(lang: Lang, probe: Probe) -> Self {
        Self { report: deps::probe(lang, &probe), selected: 0, lang, probe }
    }

    pub fn recheck(&mut self, probe: Probe) {
        self.report = deps::probe(self.lang, &probe);
        self.probe = probe;
        self.selected = self.selected.min(self.report.checks.len().saturating_sub(1));
    }

    /// Cambia el idioma y re-corre el probe para regenerar los textos.
    pub fn set_lang(&mut self, lang: Lang) {
        if self.lang != lang {
            self.lang = lang;
            self.report = deps::probe(lang, &self.probe);
        }
    }

    pub fn on_key(&mut self, key: KeyEvent, mode: SystemCheckMode) -> SystemCheckOutcome {
        match key.code {
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                SystemCheckOutcome::None
            }
            KeyCode::Down => {
                self.selected = (self.selected + 1).min(self.report.checks.len().saturating_sub(1));
                SystemCheckOutcome::None
            }
            KeyCode::Char('r') => SystemCheckOutcome::Recheck,
            KeyCode::Char('a') if mode == SystemCheckMode::Gate => SystemCheckOutcome::Continue,
            KeyCode::Esc if mode == SystemCheckMode::Gate => SystemCheckOutcome::Cancel,
            _ => SystemCheckOutcome::None,
        }
    }
}
