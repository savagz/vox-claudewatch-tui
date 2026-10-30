use chrono::{Datelike, Local, TimeZone};

use crate::i18n::{self, Key, Lang};

/// Duración compacta: `12s`, `3:05`, `1h 02m`.
pub fn dur(secs: f64) -> String {
    let secs = secs.max(0.0) as u64;
    let (h, rem) = (secs / 3600, secs % 3600);
    let (m, s) = (rem / 60, rem % 60);
    if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}:{s:02}")
    } else {
        format!("{s}s")
    }
}

/// Dólares con más decimales cuanto menor es la cifra.
pub fn money(v: Option<f64>) -> String {
    match v {
        None => "--".to_string(),
        Some(v) if v >= 1.0 => format!("${v:.2}"),
        Some(v) if v >= 0.01 => format!("${v:.3}"),
        Some(v) => format!("${v:.4}"),
    }
}

/// Cantidades grandes: `950`, `12.3k`, `4.1M`.
pub fn kilo(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1e6)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1e3)
    } else {
        n.to_string()
    }
}

pub fn ago(lang: Lang, ts: f64, now: f64) -> String {
    if now - ts < 2.0 {
        i18n::t(lang, Key::AgoNow).to_string()
    } else {
        i18n::t(lang, Key::AgoTemplate).replacen("{}", &dur(now - ts), 1)
    }
}

/// Hora local de reinicio de un límite: `18:30` si es hoy, `jue 09:00` si no.
pub fn reset_at(lang: Lang, epoch: i64) -> String {
    let Some(when) = Local.timestamp_opt(epoch, 0).single() else {
        return String::new();
    };
    if when.date_naive() == Local::now().date_naive() {
        when.format("%H:%M").to_string()
    } else {
        format!("{} {}", i18n::weekday(lang, when.weekday().num_days_from_monday()), when.format("%H:%M"))
    }
}

pub fn clock(ts: f64) -> String {
    Local.timestamp_opt(ts as i64, 0).single().map(|t| t.format("%H:%M:%S").to_string()).unwrap_or_default()
}

/// Recorta por la izquierda: lo útil de una ruta es el final.
pub fn short_path(path: &str, width: usize) -> String {
    let count = path.chars().count();
    if count <= width || width == 0 {
        return path.to_string();
    }
    let tail: String = path.chars().skip(count - (width - 1)).collect();
    format!("…{tail}")
}

/// Recorta por la derecha a `width` caracteres.
pub fn fit(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    let mut out: String = text.chars().take(width.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duraciones() {
        assert_eq!(dur(7.9), "7s");
        assert_eq!(dur(185.0), "3:05");
        assert_eq!(dur(3720.0), "1h 02m");
    }

    #[test]
    fn importes() {
        assert_eq!(money(None), "--");
        assert_eq!(money(Some(2.5)), "$2.50");
        assert_eq!(money(Some(0.123)), "$0.123");
        assert_eq!(money(Some(0.0042)), "$0.0042");
    }

    #[test]
    fn recortes() {
        assert_eq!(short_path("/home/user/proyecto/src/main.rs", 12), "…src/main.rs");
        assert_eq!(fit("abcdef", 4), "abc…");
        assert_eq!(kilo(46_000), "46.0k");
    }
}
