use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Uso de un límite de la cuenta (ventana de 5 h o de 7 días).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Limit {
    pub percent: Option<f64>,
    /// Momento en que se reinicia, en segundos Unix.
    pub resets_at: Option<i64>,
}

/// Ocupación de la ventana de contexto de la sesión.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Context {
    pub percent: Option<f64>,
    pub tokens: Option<u64>,
    pub window: Option<u64>,
}

/// Última medida conocida de una sesión. Cada evento `measure` trae sólo lo
/// que cambió: los campos ausentes conservan su valor anterior.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Measure {
    pub five_hour: Option<Limit>,
    pub seven_day: Option<Limit>,
    pub context: Option<Context>,
    pub usd: Option<f64>,
}

impl Measure {
    /// Lee el objeto `measure` del mod (`fiveHour`, `sevenDay`, `context`, `usd`).
    pub fn from_json(v: &Value) -> Self {
        let context = v.get("context").map(|c| Context {
            percent: c.get("percent").and_then(Value::as_f64),
            tokens: c.get("tokens").and_then(Value::as_f64).map(|t| t as u64),
            window: c.get("window").and_then(Value::as_f64).map(|w| w as u64),
        });
        Self {
            five_hour: limit(v.get("fiveHour")),
            seven_day: limit(v.get("sevenDay")),
            // Un contexto sin porcentaje no aporta nada que mostrar.
            context: context.filter(|c| c.percent.is_some()),
            usd: v.get("usd").and_then(Value::as_f64),
        }
    }

    pub fn merge(&mut self, newer: &Measure) {
        if newer.five_hour.is_some() {
            self.five_hour = newer.five_hour;
        }
        if newer.seven_day.is_some() {
            self.seven_day = newer.seven_day;
        }
        if newer.context.is_some() {
            self.context = newer.context;
        }
        if newer.usd.is_some() {
            self.usd = newer.usd;
        }
    }
}

fn limit(v: Option<&Value>) -> Option<Limit> {
    let v = v.filter(|v| v.is_object())?;
    Some(Limit {
        percent: v.get("percent").and_then(Value::as_f64),
        resets_at: v.get("resetsAt").and_then(resets_at),
    })
}

/// `resetsAt` llega como epoch (en segundos o milisegundos) o como fecha ISO.
fn resets_at(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => {
            let n = n.as_f64()?;
            if n <= 0.0 {
                None
            } else if n > 1e11 {
                Some((n / 1000.0) as i64)
            } else {
                Some(n as i64)
            }
        }
        Value::String(s) => DateTime::parse_from_rfc3339(s).ok().map(|d| d.timestamp()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lee_limites_en_ms_segundos_e_iso() {
        let m = Measure::from_json(&json!({
            "fiveHour": {"percent": 42, "resetsAt": 1_800_000_000_000_i64},
            "sevenDay": {"percent": 18, "resetsAt": "2027-01-15T10:00:00Z"},
            "context": {"percent": 23, "tokens": 46000, "window": 200000},
            "usd": 0.4
        }));
        assert_eq!(m.five_hour.unwrap().resets_at, Some(1_800_000_000));
        assert_eq!(m.seven_day.unwrap().resets_at, Some(1_800_007_200));
        assert_eq!(m.context.unwrap().tokens, Some(46000));
        assert_eq!(m.usd, Some(0.4));
    }

    #[test]
    fn merge_conserva_lo_que_no_llega() {
        let mut m = Measure::from_json(&json!({"fiveHour": {"percent": 10}, "usd": 1.0}));
        m.merge(&Measure::from_json(&json!({"usd": 2.0, "fiveHour": null, "context": {"percent": null}})));
        assert_eq!(m.five_hour.unwrap().percent, Some(10.0));
        assert_eq!(m.usd, Some(2.0));
        assert!(m.context.is_none());
    }
}
