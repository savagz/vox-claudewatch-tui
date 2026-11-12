//! Mascotas pixel art del tablero: Pixo (blob coral), Byte (robot), Michi (gato),
//! Axo (ajolote), Nopal (cactus) y Bruma (fantasma). Comparten cara y accesorios
//! por estado; cambia el cuerpo y la paleta.
//!
//! Se dibuja en una rejilla de `W` x `H` píxeles y se pinta con medios bloques
//! (cada celda son dos píxeles verticales), así que ocupa `W` columnas y
//! `H / 2` filas. Cada estado compone el cuerpo base con accesorios y varía
//! según el cuadro de animación.

use std::f64::consts::{PI, TAU};

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use serde::{Deserialize, Serialize};

use crate::beacon::State;
use crate::i18n::Lang;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mascot {
    #[default]
    Pixo,
    Byte,
    Michi,
    Axo,
    Nopal,
    Bruma,
}

impl Mascot {
    pub const ALL: [Mascot; 6] = [Mascot::Pixo, Mascot::Byte, Mascot::Michi, Mascot::Axo, Mascot::Nopal, Mascot::Bruma];

    pub fn name(self) -> &'static str {
        match self {
            Mascot::Pixo => "Pixo",
            Mascot::Byte => "Byte",
            Mascot::Michi => "Michi",
            Mascot::Axo => "Axo",
            Mascot::Nopal => "Nopal",
            Mascot::Bruma => "Bruma",
        }
    }

    /// Nombre con una descripción corta, para el selector.
    pub fn label(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Mascot::Pixo, Lang::Es) => "Pixo · blob coral",
            (Mascot::Pixo, Lang::En) => "Pixo · coral blob",
            (Mascot::Byte, Lang::Es) => "Byte · robot",
            (Mascot::Byte, Lang::En) => "Byte · robot",
            (Mascot::Michi, Lang::Es) => "Michi · gato",
            (Mascot::Michi, Lang::En) => "Michi · cat",
            (Mascot::Axo, Lang::Es) => "Axo · ajolote",
            (Mascot::Axo, Lang::En) => "Axo · axolotl",
            (Mascot::Nopal, Lang::Es) => "Nopal · cactus",
            (Mascot::Nopal, Lang::En) => "Nopal · cactus",
            (Mascot::Bruma, Lang::Es) => "Bruma · fantasma",
            (Mascot::Bruma, Lang::En) => "Bruma · ghost",
        }
    }

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let i = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

pub const W: usize = 32;
pub const H: usize = 26;
/// Filas de terminal que ocupa el dibujo.
pub const ROWS: u16 = (H / 2) as u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ink {
    Body,
    Belly,
    Line,
    White,
    Pupil,
    Cheek,
    Yellow,
    Green,
    Cyan,
    Blue,
    Magenta,
    Red,
    Gray,
    Dark,
    Paper,
    Wood,
    Metal,
}

impl Ink {
    /// `gray`: mascota sin sesiones, con el cuerpo apagado.
    fn color(self, mascot: Mascot, gray: bool) -> Color {
        let rgb = |r, g, b| Color::Rgb(r, g, b);
        match (self, gray) {
            (Ink::Body, false) => match mascot {
                Mascot::Pixo => rgb(0xe8, 0x87, 0x5f),
                Mascot::Byte => rgb(0x7d, 0xa6, 0xc9),
                Mascot::Michi => rgb(0xb8, 0xa4, 0xe3),
                Mascot::Axo => rgb(0xf4, 0xa6, 0xc4),
                Mascot::Nopal => rgb(0x6f, 0xb5, 0x5e),
                Mascot::Bruma => rgb(0x9a, 0xe0, 0xd4),
            },
            (Ink::Belly, false) => match mascot {
                Mascot::Pixo => rgb(0xf6, 0xc2, 0x9a),
                Mascot::Byte => rgb(0xc9, 0xdc, 0xea),
                Mascot::Michi => rgb(0xe6, 0xdc, 0xf7),
                Mascot::Axo => rgb(0xfb, 0xdc, 0xe6),
                Mascot::Nopal => rgb(0xb5, 0xdc, 0x9a),
                Mascot::Bruma => rgb(0xe2, 0xf8, 0xf3),
            },
            (Ink::Line, false) => match mascot {
                Mascot::Pixo => rgb(0x3b, 0x22, 0x19),
                Mascot::Byte => rgb(0x1f, 0x2a, 0x38),
                Mascot::Michi => rgb(0x2e, 0x22, 0x40),
                Mascot::Axo => rgb(0x4a, 0x1f, 0x35),
                Mascot::Nopal => rgb(0x1e, 0x3a, 0x1f),
                Mascot::Bruma => rgb(0x1c, 0x3d, 0x3f),
            },
            (Ink::White, false) => rgb(0xfd, 0xf6, 0xec),
            (Ink::Pupil, false) => rgb(0x1d, 0x1d, 0x2b),
            (Ink::Cheek, false) => match mascot {
                Mascot::Byte => rgb(0xff, 0x9f, 0x43),
                Mascot::Pixo | Mascot::Michi | Mascot::Axo | Mascot::Bruma => rgb(0xf0, 0x71, 0x8a),
                Mascot::Nopal => rgb(0xe8, 0x5d, 0x75),
            },
            (Ink::Body, true) => rgb(0x7b, 0x7b, 0x86),
            (Ink::Belly, true) => rgb(0xa3, 0xa3, 0xad),
            (Ink::Line | Ink::Pupil, true) => rgb(0x2b, 0x2b, 0x33),
            (Ink::White, true) => rgb(0xc9, 0xc9, 0xd1),
            (Ink::Cheek, true) => rgb(0x8d, 0x8d, 0x9b),
            (Ink::Yellow, _) => rgb(0xff, 0xd2, 0x3f),
            (Ink::Green, _) => rgb(0x5f, 0xd3, 0x8d),
            (Ink::Cyan, _) => rgb(0x4c, 0xc9, 0xf0),
            (Ink::Blue, _) => rgb(0x3a, 0x6e, 0xa5),
            (Ink::Magenta, _) => rgb(0xd6, 0x5d, 0xb1),
            (Ink::Red, _) => rgb(0xef, 0x47, 0x6f),
            (Ink::Gray, _) => rgb(0x8d, 0x8d, 0x9b),
            (Ink::Dark, _) => rgb(0x4a, 0x4a, 0x58),
            (Ink::Paper, _) => rgb(0xef, 0xe6, 0xd2),
            (Ink::Wood, _) => rgb(0xa0, 0x67, 0x3a),
            (Ink::Metal, _) => rgb(0xc9, 0xd1, 0xd9),
        }
    }
}

struct Canvas {
    px: [[Option<Ink>; W]; H],
}

impl Canvas {
    fn new() -> Self {
        Self { px: [[None; W]; H] }
    }

    fn get(&self, x: i32, y: i32) -> Option<Ink> {
        if (0..W as i32).contains(&x) && (0..H as i32).contains(&y) {
            self.px[y as usize][x as usize]
        } else {
            None
        }
    }

    fn set(&mut self, x: i32, y: i32, ink: Ink) {
        if (0..W as i32).contains(&x) && (0..H as i32).contains(&y) {
            self.px[y as usize][x as usize] = Some(ink);
        }
    }

    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, ink: Ink) {
        for j in 0..h {
            for i in 0..w {
                self.set(x + i, y + j, ink);
            }
        }
    }

    fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, ink: Ink) {
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for s in 0..=steps {
            let t = s as f64 / steps as f64;
            let x = (x0 as f64 + (x1 - x0) as f64 * t).round_ties_even() as i32;
            let y = (y0 as f64 + (y1 - y0) as f64 * t).round_ties_even() as i32;
            self.set(x, y, ink);
        }
    }

    /// Elipse rellena con borde opcional; `only_on` limita el relleno a los
    /// píxeles que ya tienen ese color (la panza sólo sobre el cuerpo).
    #[allow(clippy::too_many_arguments)]
    fn ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, fill: Ink, edge: Option<Ink>, only_on: Option<Ink>) {
        let inside = |x: i32, y: i32| ((x as f64 - cx) / rx).powi(2) + ((y as f64 - cy) / ry).powi(2) <= 1.0;
        for y in 0..H as i32 {
            for x in 0..W as i32 {
                if !inside(x, y) || only_on.is_some_and(|ink| self.get(x, y) != Some(ink)) {
                    continue;
                }
                let border = ![(1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|(i, j)| inside(x + i, y + j));
                self.set(x, y, if border { edge.unwrap_or(fill) } else { fill });
            }
        }
    }

    /// Caja con las cuatro esquinas recortadas y borde de otro color.
    fn rounded_box(&mut self, x: i32, y: i32, w: i32, h: i32, fill: Ink, edge: Ink) {
        for j in 0..h {
            for i in 0..w {
                let corner = (i == 0 || i == w - 1) && (j == 0 || j == h - 1);
                if corner {
                    continue;
                }
                let border = i == 0 || i == w - 1 || j == 0 || j == h - 1;
                let next_to_corner = (i == 1 || i == w - 2) && (j == 1 || j == h - 2);
                self.set(x + i, y + j, if border || next_to_corner { edge } else { fill });
            }
        }
    }

    fn sprite(&mut self, x: i32, y: i32, rows: &[&str], ink: Ink) {
        for (j, row) in rows.iter().enumerate() {
            for (i, ch) in row.chars().enumerate() {
                if ch != '.' {
                    self.set(x + i as i32, y + j as i32, ink);
                }
            }
        }
    }
}

const QUESTION: [&str; 6] = [".XXX.", "X...X", "...X.", "..X..", ".....", "..X.."];
const ZED: [&str; 4] = ["XXX", "..X", ".X.", "XXX"];
const STAR: [&str; 3] = [".X.", "XXX", ".X."];

/// Elige de una secuencia según el cuadro.
fn cycle<T: Copy>(seq: &[T], f: u64) -> T {
    seq[(f % seq.len() as u64) as usize]
}

/// Luz de estado: antena, cola de Michi, branquias de Axo, tuna de Nopal o fuego de Bruma.
fn bulb(state: State, f: u64) -> Ink {
    use State::*;
    match state {
        Thinking | Asking | Error if f % 2 == 1 => Ink::White,
        Thinking => Ink::Yellow,
        Asking | PlanReady => Ink::Magenta,
        Error => Ink::Red,
        Done | Running => Ink::Green,
        Web => Ink::Cyan,
        Offline => Ink::Gray,
        _ => Ink::Yellow,
    }
}

/// Vaivén de antena y cola: quieto al dormir.
fn sway(state: State, f: u64) -> i32 {
    if matches!(state, State::Sleeping | State::Offline) || !matches!(f % 4, 1 | 2) { 0 } else { 1 }
}

/// Pies (o ruedas) visibles: en `done` saltan y desaparecen en el aire.
fn feet_down(state: State, f: u64) -> bool {
    state != State::Done || matches!(f % 4, 0 | 3)
}

/// Pixo: blob coral con antena.
fn body_pixo(c: &mut Canvas, state: State, f: u64, cx: i32, cy: i32) {
    if state == State::Running {
        let lift = (f % 2) as i32;
        c.rect(cx - 6, cy + 7 - lift, 4, 2, Ink::Line);
        c.rect(cx + 3, cy + 6 + lift, 4, 2, Ink::Line);
    } else if feet_down(state, f) {
        c.rect(cx - 6, cy + 7, 4, 2, Ink::Line);
        c.rect(cx + 3, cy + 7, 4, 2, Ink::Line);
    }

    c.ellipse(cx as f64, cy as f64, 10.4, 7.6, Ink::Body, Some(Ink::Line), None);
    c.ellipse(cx as f64, (cy + 3) as f64, 6.0, 4.0, Ink::Belly, None, Some(Ink::Body));

    let sway = sway(state, f);
    c.line(cx, cy - 8, cx + sway, cy - 11, Ink::Line);
    c.rect(cx - 1 + sway, cy - 13, 3, 2, bulb(state, f));
}

/// Byte: robot de chapa con ruedas, tornillos laterales y panel de luces.
fn body_byte(c: &mut Canvas, state: State, f: u64, cx: i32, cy: i32) {
    let fi = f as i32;
    if state == State::Running || feet_down(state, f) {
        let running = state == State::Running;
        let lift = if running { fi % 2 } else { 0 };
        for (x, l) in [(cx - 8, lift), (cx + 3, if running { 1 - lift } else { 0 })] {
            c.rect(x, cy + 8 - l, 6, 2, Ink::Dark);
            // Radios de la rueda: giran al rodar.
            let spoke = if running { fi % 3 } else { 1 };
            c.set(x + 1 + spoke, cy + 9 - l, Ink::Metal);
            c.set(x + 4 - spoke, cy + 8 - l, Ink::Metal);
        }
    }

    c.rounded_box(cx - 10, cy - 7, 21, 15, Ink::Body, Ink::Line);
    // Tornillos a los lados de la cabeza.
    c.rect(cx - 12, cy - 2, 2, 3, Ink::Metal);
    c.rect(cx + 11, cy - 2, 2, 3, Ink::Metal);

    // Panel de luces en el pecho: parpadean mientras trabaja.
    c.rect(cx - 5, cy + 4, 11, 2, Ink::Belly);
    let busy = state.is_busy();
    let lights = [
        (cx - 3, if busy && f.is_multiple_of(2) { Ink::Green } else { Ink::Dark }),
        (cx, if busy && (f / 2).is_multiple_of(2) { Ink::Yellow } else { Ink::Dark }),
        (cx + 3, if state == State::Error { Ink::Red } else if busy && f.is_multiple_of(3) { Ink::Cyan } else { Ink::Dark }),
    ];
    for (x, ink) in lights {
        c.rect(x, cy + 4, 1, 2, ink);
    }

    let sway = sway(state, f);
    c.line(cx, cy - 8, cx + sway, cy - 11, Ink::Line);
    c.rect(cx - 1 + sway, cy - 13, 3, 2, bulb(state, f));
}

/// Michi: gato lila con orejas, bigotes y la luz de estado en la punta de la cola.
fn body_michi(c: &mut Canvas, state: State, f: u64, cx: i32, cy: i32) {
    // Cola por detrás del cuerpo, meneándose.
    let wag = if matches!(state, State::Sleeping | State::Offline) { 0 } else { cycle(&[0, 1, 2, 1], f) };
    let (tx, ty) = (cx - 14 + wag, cy - 6);
    for off in 0..2 {
        c.line(cx - 8, cy + 4 + off, cx - 13, cy - 1 + off, Ink::Body);
        c.line(cx - 13 + off, cy - 1, tx + off, ty, Ink::Body);
    }
    c.rect(tx, ty - 1, 2, 2, bulb(state, f));

    // Orejas: triángulos con interior rosado.
    for ax in [cx - 6, cx + 6] {
        let ay = cy - 11;
        for i in 0..6 {
            c.rect(ax - i / 2, ay + i, 1 + i, 1, Ink::Line);
            if i >= 2 {
                c.rect(ax - i / 2 + 1, ay + i, i - 1, 1, Ink::Cheek);
            }
        }
    }

    if state == State::Running {
        let lift = (f % 2) as i32;
        c.rect(cx - 6, cy + 7 - lift, 4, 2, Ink::Belly);
        c.rect(cx + 3, cy + 6 + lift, 4, 2, Ink::Belly);
    } else if feet_down(state, f) {
        c.rect(cx - 6, cy + 7, 4, 2, Ink::Belly);
        c.rect(cx + 3, cy + 7, 4, 2, Ink::Belly);
    }

    c.ellipse(cx as f64, cy as f64, 10.0, 7.4, Ink::Body, Some(Ink::Line), None);
    c.ellipse(cx as f64, (cy + 3) as f64, 6.0, 4.0, Ink::Belly, None, Some(Ink::Body));

    // Bigotes, por fuera de la cara.
    for (x0, x1) in [(cx - 13, cx - 10), (cx + 13, cx + 10)] {
        c.line(x0, cy, x1, cy + 1, Ink::Line);
        c.line(x0, cy + 3, x1, cy + 2, Ink::Line);
    }
}

/// Axo: ajolote rosa con branquias que se encienden con la luz de estado y cola.
fn body_axo(c: &mut Canvas, state: State, f: u64, cx: i32, cy: i32) {
    let sway = sway(state, f);

    // Cola por detrás del cuerpo, meneándose.
    let wag = if matches!(state, State::Sleeping | State::Offline) { 0 } else { cycle(&[0, 1, 0, -1], f) };
    for off in 0..2 {
        c.line(cx + 7, cy + 4 + off, cx + 13, cy + 6 + wag + off, Ink::Body);
    }

    // Tres branquias por lado; la de arriba lleva la luz en la punta.
    for side in [-1, 1] {
        let (rx, ry) = (cx + side * 9, cy - 2);
        c.line(rx, ry, cx + side * 14, cy - 7 - sway, Ink::Cheek);
        c.line(rx, ry + 1, cx + side * 15, cy - 3 - sway, Ink::Cheek);
        c.line(rx, ry + 2, cx + side * 14, cy + 1 + sway, Ink::Cheek);
        c.rect(cx + side * 14 - 1, cy - 9 - sway, 2, 2, bulb(state, f));
    }

    if state == State::Running {
        let lift = (f % 2) as i32;
        c.rect(cx - 6, cy + 7 - lift, 4, 2, Ink::Belly);
        c.rect(cx + 3, cy + 6 + lift, 4, 2, Ink::Belly);
    } else if feet_down(state, f) {
        c.rect(cx - 6, cy + 7, 4, 2, Ink::Belly);
        c.rect(cx + 3, cy + 7, 4, 2, Ink::Belly);
    }

    c.ellipse(cx as f64, cy as f64, 10.4, 7.6, Ink::Body, Some(Ink::Line), None);
    c.ellipse(cx as f64, (cy + 3) as f64, 6.0, 4.0, Ink::Belly, None, Some(Ink::Body));
}

/// Nopal: penca en maceta con dos pencas chicas, espinas y una tuna como luz de estado.
fn body_nopal(c: &mut Canvas, state: State, f: u64, cx: i32, cy: i32) {
    // Las pencas chicas saludan mientras está despierto.
    let sway = sway(state, f);
    c.ellipse((cx - 12) as f64, (cy - 3 - sway) as f64, 3.4, 4.4, Ink::Body, Some(Ink::Line), None);
    c.ellipse((cx + 12) as f64, (cy - 5 + sway) as f64, 3.4, 4.4, Ink::Body, Some(Ink::Line), None);

    // Tuna en lo alto.
    c.rect(cx - 2 + sway, cy - 13, 4, 3, bulb(state, f));

    c.ellipse(cx as f64, (cy - 1) as f64, 10.0, 9.0, Ink::Body, Some(Ink::Line), None);
    c.ellipse(cx as f64, (cy + 3) as f64, 6.0, 3.6, Ink::Belly, None, Some(Ink::Body));

    // Espinas, alrededor de la cara.
    for (x, y) in [(-4, -7), (3, -7), (-8, -3), (8, -4), (-1, -8), (-8, 3), (8, 3), (-13, -5), (-11, -1), (12, -7), (13, -3)] {
        c.set(cx + x, cy + y, Ink::White);
    }

    // Maceta.
    c.rect(cx - 8, cy + 6, 17, 2, Ink::Wood);
    c.rect(cx - 8, cy + 8, 17, 1, Ink::Dark);
    c.rect(cx - 6, cy + 9, 13, 1, Ink::Wood);
}

/// Bruma: fantasma menta que flota, con el borde ondulante y un fuego fatuo como luz de estado.
fn body_bruma(c: &mut Canvas, state: State, f: u64, cx: i32, cy: i32) {
    // El fuego fatuo flota a su lado; debajo, los dos bracitos.
    let still = matches!(state, State::Sleeping | State::Offline);
    let bob = if still { 0 } else { cycle(&[0, -1, -2, -1], f) };
    c.rect(cx - 15, cy - 5 + bob, 3, 4, bulb(state, f));
    c.rect(cx - 12, cy + 1, 3, 2, Ink::Body);
    c.rect(cx + 10, cy + 1, 3, 2, Ink::Body);

    // Cabeza y faldón. El borde inferior ondula, más rápido si corre.
    c.ellipse(cx as f64, (cy - 1) as f64, 10.4, 9.0, Ink::Body, Some(Ink::Line), None);
    let phase = match state {
        State::Sleeping | State::Offline => 0,
        State::Running => f as i32,
        _ => (f / 2) as i32,
    };
    for x in -10..=10 {
        let hem = cy + 6 + cycle(&[0, 1, 2, 2, 1, 0], (x + 10 + phase) as u64);
        let edge = x.abs() == 10;
        for y in (cy - 1)..=hem {
            c.set(cx + x, y, if edge || y == hem { Ink::Line } else { Ink::Body });
        }
    }

    // Brillo en la cabeza.
    c.rect(cx - 6, cy - 7, 3, 1, Ink::Belly);
    c.rect(cx - 7, cy - 6, 1, 2, Ink::Belly);
}

/// Cuerpo de la mascota elegida más la cara común: ojos, mejillas y boca.
fn creature(c: &mut Canvas, mascot: Mascot, state: State, f: u64, dy: i32, dx: i32) {
    use State::*;
    let (cx, cy) = (15 + dx, 15 + dy);

    match mascot {
        Mascot::Pixo => body_pixo(c, state, f, cx, cy),
        Mascot::Byte => body_byte(c, state, f, cx, cy),
        Mascot::Michi => body_michi(c, state, f, cx, cy),
        Mascot::Axo => body_axo(c, state, f, cx, cy),
        Mascot::Nopal => body_nopal(c, state, f, cx, cy),
        Mascot::Bruma => body_bruma(c, state, f, cx, cy),
    }

    // Ojos
    let (ex1, ex2, ey) = (cx - 5, cx + 3, cy - 3);
    let closed = matches!(state, Sleeping | Offline) || (state == Idle && f.is_multiple_of(12));
    if closed {
        c.rect(ex1, ey + 2, 3, 1, Ink::Line);
        c.rect(ex2, ey + 2, 3, 1, Ink::Line);
    } else if state == Error {
        for ex in [ex1, ex2] {
            c.line(ex, ey, ex + 2, ey + 2, Ink::Line);
            c.line(ex + 2, ey, ex, ey + 2, Ink::Line);
        }
    } else if state == Done {
        for ex in [ex1, ex2] {
            c.set(ex, ey + 2, Ink::Line);
            c.set(ex + 1, ey + 1, Ink::Line);
            c.set(ex + 2, ey + 2, Ink::Line);
        }
    } else {
        let (px, py) = match state {
            Thinking if f % 8 < 4 => (0, 0),
            Thinking => (2, 0),
            Reading => (cycle(&[0, 1, 2, 1], f), 2),
            Searching => (2, 1),
            Asking => (1, 0),
            _ => (1, 1),
        };
        for ex in [ex1, ex2] {
            c.rect(ex, ey, 3, 4, Ink::White);
            c.rect(ex + px, ey + py, 1, 2, Ink::Pupil);
        }
        if state == Reading {
            for ex in [ex1, ex2] {
                c.rect(ex - 1, ey - 1, 5, 1, Ink::Line);
                c.rect(ex - 1, ey + 4, 5, 1, Ink::Line);
                c.rect(ex - 1, ey - 1, 1, 6, Ink::Line);
                c.rect(ex + 3, ey - 1, 1, 6, Ink::Line);
            }
            c.rect(ex1 + 4, ey + 1, ex2 - ex1 - 5, 1, Ink::Line);
        }
    }

    // Mejillas
    c.rect(cx - 8, cy + 1, 2, 1, Ink::Cheek);
    c.rect(cx + 7, cy + 1, 2, 1, Ink::Cheek);

    // Boca
    let my = cy + 2;
    match state {
        Answering if f % 2 == 1 => c.rect(cx - 1, my, 3, 2, Ink::Line),
        Done | Idle | Working | Organizing => {
            c.set(cx - 2, my, Ink::Line);
            c.rect(cx - 1, my + 1, 3, 1, Ink::Line);
            c.set(cx + 2, my, Ink::Line);
        }
        Error => {
            c.rect(cx - 2, my + 1, 5, 1, Ink::Line);
            c.set(cx - 2, my, Ink::Line);
            c.set(cx + 2, my, Ink::Line);
        }
        Thinking | Asking => c.rect(cx, my, 2, 2, Ink::Line),
        Sleeping | Offline => c.rect(cx - 1, my + 1, 2, 1, Ink::Line),
        _ => c.rect(cx - 1, my + 1, 3, 1, Ink::Line),
    }
}

fn accessories(c: &mut Canvas, state: State, f: u64, dy: i32) {
    use State::*;
    let (cx, cy) = (15, 15 + dy);
    let fi = f as i32;
    match state {
        Thinking => {
            for (i, (sx, sy)) in [(cx - 5, cy - 14), (cx + 5, cy - 15), (cx + 8, cy - 11)].into_iter().enumerate() {
                if (f + i as u64).is_multiple_of(3) {
                    c.sprite(sx, sy, &STAR, Ink::Yellow);
                }
            }
        }
        Sleeping | Offline => {
            let z = (f % 6) as i32;
            let ink = if state == Offline { Ink::Gray } else { Ink::Cyan };
            c.sprite(cx + 9, cy - 9 - z / 2, &ZED, ink);
        }
        Asking => c.sprite(cx + 8, cy - 14 + cycle(&[0, 1, 2, 1], f), &QUESTION, Ink::Magenta),
        Reading => {
            c.rect(cx - 6, cy + 4, 13, 4, Ink::Paper);
            c.rect(cx, cy + 4, 1, 4, Ink::Line);
            for i in 0..3 {
                let ink = if !(f + i as u64).is_multiple_of(3) { Ink::Gray } else { Ink::Dark };
                c.rect(cx - 5, cy + 5 + i, 4, 1, ink);
            }
        }
        Searching => {
            let mx = cx + 7 + cycle(&[0, 1, 2, 1], f);
            let my = cy + 1;
            c.ellipse(mx as f64, my as f64, 3.2, 3.2, Ink::Cyan, Some(Ink::Metal), None);
            c.line(mx + 2, my + 2, mx + 5, my + 5, Ink::Wood);
        }
        Editing => {
            let (px, py) = (cx + 8, cy - 2 + cycle(&[0, 1, 0, -1], f));
            c.line(px, py, px + 5, py - 5, Ink::Yellow);
            c.line(px + 1, py, px + 6, py - 5, Ink::Yellow);
            c.set(px, py + 1, Ink::Line);
            c.rect(px + 6, py - 6, 1, 1, Ink::Cheek);
            c.rect(cx - 9, cy + 9, 6 + (fi % 4) * 2, 1, Ink::Blue);
        }
        Running => {
            for i in 0..3 {
                if (fi + i) % 3 != 0 {
                    c.rect(cx - 12 - i * 2 - fi % 2, cy + 4 - i, 2, 2, Ink::Gray);
                }
            }
            c.rect(cx + 9, cy - 6, 7, 5, Ink::Dark);
            c.sprite(cx + 10, cy - 5, &["X.", ".X", "X."], Ink::Green);
            if f % 2 == 1 {
                c.rect(cx + 13, cy - 3, 2, 1, Ink::Green);
            }
        }
        Web => {
            let (gx, gy) = (cx + 10, cy - 8);
            c.ellipse(gx as f64, gy as f64, 4.0, 4.0, Ink::Cyan, Some(Ink::Blue), None);
            let meridian = gx - 2 + fi % 5;
            c.line(meridian, gy - 3, meridian, gy + 3, Ink::Blue);
            c.rect(gx - 3, gy, 7, 1, Ink::Blue);
        }
        Delegating => {
            for (i, sx) in [cx - 15, cx + 12].into_iter().enumerate() {
                let hop = (fi + i as i32) % 2;
                c.ellipse((sx + 2) as f64, (cy + 5 - hop) as f64, 3.0, 2.6, Ink::Body, Some(Ink::Line), None);
                c.set(sx + 1, cy + 4 - hop, Ink::Pupil);
                c.set(sx + 3, cy + 4 - hop, Ink::Pupil);
                c.set(sx + 2, cy + 1 - hop, Ink::Line);
            }
        }
        Planning | PlanReady | Organizing => {
            let (bx, by) = (cx + 8, cy - 4);
            c.rect(bx, by, 7, 9, Ink::Wood);
            c.rect(bx + 1, by + 1, 5, 7, Ink::Paper);
            c.rect(bx + 2, by - 1, 3, 1, Ink::Metal);
            let checks = if state == PlanReady { 3 } else { (fi / 2) % 4 };
            for i in 0..3 {
                c.rect(bx + 2, by + 2 + i * 2, 3, 1, if i < checks { Ink::Green } else { Ink::Gray });
            }
            if state == PlanReady {
                c.sprite(cx - 13, cy - 12 + fi % 2, &QUESTION, Ink::Magenta);
            }
        }
        Done => {
            const CONFETTI: [Ink; 5] = [Ink::Yellow, Ink::Green, Ink::Cyan, Ink::Magenta, Ink::Red];
            for i in 0..7 {
                let ang = (i as f64 / 7.0) * TAU + f as f64 * 0.5;
                let r = 12.0 + (f % 3) as f64;
                let x = (cx as f64 + ang.cos() * r).round_ties_even() as i32;
                let y = ((cy - 3) as f64 + ang.sin() * r * 0.7).round_ties_even() as i32;
                c.set(x, y, CONFETTI[i % 5]);
            }
        }
        Error => {
            for i in 0..4 {
                let ang = f as f64 * 0.9 + i as f64 * PI / 2.0;
                let x = (cx as f64 + ang.cos() * 4.0).round_ties_even() as i32;
                let y = ((cy - 13) as f64 + ang.sin() * 1.5).round_ties_even() as i32;
                c.set(x, y, Ink::Yellow);
            }
        }
        Mcp | Tool => {
            let (gx, gy) = (cx + 11, cy - 6);
            const TEETH: [(i32, i32); 8] = [(0, -3), (2, -2), (3, 0), (2, 2), (0, 3), (-2, 2), (-3, 0), (-2, -2)];
            c.ellipse(gx as f64, gy as f64, 2.4, 2.4, Ink::Metal, None, None);
            for (i, (tx, ty)) in TEETH.into_iter().enumerate() {
                if (i as u64 + f).is_multiple_of(2) {
                    c.set(gx + tx, gy + ty, Ink::Metal);
                }
            }
        }
        Idle | Working | Answering => {}
    }
}

fn frame(mascot: Mascot, state: State, f: u64) -> Canvas {
    use State::*;
    let mut c = Canvas::new();
    let dy = match state {
        Done => cycle(&[0, -2, -3, -1], f),
        Idle | Working | Answering | Organizing => i32::from(f % 8 >= 4),
        Running => (f % 2) as i32,
        _ => 0,
    };
    let dx = if state == Error { cycle(&[0, 1, 0, -1], f) } else { 0 };
    creature(&mut c, mascot, state, f, dy, dx);
    accessories(&mut c, state, f, dy);
    c
}

/// Pinta el cuadro `f` del estado como líneas de medios bloques en truecolor.
/// `background` es el color del panel, para los medios píxeles vacíos.
pub fn render(mascot: Mascot, state: State, f: u64, background: Color) -> Vec<Line<'static>> {
    let gray = state == State::Offline;
    let c = frame(mascot, state, f);
    halfblocks(W, H, |x, y| c.px[y][x].map(|ink| ink.color(mascot, gray)), background)
}

/// Pinta una rejilla de `w` x `h` píxeles con medios bloques: cada fila de
/// terminal son dos píxeles. `background` rellena el medio píxel vacío; una
/// celda sin ningún píxel queda en blanco y deja ver el panel.
pub fn halfblocks(
    w: usize,
    h: usize,
    get: impl Fn(usize, usize) -> Option<Color>,
    background: Color,
) -> Vec<Line<'static>> {
    (0..h)
        .step_by(2)
        .map(|y| {
            let spans: Vec<Span<'static>> = (0..w)
                .map(|x| {
                    let top = get(x, y);
                    let bottom = if y + 1 < h { get(x, y + 1) } else { None };
                    match (top, bottom) {
                        (None, None) => Span::raw(" "),
                        (None, Some(b)) => Span::styled("▄", Style::default().fg(b).bg(background)),
                        (Some(t), None) => Span::styled("▀", Style::default().fg(t).bg(background)),
                        (Some(t), Some(b)) => Span::styled("▀", Style::default().fg(t).bg(b)),
                    }
                })
                .collect();
            Line::from(spans)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todos_los_estados_dibujan_algo_en_todos_los_cuadros() {
        for mascot in Mascot::ALL {
            for state in State::ALL {
                for f in 0..24 {
                    let lines = render(mascot, state, f, Color::Black);
                    assert_eq!(lines.len(), ROWS as usize);
                    assert!(lines.iter().all(|l| l.spans.len() == W));
                    let inked = lines.iter().flat_map(|l| &l.spans).filter(|s| s.content != " ").count();
                    assert!(inked > 100, "{mascot:?} {state:?} cuadro {f}: sólo {inked} celdas");
                }
            }
        }
    }

    #[test]
    fn la_animacion_cambia_entre_cuadros() {
        for mascot in Mascot::ALL {
            for state in [State::Thinking, State::Running, State::Done, State::Editing] {
                assert_ne!(
                    render(mascot, state, 0, Color::Black),
                    render(mascot, state, 1, Color::Black),
                    "{mascot:?} {state:?}"
                );
            }
        }
    }

    #[test]
    fn las_mascotas_se_distinguen() {
        for (i, a) in Mascot::ALL.iter().enumerate() {
            for b in &Mascot::ALL[i + 1..] {
                assert_ne!(render(*a, State::Idle, 1, Color::Black), render(*b, State::Idle, 1, Color::Black), "{a:?} {b:?}");
            }
        }
    }

    #[test]
    fn el_selector_da_la_vuelta() {
        assert_eq!(Mascot::Bruma.next(), Mascot::Pixo);
        assert_eq!(Mascot::Pixo.prev(), Mascot::Bruma);
    }
}
