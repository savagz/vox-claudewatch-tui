//! Sonidos de la mascota: jingles de 8 bits sintetizados al arrancar (sin
//! archivos ni dependencias) y reproducidos con el reproductor del sistema.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::beacon::Cue;

const RATE: u32 = 22_050;
const VOLUME: f32 = 0.22;
/// El mismo sonido no se repite antes de esto (ráfagas de eventos).
const SAME_CUE_GAP: Duration = Duration::from_millis(1200);
/// Entre dos sonidos distintos, para que no se pisen. Un aviso más importante
/// que el anterior no espera: no se pierde un "te pregunto" por un blip.
const ANY_CUE_GAP: Duration = Duration::from_millis(250);

/// Reproductores probados en orden: PipeWire, PulseAudio, ALSA, macOS.
const PLAYERS: [(&str, &[&str]); 4] = [("pw-play", &[]), ("paplay", &[]), ("aplay", &["-q"]), ("afplay", &[])];

#[derive(Debug, Clone, Copy)]
enum Wave {
    Square,
    Triangle,
    Sine,
    Saw,
}

/// (frecuencia en Hz, 0 = silencio; duración en ms; forma de onda).
type Note = (f32, u32, Wave);

const C4: f32 = 261.63;
const E4: f32 = 329.63;
const G4: f32 = 392.00;
const C5: f32 = 523.25;
const D5: f32 = 587.33;
const E5: f32 = 659.25;
const F5: f32 = 698.46;
const G5: f32 = 783.99;
const A5: f32 = 880.00;
const C6: f32 = 1046.50;
const D6: f32 = 1174.66;

/// Melodía de cada aviso. Las alegres suben, las malas bajan, las que te
/// esperan terminan en una nota alta y abierta.
fn melody(cue: Cue) -> Vec<Note> {
    use Wave::*;
    match cue {
        Cue::SessionStart => vec![(C5, 70, Triangle), (E5, 70, Triangle), (G5, 110, Triangle)],
        Cue::SessionEnd => vec![(G5, 70, Triangle), (E5, 70, Triangle), (C5, 140, Triangle)],
        Cue::Prompt => vec![(A5, 45, Square)],
        Cue::Asking => vec![(E5, 110, Sine), (C5, 110, Sine), (0.0, 40, Sine), (A5, 220, Sine)],
        Cue::PlanReady => vec![(D5, 80, Triangle), (F5, 80, Triangle), (A5, 80, Triangle), (D6, 220, Triangle)],
        Cue::Delegating => vec![(G5, 45, Square), (0.0, 30, Square), (G5, 45, Square), (C6, 70, Square)],
        Cue::Done => vec![(C5, 70, Square), (E5, 70, Square), (G5, 70, Square), (C6, 200, Square)],
        Cue::Error => vec![(G4, 110, Saw), (E4, 110, Saw), (C4, 260, Saw)],
    }
}

fn sample(wave: Wave, phase: f32) -> f32 {
    let p = phase.fract();
    match wave {
        Wave::Square => {
            if p < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Wave::Triangle => 4.0 * (p - 0.5).abs() - 1.0,
        Wave::Sine => (p * std::f32::consts::TAU).sin(),
        Wave::Saw => 2.0 * p - 1.0,
    }
}

/// PCM de 16 bits mono. Cada nota tiene ataque corto y caída, sin chasquidos.
fn synth(notes: &[Note]) -> Vec<i16> {
    let mut out = Vec::new();
    for &(freq, ms, wave) in notes {
        let n = (RATE * ms / 1000) as usize;
        let attack = (RATE as usize / 200).min(n / 2).max(1);
        // Las ondas con armónicos suenan más fuerte: se compensan.
        let gain = match wave {
            Wave::Square | Wave::Saw => 0.55,
            Wave::Triangle | Wave::Sine => 1.0,
        };
        for i in 0..n {
            if freq <= 0.0 {
                out.push(0);
                continue;
            }
            let env = if i < attack { i as f32 / attack as f32 } else { 1.0 - (i - attack) as f32 / (n - attack) as f32 };
            let v = sample(wave, freq * i as f32 / RATE as f32) * env.powf(1.5) * VOLUME * gain;
            out.push((v * i16::MAX as f32) as i16);
        }
    }
    out
}

fn wav(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&s.to_le_bytes());
    }
    b
}

pub(crate) fn in_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
        .unwrap_or(false)
}

pub struct Player {
    /// Reproductor encontrado y sus argumentos fijos.
    program: Option<(&'static str, &'static [&'static str])>,
    files: HashMap<Cue, PathBuf>,
    last: HashMap<Cue, Instant>,
    last_any: Option<(Instant, Cue)>,
}

impl Player {
    /// Busca un reproductor y escribe los WAV en el directorio de caché.
    pub fn detect() -> Self {
        let program = PLAYERS.into_iter().find(|(p, _)| in_path(p));
        let dir = directories::ProjectDirs::from("", "", "voxclaudewatch-tui").map(|d| d.cache_dir().join("sounds"));
        let files = match (program, dir) {
            (Some(_), Some(dir)) => write_sounds(&dir).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "no se pudieron escribir los sonidos");
                HashMap::new()
            }),
            _ => HashMap::new(),
        };
        Self { program: program.filter(|_| !files.is_empty()), files, last: HashMap::new(), last_any: None }
    }

    /// Nombre del reproductor, o `None` si no hay forma de sonar.
    pub fn name(&self) -> Option<&'static str> {
        self.program.map(|(p, _)| p)
    }

    /// Decide si `cue` suena ahora y, si suena, lo anota.
    fn admit(&mut self, cue: Cue, now: Instant) -> bool {
        let too_soon = self.last.get(&cue).is_some_and(|t| now - *t < SAME_CUE_GAP)
            || self.last_any.is_some_and(|(t, prev)| now - t < ANY_CUE_GAP && cue.priority() <= prev.priority());
        if too_soon {
            return false;
        }
        self.last.insert(cue, now);
        self.last_any = Some((now, cue));
        true
    }

    pub fn play(&mut self, cue: Cue) {
        let Some((program, args)) = self.program else {
            return;
        };
        if !self.files.contains_key(&cue) || !self.admit(cue, Instant::now()) {
            return;
        }
        let file = &self.files[&cue];
        match Command::new(program).args(args).arg(file).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            // Se espera en otro hilo para no dejar procesos zombis.
            Ok(mut child) => {
                std::thread::spawn(move || child.wait());
            }
            Err(e) => tracing::warn!(error = %e, program, "no se pudo reproducir el sonido"),
        }
    }
}

fn write_sounds(dir: &Path) -> std::io::Result<HashMap<Cue, PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let mut files = HashMap::new();
    for cue in Cue::ALL {
        let path = dir.join(format!("{}.wav", cue.as_str()));
        std::fs::write(&path, wav(&synth(&melody(cue))))?;
        files.insert(cue, path);
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_aviso_suena_distinto() {
        let all: Vec<Vec<i16>> = Cue::ALL.iter().map(|c| synth(&melody(*c))).collect();
        for (i, a) in all.iter().enumerate() {
            assert!(!a.is_empty());
            assert!(a.iter().any(|s| *s != 0));
            for b in &all[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn la_cabecera_wav_es_coherente() {
        let samples = synth(&melody(Cue::Done));
        let bytes = wav(&samples);
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(bytes.len(), 44 + samples.len() * 2);
        assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize, samples.len() * 2);
    }

    fn silent() -> Player {
        Player { program: None, files: HashMap::new(), last: HashMap::new(), last_any: None }
    }

    #[test]
    fn limita_las_rafagas_sin_perder_lo_importante() {
        let mut p = silent();
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        assert!(p.admit(Cue::Prompt, t0));
        // Más importante y casi a la vez: suena igual.
        assert!(p.admit(Cue::Asking, ms(50)));
        // Menos importante dentro del hueco: se descarta.
        assert!(!p.admit(Cue::Done, ms(100)));
        // Pasado el hueco suena, pero el mismo aviso no se repite antes de 1,2 s.
        assert!(p.admit(Cue::Done, ms(400)));
        assert!(!p.admit(Cue::Asking, ms(1000)));
        assert!(p.admit(Cue::Asking, ms(1300)));
    }

    #[test]
    fn escribe_wav_validos_en_disco() {
        let dir = std::env::temp_dir().join(format!("voxclaudewatch-sounds-{}", std::process::id()));
        let files = write_sounds(&dir).unwrap();
        assert_eq!(files.len(), Cue::ALL.len());
        for (cue, path) in &files {
            let bytes = std::fs::read(path).unwrap();
            assert_eq!(&bytes[..4], b"RIFF", "{cue:?}");
            assert_eq!(&bytes[8..16], b"WAVEfmt ", "{cue:?}");
            let riff = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
            assert_eq!(riff + 8, bytes.len(), "{cue:?}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Para oírlos: `cargo test --release suenan_todos -- --ignored --nocapture`.
    /// Deja los WAV en el directorio temporal y los reproduce uno tras otro.
    #[test]
    #[ignore]
    fn suenan_todos() {
        let dir = std::env::temp_dir().join("voxclaudewatch-sounds");
        let files = write_sounds(&dir).unwrap();
        let (program, args) = PLAYERS.into_iter().find(|(p, _)| in_path(p)).expect("no hay reproductor");
        for cue in Cue::ALL {
            println!("{cue:?}: {}", files[&cue].display());
            let status = Command::new(program).args(args).arg(&files[&cue]).status().unwrap();
            assert!(status.success(), "{program} falló con {cue:?}");
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    #[test]
    fn no_satura() {
        for cue in Cue::ALL {
            let peak = synth(&melody(cue)).iter().map(|s| s.unsigned_abs()).max().unwrap();
            assert!(peak < i16::MAX as u16 / 3, "{cue:?}: pico {peak}");
        }
    }
}
