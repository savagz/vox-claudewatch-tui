pub mod activity;
pub mod now;
pub mod sessions;
pub mod stats;
pub mod syscheck;

use crate::i18n::{self, Key, Lang};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Now,
    Sessions,
    Stats,
    Activity,
    SystemCheck,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Now,
        Tab::Sessions,
        Tab::Stats,
        Tab::Activity,
        Tab::SystemCheck,
    ];

    pub fn title(&self, lang: Lang) -> &'static str {
        let key = match self {
            Tab::Now => Key::TabNow,
            Tab::Sessions => Key::TabSessions,
            Tab::Stats => Key::TabStats,
            Tab::Activity => Key::TabActivity,
            Tab::SystemCheck => Key::TabSystemCheck,
        };
        i18n::t(lang, key)
    }

    pub fn index(&self) -> usize {
        Self::ALL.iter().position(|t| t == self).expect("Tab siempre está en Tab::ALL")
    }

    pub fn from_index(index: usize) -> Option<Tab> {
        Self::ALL.get(index).copied()
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}
