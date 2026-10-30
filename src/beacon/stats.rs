use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;

use chrono::Local;
use serde::{Deserialize, Serialize};

use super::measure::{Context, Limit};

/// Minutos de actividad que se recuerdan para el sparkline.
const ACTIVITY_MINUTES: usize = 60;
/// Tope de archivos distintos por lista, para que el JSON no crezca sin límite.
const MAX_FILES: usize = 5000;

/// Gasto de una sesión: el mod envía el total acumulado y el gasto del día es
/// `last - base` (`base` es lo que la sesión ya había gastado antes de hoy o
/// antes de empezar a vigilarla).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Cost {
    pub base: f64,
    pub last: f64,
    pub project: String,
}

/// Contadores del día. Se reinician a medianoche.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Day {
    pub date: String,
    pub turns: u64,
    pub turn_ms: u64,
    pub aborted: u64,
    pub tools: BTreeMap<String, u64>,
    pub tool_errors: u64,
    pub tool_ms: u64,
    pub files_read: Vec<String>,
    pub files_edited: Vec<String>,
    pub bash: u64,
    pub questions: u64,
    pub plans: u64,
    pub agents: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub waiting_ms: u64,
    pub usd: BTreeMap<String, Cost>,
    pub tasks_done: u64,
}

impl Day {
    fn today() -> Self {
        Self { date: today(), ..Self::default() }
    }

    pub fn tools_total(&self) -> u64 {
        self.tools.values().sum()
    }

    /// Herramientas más usadas, de mayor a menor.
    pub fn top_tools(&self, n: usize) -> Vec<(&str, u64)> {
        let mut tools: Vec<(&str, u64)> = self.tools.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        tools.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        tools.truncate(n);
        tools
    }

    /// Porcentaje de tokens de entrada servidos desde caché.
    pub fn cache_hit(&self) -> Option<f64> {
        let total = self.tokens_in + self.cache_read + self.cache_write;
        (total > 0).then(|| self.cache_read as f64 / total as f64 * 100.0)
    }
}

/// Última lectura de límites y contexto. Son de la cuenta, no del día, y
/// sirven para seguir mostrando algo cuando no hay sesiones vivas.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Last {
    pub five_hour: Option<Limit>,
    pub seven_day: Option<Limit>,
    pub limits_at: Option<f64>,
    pub context: Option<Context>,
    pub context_project: String,
    pub context_at: Option<f64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct StatsFile {
    day: Day,
    last: Last,
}

pub struct Stats {
    pub day: Day,
    pub last: Last,
    /// (minuto Unix, herramientas usadas en ese minuto).
    minutes: VecDeque<(i64, u32)>,
    /// `None` en tests: no se lee ni se escribe nada en disco.
    path: Option<PathBuf>,
}

impl Stats {
    pub fn new(path: Option<PathBuf>) -> Self {
        let mut stats = Self {
            day: Day::today(),
            last: Last::default(),
            minutes: VecDeque::with_capacity(ACTIVITY_MINUTES),
            path,
        };
        stats.load();
        stats
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    fn load(&mut self) {
        let Some(path) = &self.path else {
            return;
        };
        let Ok(raw) = std::fs::read_to_string(path) else {
            return;
        };
        match serde_json::from_str::<StatsFile>(&raw) {
            Ok(saved) => {
                self.last = saved.last;
                self.day = saved.day;
                // Aún no se sabe qué sesiones siguen vivas: se conservan todas.
                self.roll(|_| true);
            }
            Err(e) => tracing::warn!(error = %e, path = %path.display(), "estadísticas ilegibles, se empieza de cero"),
        }
    }

    /// Escritura atómica (archivo temporal + rename) para no dejar el JSON a medias.
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let body = serde_json::to_string(&StatsFile { day: self.day.clone(), last: self.last.clone() })?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, body)?;
        std::fs::rename(&tmp, path)
    }

    /// Cambio de día: contadores a cero y lo ya gastado por cada sesión pasa a
    /// ser su base, para que no cuente para hoy. Sólo pasan las sesiones que
    /// cumplen `keep`; si una descartada vuelve, su primera medida se toma
    /// como base igual (ver `record_cost`), así que no se infla el coste.
    pub fn roll(&mut self, keep: impl Fn(&str) -> bool) {
        let today = today();
        if self.day.date == today {
            return;
        }
        let usd = std::mem::take(&mut self.day.usd);
        self.day = Day::today();
        self.day.usd = usd
            .into_iter()
            .filter(|(sid, _)| keep(sid))
            .map(|(sid, c)| (sid, Cost { base: c.last, ..c }))
            .collect();
    }

    /// Guarda el total acumulado de una sesión. `fresh` indica que la sesión
    /// acaba de empezar: si no, su gasto anterior es de antes de vigilarla.
    pub fn record_cost(&mut self, sid: &str, project: &str, usd: f64, fresh: bool) {
        match self.day.usd.get_mut(sid) {
            None => {
                let base = if fresh { 0.0 } else { usd };
                self.day.usd.insert(sid.to_string(), Cost { base, last: usd, project: project.to_string() });
            }
            Some(c) => {
                if usd < c.last {
                    // El contador de la sesión volvió a empezar (resume): conserva lo ya sumado.
                    c.base -= c.last;
                }
                c.last = usd;
                c.project = project.to_string();
            }
        }
    }

    pub fn day_cost(&self) -> f64 {
        self.day.usd.values().map(|c| (c.last - c.base).max(0.0)).sum()
    }

    pub fn cost_by_project(&self) -> Vec<(String, f64)> {
        let mut totals: BTreeMap<&str, f64> = BTreeMap::new();
        for c in self.day.usd.values() {
            *totals.entry(c.project.as_str()).or_default() += (c.last - c.base).max(0.0);
        }
        let mut out: Vec<(String, f64)> =
            totals.into_iter().filter(|(_, v)| *v > 0.0).map(|(p, v)| (p.to_string(), v)).collect();
        out.sort_by(|a, b| b.1.total_cmp(&a.1));
        out
    }

    pub fn tick_activity(&mut self, now: f64) {
        let minute = (now / 60.0) as i64;
        match self.minutes.back_mut() {
            Some((m, n)) if *m == minute => *n += 1,
            _ => {
                if self.minutes.len() == ACTIVITY_MINUTES {
                    self.minutes.pop_front();
                }
                self.minutes.push_back((minute, 1));
            }
        }
    }

    /// Herramientas por minuto en los últimos `width` minutos, del más viejo al actual.
    pub fn activity(&self, width: usize, now: f64) -> Vec<u32> {
        let current = (now / 60.0) as i64;
        (0..width as i64)
            .rev()
            .map(|ago| {
                let minute = current - ago;
                self.minutes.iter().find(|(m, _)| *m == minute).map_or(0, |(_, n)| *n)
            })
            .collect()
    }

    pub fn add_file(list: &mut Vec<String>, file: &str) {
        if !file.is_empty() && !list.iter().any(|f| f == file) {
            list.push(file.to_string());
            if list.len() > MAX_FILES {
                list.drain(..list.len() - MAX_FILES);
            }
        }
    }
}

fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coste_del_dia_descuenta_lo_anterior() {
        let mut s = Stats::new(None);
        // Sesión vista a mitad de vida: lo gastado antes no cuenta.
        s.record_cost("a", "web", 2.0, false);
        s.record_cost("a", "web", 2.5, false);
        // Sesión nueva: cuenta desde cero.
        s.record_cost("b", "api", 0.3, true);
        assert!((s.day_cost() - 0.8).abs() < 1e-9);
        // Resume: el contador de la sesión vuelve a empezar.
        s.record_cost("a", "web", 0.1, false);
        assert!((s.day_cost() - 0.9).abs() < 1e-9);
        assert_eq!(s.cost_by_project()[0].0, "web");
    }

    #[test]
    fn cambio_de_dia_convierte_el_gasto_en_base() {
        let mut s = Stats::new(None);
        s.record_cost("a", "web", 1.0, true);
        s.day.turns = 7;
        s.day.date = "2000-01-01".into();
        s.roll(|_| true);
        assert_eq!(s.day.turns, 0);
        assert_eq!(s.day_cost(), 0.0);
        s.record_cost("a", "web", 1.5, false);
        assert!((s.day_cost() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn cambio_de_dia_olvida_las_sesiones_que_ya_no_existen() {
        let mut s = Stats::new(None);
        s.record_cost("viva", "web", 1.0, true);
        s.record_cost("vieja", "api", 2.0, true);
        s.day.date = "2000-01-01".into();
        s.roll(|sid| sid == "viva");
        assert_eq!(s.day.usd.keys().collect::<Vec<_>>(), vec!["viva"]);
        // Si la vieja reaparece, lo gastado antes no cuenta para hoy.
        s.record_cost("vieja", "api", 2.5, false);
        assert_eq!(s.day_cost(), 0.0);
    }

    #[test]
    fn actividad_por_minuto() {
        let mut s = Stats::new(None);
        let now = 6000.0;
        s.tick_activity(now - 120.0);
        s.tick_activity(now);
        s.tick_activity(now + 1.0);
        assert_eq!(s.activity(3, now + 1.0), vec![1, 0, 2]);
    }

    #[test]
    fn guarda_y_vuelve_a_cargar() {
        let dir = std::env::temp_dir().join(format!("voxclaudewatch-test-{}", std::process::id()));
        let path = dir.join("stats.json");
        let mut s = Stats::new(Some(path.clone()));
        s.day.turns = 3;
        s.last.context_project = "demo".into();
        s.save().unwrap();
        let loaded = Stats::new(Some(path));
        assert_eq!(loaded.day.turns, 3);
        assert_eq!(loaded.last.context_project, "demo");
        let _ = std::fs::remove_dir_all(dir);
    }
}
