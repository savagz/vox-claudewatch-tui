//! Sala de máquinas de los subagentes, con el lenguaje industrial del resto
//! de la TUI: un fondo de chapa oscura con remaches y, en la tarjeta de cada
//! subagente, una computadora (monitor CRT y torre sobre una base con franja
//! de peligro) con la pantalla animada según la herramienta y LEDs de estado
//! y actividad.
//!
//! Se pinta con el mismo motor de medios bloques que la mascota
//! (`mascot::halfblocks`): cada fila de terminal son dos píxeles.

use ratatui::{layout::Rect, style::Color, text::Line};

use crate::beacon::State;
use crate::mascot;
use crate::theme::Theme;

/// Tamaño del dibujo de una computadora: 30 columnas y 16 filas (32 píxeles:
/// monitor, pie, torre de sobremesa y base).
pub const PC_W: u16 = 30;
pub const PC_ROWS: u16 = 16;
/// Pantalla del monitor, en píxeles.
const SCREEN_W: i32 = 18;
const SCREEN_H: i32 = 12;
/// Filas de stats bajo la computadora, dentro de la tarjeta.
pub const STATS_ROWS: u16 = 3;
/// Tamaño de una tarjeta con su marco, y separación entre tarjetas.
pub const CARD_W: u16 = PC_W + 4;
pub const CARD_H: u16 = PC_ROWS + STATS_ROWS + 2;
const GAP: u16 = 1;
/// Décimas de segundo que dura el apagado del CRT de un subagente terminado.
pub const SHUTDOWN_DS: u32 = 15;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

/// Acero del chasis: brillo, cara, sombra y canto oscuro.
const STEEL: [Color; 4] = [rgb(0x5c, 0x68, 0x74), rgb(0x3a, 0x44, 0x4e), rgb(0x27, 0x2f, 0x37), rgb(0x16, 0x1b, 0x20)];
const RIVET: Color = rgb(0x7c, 0x88, 0x94);
const VENT: Color = rgb(0x0e, 0x12, 0x15);
const BEZEL: Color = rgb(0x0b, 0x0d, 0x0f);
const SCREEN_OFF: Color = rgb(0x10, 0x16, 0x14);
const PLATE_DOT: Color = rgb(0x1b, 0x21, 0x27);
const FLOOR: [Color; 2] = [rgb(0x2c, 0x35, 0x3e), rgb(0x1b, 0x21, 0x27)];
const HAZARD: [Color; 2] = [rgb(0xf2, 0xa2, 0x1c), rgb(0x0b, 0x0d, 0x0f)];
const LED_OFF: Color = rgb(0x22, 0x2a, 0x30);
const WHITE: Color = rgb(0xf2, 0xf2, 0xf2);

/// Pantallas: fondos y trazos en tonos de fósforo.
const PHOSPHOR_BG: Color = rgb(0x06, 0x12, 0x0c);
const PHOSPHOR: [Color; 3] = [rgb(0x3f, 0xbf, 0x63), rgb(0x8f, 0xe8, 0xa4), rgb(0x24, 0x6e, 0x3a)];
const AMBER_BG: Color = rgb(0x14, 0x0c, 0x02);
const AMBER: [Color; 2] = [rgb(0xf2, 0xa2, 0x1c), rgb(0x8a, 0x5a, 0x10)];
const CYAN_BG: Color = rgb(0x04, 0x10, 0x16);
const CYAN: [Color; 2] = [rgb(0x45, 0xa8, 0xc9), rgb(0x1e, 0x55, 0x68)];
const BLUE_BG: Color = rgb(0x06, 0x0c, 0x1c);
const BLUE: [Color; 2] = [rgb(0x6f, 0x8f, 0xd9), rgb(0x2e, 0x40, 0x70)];

/// Colores de identificación de cada tipo de subagente, sacados del tema:
/// ámbar, cian, violeta, coral, verde y azul.
const ACCENTS: [Color; 6] = [
    rgb(0xf2, 0xa2, 0x1c),
    rgb(0x45, 0xa8, 0xc9),
    rgb(0xb3, 0x92, 0xf0),
    rgb(0xe8, 0x87, 0x5f),
    rgb(0x3f, 0xbf, 0x63),
    rgb(0x6f, 0x8f, 0xd9),
];

fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// Color estable de un tipo de subagente.
pub fn shirt(kind: &str) -> Color {
    ACCENTS[(fnv(kind) % ACCENTS.len() as u64) as usize]
}

/// Qué se ve en la pantalla, según la herramienta del subagente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Work {
    Typing,
    Reading,
    Searching,
    Terminal,
    Web,
    Thinking,
    Other,
}

impl Work {
    /// Estado equivalente, para reutilizar sus etiquetas.
    pub fn state(self) -> State {
        match self {
            Work::Typing => State::Editing,
            Work::Reading => State::Reading,
            Work::Searching => State::Searching,
            Work::Terminal => State::Running,
            Work::Web => State::Web,
            Work::Thinking => State::Thinking,
            Work::Other => State::Tool,
        }
    }

    /// Color del LED de estado: la misma semántica que el resto del tablero.
    pub fn led(self, theme: &Theme) -> Color {
        match self {
            Work::Typing | Work::Terminal => theme.ok,
            Work::Reading | Work::Searching => theme.accent_secondary,
            Work::Web => theme.accent_quaternary,
            Work::Thinking | Work::Other => theme.warning,
        }
    }

    /// Cada cuántos cuadros cambia la fila de LEDs de actividad.
    fn blink(self) -> u64 {
        match self {
            Work::Typing | Work::Terminal => 1,
            Work::Reading | Work::Searching | Work::Web => 2,
            Work::Thinking | Work::Other => 4,
        }
    }
}

/// `tool` es la última herramienta con su detalle (`"Grep fn main"`).
pub fn work_for(tool: &str) -> Work {
    match tool.split_whitespace().next().unwrap_or("") {
        "" => Work::Thinking,
        "Edit" | "Write" | "NotebookEdit" | "MultiEdit" => Work::Typing,
        "Read" | "NotebookRead" => Work::Reading,
        "Grep" | "Glob" | "LSP" | "ToolSearch" => Work::Searching,
        "Bash" => Work::Terminal,
        "WebFetch" | "WebSearch" => Work::Web,
        _ => Work::Other,
    }
}

/// Una computadora: trabajando, o ya terminada (`since_ds`: décimas de
/// segundo desde entonces, para el apagado del CRT).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desk {
    Busy { work: Work, accent: Color },
    Gone { accent: Color, since_ds: u32 },
}

/// Tarjetas que caben en un área de `width` x `height` celdas, en orden de
/// lectura y centradas en horizontal. Cada `Rect` es relativo al área.
pub fn layout(width: u16, height: u16) -> Vec<Rect> {
    let cols = (width + GAP) / (CARD_W + GAP);
    let rows = (height + GAP) / (CARD_H + GAP);
    let used = (cols * (CARD_W + GAP)).saturating_sub(GAP);
    let x0 = width.saturating_sub(used) / 2;
    (0..rows)
        .flat_map(|r| {
            (0..cols).map(move |c| Rect {
                x: x0 + c * (CARD_W + GAP),
                y: r * (CARD_H + GAP),
                width: CARD_W,
                height: CARD_H,
            })
        })
        .collect()
}

/// Rejilla de píxeles de tamaño variable.
struct Scene {
    w: i32,
    h: i32,
    px: Vec<Option<Color>>,
}

impl Scene {
    fn new(w: u16, h: u16) -> Self {
        let (w, h) = (i32::from(w), i32::from(h) * 2);
        Self { w, h, px: vec![None; (w * h) as usize] }
    }

    fn get(&self, x: usize, y: usize) -> Option<Color> {
        self.px[y * self.w as usize + x]
    }

    fn put(&mut self, x: i32, y: i32, c: Color) {
        if (0..self.w).contains(&x) && (0..self.h).contains(&y) {
            self.px[(y * self.w + x) as usize] = Some(c);
        }
    }

    /// Color de un píxel (o el del bisel si está fuera o vacío).
    fn at(&self, x: i32, y: i32) -> Color {
        if (0..self.w).contains(&x) && (0..self.h).contains(&y) {
            self.px[(y * self.w + x) as usize].unwrap_or(BEZEL)
        } else {
            BEZEL
        }
    }

    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        for j in 0..h {
            for i in 0..w {
                self.put(x + i, y + j, c);
            }
        }
    }

    fn sprite(&mut self, x: i32, y: i32, rows: &[&str], c: Color) {
        for (j, row) in rows.iter().enumerate() {
            for (i, ch) in row.chars().enumerate() {
                if ch != '.' {
                    self.put(x + i as i32, y + j as i32, c);
                }
            }
        }
    }

    /// Caja de acero con brillo arriba e izquierda y sombra abajo y derecha.
    fn steel_box(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.rect(x, y, w, h, STEEL[1]);
        self.rect(x, y, w, 1, STEEL[0]);
        self.rect(x, y, 1, h, STEEL[0]);
        self.rect(x, y + h - 1, w, 1, STEEL[2]);
        self.rect(x + w - 1, y, 1, h, STEEL[2]);
        // Canto oscuro para separarla del fondo.
        self.rect(x + w, y + 1, 1, h, STEEL[3]);
        self.rect(x + 1, y + h, w, 1, STEEL[3]);
    }

    fn lines(&self, background: Color) -> Vec<Line<'static>> {
        mascot::halfblocks(self.w as usize, self.h as usize, |x, y| self.get(x, y), background)
    }
}

/// Fondo de la sala: chapa oscura con remaches en rejilla.
pub fn background(theme: &Theme, width: u16, height: u16) -> Vec<Line<'static>> {
    let mut s = Scene::new(width, height);
    s.rect(0, 0, s.w, s.h, theme.bg);
    for y in (3..s.h).step_by(8) {
        for x in (3..s.w).step_by(8) {
            s.put(x, y, PLATE_DOT);
        }
    }
    s.lines(theme.bg)
}

/// Una computadora sobre su base, lista para meter en una tarjeta
/// (`PC_W` x `PC_ROWS` celdas).
pub fn computer_lines(theme: &Theme, desk: Desk, tick: u64) -> Vec<Line<'static>> {
    let mut s = Scene::new(PC_W, PC_ROWS);
    s.rect(0, 0, s.w, s.h, theme.bg_panel);
    // Base de acero con franja de peligro.
    let fy = s.h - 2;
    for x in 0..s.w {
        s.put(x, fy, HAZARD[usize::from((x / 2) % 2 == 1)]);
        s.put(x, fy + 1, FLOOR[0]);
    }
    computer(&mut s, theme, desk, tick);
    s.lines(theme.bg_panel)
}

/// Una computadora: monitor CRT sobre su pie y torre de sobremesa, 30 x 30
/// píxeles (la base ocupa las dos filas de abajo).
fn computer(s: &mut Scene, theme: &Theme, desk: Desk, f: u64) {
    let (accent, work) = match desk {
        Desk::Busy { accent, work } => (accent, Some(work)),
        Desk::Gone { accent, .. } => (accent, None),
    };
    let on = work.is_some();

    // --- Monitor ---
    s.steel_box(1, 0, 28, 19);
    // Marco hundido alrededor de la pantalla, con su canto iluminado abajo.
    s.rect(3, 2, 20, 14, BEZEL);
    s.rect(3, 15, 20, 1, STEEL[2]);
    s.rect(22, 2, 1, 14, STEEL[2]);
    let (sx, sy) = (4, 3);
    match desk {
        Desk::Busy { work, .. } => {
            screen(s, sx, sy, work, f);
            scanlines(s, sx, sy);
            // Reflejo del cristal en la esquina.
            s.put(sx, sy, lerp(s.at(sx, sy), WHITE, 0.45));
            s.put(sx + 1, sy, lerp(s.at(sx + 1, sy), WHITE, 0.25));
            s.put(sx, sy + 1, lerp(s.at(sx, sy + 1), WHITE, 0.25));
        }
        Desk::Gone { since_ds, .. } => shutdown(s, theme, sx, sy, since_ds),
    }
    // Placa de marca bajo la pantalla, en el color del tipo.
    s.rect(9, 17, 8, 1, if on { accent } else { lerp(accent, STEEL[2], 0.6) });
    // Columna lateral: tres LEDs, dos mandos y rejilla de altavoz.
    let status = work.map_or(LED_OFF, |w| w.led(theme));
    let activity = if on && f % 4 < 2 { theme.ok } else { LED_OFF };
    let disk = work.is_some_and(|w| mix(f / w.blink() + 3).is_multiple_of(3));
    for (ly, color) in [(3, status), (5, activity), (7, if disk { theme.warning } else { LED_OFF })] {
        s.rect(25, ly, 2, 1, color);
    }
    for ky in [10, 13] {
        s.rect(25, ky, 2, 2, STEEL[3]);
        s.put(25, ky, RIVET);
    }
    for gx in (24..28).step_by(2) {
        s.put(gx, 16, VENT);
        s.put(gx + 1, 17, VENT);
    }
    for (rx, ry) in [(2, 1), (27, 1), (2, 17), (27, 17)] {
        s.put(rx, ry, RIVET);
    }

    // --- Pie ---
    s.rect(12, 19, 6, 2, STEEL[2]);
    s.rect(12, 19, 1, 2, STEEL[1]);
    s.rect(9, 21, 12, 1, STEEL[1]);
    s.rect(9, 21, 12, 1, STEEL[0]);

    // --- Torre de sobremesa ---
    s.steel_box(0, 22, 29, 7);
    s.rect(1, 23, 27, 1, if on { accent } else { lerp(accent, STEEL[2], 0.6) });
    // Ranura de disquete con su LED y botón de expulsión.
    s.rect(2, 25, 8, 1, VENT);
    s.put(8, 26, STEEL[0]);
    s.put(2, 27, if disk { theme.ok } else { LED_OFF });
    // Rejilla de ventilación.
    for vx in (12..19).step_by(2) {
        s.rect(vx, 25, 1, 3, VENT);
    }
    // Banco de seis LEDs que parpadean al ritmo del trabajo.
    for (i, (lx, ly)) in [(20, 25), (22, 25), (24, 25), (20, 27), (22, 27), (24, 27)].into_iter().enumerate() {
        let lit = work.is_some_and(|w| mix(f / w.blink() + i as u64 * 7).is_multiple_of(2));
        let color = if !lit {
            LED_OFF
        } else if i == 5 {
            theme.warning
        } else {
            theme.ok
        };
        s.put(lx, ly, color);
    }
    // Botón y piloto de encendido.
    s.rect(26, 25, 2, 2, STEEL[3]);
    s.put(26, 25, STEEL[0]);
    s.put(27, 27, if on { theme.ok } else { theme.error });
    for (rx, ry) in [(1, 22), (27, 22), (1, 27)] {
        s.put(rx, ry, RIVET);
    }
}

fn lerp(a: Color, b: Color, t: f32) -> Color {
    match (a, b) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
            let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t.clamp(0.0, 1.0)).round() as u8;
            Color::Rgb(m(r1, r2), m(g1, g2), m(b1, b2))
        }
        _ => a,
    }
}

/// Líneas de barrido del CRT: oscurece una fila de cada dos.
fn scanlines(s: &mut Scene, x: i32, y: i32) {
    for j in (1..SCREEN_H).step_by(2) {
        for i in 0..SCREEN_W {
            let c = s.at(x + i, y + j);
            s.put(x + i, y + j, lerp(c, BEZEL, 0.22));
        }
    }
}

/// Mezcla de bits para el parpadeo de los LEDs.
fn mix(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x
}

/// Apagado del CRT: la imagen se aplasta en una línea, luego en un punto, y
/// queda una ✔ verde tenue.
fn shutdown(s: &mut Scene, theme: &Theme, x: i32, y: i32, since_ds: u32) {
    s.rect(x, y, SCREEN_W, SCREEN_H, SCREEN_OFF);
    let (cx, cy) = (x + SCREEN_W / 2, y + SCREEN_H / 2);
    if since_ds < SHUTDOWN_DS {
        let k = since_ds as i32;
        if k < 6 {
            // Línea que se encoge hacia el centro.
            let half = (SCREEN_W / 2) * (6 - k) / 6;
            s.rect(cx - half.max(1), cy - 1, half.max(1) * 2, 2, WHITE);
        } else if k < 10 {
            s.rect(cx - 1, cy - 1, 2, 2, WHITE);
        } else {
            s.put(cx, cy, theme.fg_muted);
        }
    } else {
        s.sprite(cx - 4, cy - 3, &[".......X", "......X.", ".....X..", "X...X...", ".X.X....", "..X....."], PHOSPHOR[2]);
    }
}

/// Pantalla de `SCREEN_W` x `SCREEN_H` píxeles con lo que hace el subagente.
fn screen(s: &mut Scene, x: i32, y: i32, work: Work, f: u64) {
    let (w, h) = (SCREEN_W, SCREEN_H);
    match work {
        Work::Typing => {
            // Editor: números de línea y código con colores que se escribe.
            s.rect(x, y, w, h, PHOSPHOR_BG);
            s.rect(x, y, 2, h, PHOSPHOR[2]);
            let shown = (f % 13) as i32;
            let rows = [(0, 9), (2, 7), (2, 12), (4, 6), (4, 9), (2, 5), (0, 3), (0, 10), (2, 8), (4, 11), (2, 4), (0, 6)];
            for (row, (indent, len)) in rows.into_iter().enumerate() {
                let r = row as i32;
                if r < shown.min(h) {
                    let x0 = x + 3 + indent;
                    let len = len.min(w - 3 - indent);
                    // Palabra clave, resto del código y, a veces, un literal.
                    s.rect(x0, y + r, 2.min(len), 1, PHOSPHOR[1]);
                    s.rect(x0 + 3, y + r, (len - 3).max(0), 1, PHOSPHOR[0]);
                    if row % 3 == 1 && len > 6 {
                        s.rect(x0 + len - 3, y + r, 2, 1, AMBER[0]);
                    }
                }
            }
            if f.is_multiple_of(2) {
                let r = shown.min(h - 1);
                s.rect(x + w - 2, y + r, 1, 1, WHITE);
            }
        }
        Work::Reading => {
            // Documento con barra de título y barra de desplazamiento.
            s.rect(x, y, w, h, CYAN_BG);
            s.rect(x, y, w, 1, CYAN[1]);
            s.rect(x + 1, y, 5, 1, CYAN[0]);
            for row in 2..h {
                let n = row as u64 + f;
                if !n.is_multiple_of(5) {
                    let c = if n.is_multiple_of(7) { CYAN[0] } else { CYAN[1] };
                    s.rect(x + 1, y + row, 6 + (n % 8) as i32, 1, c);
                }
            }
            s.rect(x + w - 1, y + 1, 1, h - 1, CYAN[1]);
            s.rect(x + w - 1, y + 1 + (f % (h as u64 - 3)) as i32, 1, 3, CYAN[0]);
        }
        Work::Searching => {
            // Barrido de búsqueda con coincidencias resaltadas y una lupa.
            s.rect(x, y, w, h, CYAN_BG);
            for row in (1..h).step_by(2) {
                s.rect(x + 1, y + row, 10 + (row * 3) % 5, 1, CYAN[1]);
            }
            let scan = (f % h as u64) as i32;
            s.rect(x, y + scan, w, 1, lerp(CYAN[0], CYAN_BG, 0.4));
            for (mx, my) in [(3, 1), (7, 5), (4, 9), (9, 7)] {
                if my <= scan {
                    s.rect(x + mx, y + my, 3, 1, AMBER[0]);
                }
            }
            s.sprite(x + w - 5, y, &[".XX.", "X..X", "X..X", ".XX.", "....X"], CYAN[0]);
        }
        Work::Terminal => {
            // Consola: prompt, salida que avanza y cursor.
            s.rect(x, y, w, h, PHOSPHOR_BG);
            let rows = (f % 11) as i32 + 1;
            for row in 0..rows.min(h - 1) {
                if row % 3 == 0 {
                    s.rect(x, y + row, 2, 1, PHOSPHOR[1]);
                    s.rect(x + 3, y + row, 4 + row % 5, 1, WHITE);
                } else {
                    s.rect(x + 1, y + row, 3 + (row * 5) % 12, 1, PHOSPHOR[0]);
                }
            }
            let last = rows.min(h - 1);
            s.rect(x, y + last, 2, 1, PHOSPHOR[1]);
            if f.is_multiple_of(2) {
                s.rect(x + 3, y + last, 2, 1, PHOSPHOR[1]);
            }
        }
        Work::Web => {
            // Navegador: pestañas, barra de dirección y globo girando.
            s.rect(x, y, w, h, BLUE_BG);
            s.rect(x, y, w, 1, BLUE[1]);
            s.rect(x + 1, y, 4, 1, BLUE[0]);
            s.rect(x + 6, y, 4, 1, lerp(BLUE[1], WHITE, 0.2));
            s.rect(x + 1, y + 2, w - 2, 1, lerp(BLUE[1], WHITE, 0.15));
            let (gx, gy) = (x + 5, y + 4);
            s.sprite(gx, gy, &["..XXXX..", ".XXXXXX.", "XXXXXXXX", "XXXXXXXX", "XXXXXXXX", ".XXXXXX.", "..XXXX.."], BLUE[1]);
            let off = (f % 8) as i32;
            for j in 0..7 {
                s.put(gx + (off + j) % 8, gy + j, BLUE[0]);
                s.put(gx + (off + j + 4) % 8, gy + j, lerp(BLUE[0], BLUE_BG, 0.4));
            }
            s.rect(gx, gy + 3, 8, 1, BLUE[0]);
        }
        Work::Thinking => {
            // Indicador giratorio y tres puntos que se encienden por turnos.
            s.rect(x, y, w, h, AMBER_BG);
            let ring = [(1, 0), (2, 1), (2, 2), (1, 3), (0, 2), (0, 1)];
            let (rx, ry) = (x + w / 2 - 1, y + 2);
            for (i, (dx, dy)) in ring.into_iter().enumerate() {
                let lit = (f % 6) as usize == i;
                s.put(rx + dx, ry + dy, if lit { AMBER[0] } else { AMBER[1] });
            }
            for i in 0..3 {
                let lit = (f % 3) as i32 == i;
                s.rect(x + 5 + i * 3, y + 8, 2, 2, if lit { AMBER[0] } else { AMBER[1] });
            }
        }
        Work::Other => {
            // Barra de progreso con marcas y porcentaje.
            s.rect(x, y, w, h, AMBER_BG);
            s.rect(x + 2, y + 2, 6, 1, AMBER[1]);
            s.rect(x + 2, y + 5, w - 4, 3, AMBER[1]);
            s.rect(x + 2, y + 5, ((f % 15) as i32).min(w - 4), 3, AMBER[0]);
            for t in (2..w - 2).step_by(4) {
                s.put(x + t, y + 9, AMBER[1]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> Theme {
        Theme::default()
    }

    fn busy(work: Work) -> Desk {
        Desk::Busy { work, accent: shirt("Explore") }
    }

    #[test]
    fn familia_de_cada_herramienta() {
        assert_eq!(work_for(""), Work::Thinking);
        assert_eq!(work_for("Edit src/main.rs"), Work::Typing);
        assert_eq!(work_for("Write a.txt"), Work::Typing);
        assert_eq!(work_for("Read src/lib.rs"), Work::Reading);
        assert_eq!(work_for("Grep fn main"), Work::Searching);
        assert_eq!(work_for("Glob **/*.rs"), Work::Searching);
        assert_eq!(work_for("Bash cargo test"), Work::Terminal);
        assert_eq!(work_for("WebFetch https://x"), Work::Web);
        assert_eq!(work_for("mcp__x__y algo"), Work::Other);
    }

    #[test]
    fn el_color_de_cada_tipo_es_estable() {
        assert_eq!(shirt("Explore"), shirt("Explore"));
        assert!(ACCENTS.contains(&shirt("Plan")));
    }

    #[test]
    fn la_rejilla_cabe_en_el_area() {
        assert!(layout(10, 10).is_empty());
        assert!(layout(0, 0).is_empty());
        let cards = layout(100, 50);
        // (100 + 1) / 35 = 2 columnas; (50 + 1) / 22 = 2 filas.
        assert_eq!(cards.len(), 4);
        for r in &cards {
            assert!(r.right() <= 100 && r.bottom() <= 50, "{r:?}");
        }
        // Las tarjetas no se pisan.
        assert!(cards[0].right() < cards[1].x);
    }

    #[test]
    fn la_computadora_tiene_su_tamano() {
        for desk in [busy(Work::Typing), Desk::Gone { accent: shirt("x"), since_ds: 3 }] {
            let lines = computer_lines(&theme(), desk, 1);
            assert_eq!(lines.len(), PC_ROWS as usize);
            assert!(lines.iter().all(|l| l.spans.len() == PC_W as usize));
        }
        for (w, h) in [(0, 0), (1, 1), (80, 24)] {
            assert_eq!(background(&theme(), w, h).len(), h as usize);
        }
    }

    #[test]
    fn cada_trabajo_se_ve_distinto_y_se_anima() {
        let draw = |desk: Desk, tick: u64| computer_lines(&theme(), desk, tick);
        let works = [Work::Typing, Work::Reading, Work::Searching, Work::Terminal, Work::Web, Work::Thinking, Work::Other];
        for (i, a) in works.iter().enumerate() {
            assert_ne!(draw(busy(*a), 2), draw(busy(*a), 3), "{a:?} no se anima");
            for b in &works[i + 1..] {
                assert_ne!(draw(busy(*a), 2), draw(busy(*b), 2), "{a:?} y {b:?} se ven igual");
            }
        }
    }

    #[test]
    fn el_crt_se_apaga_y_queda_la_marca() {
        let draw = |since_ds| computer_lines(&theme(), Desk::Gone { accent: shirt("x"), since_ds }, 0);
        assert_ne!(draw(0), draw(8));
        assert_ne!(draw(8), draw(100));
        assert_eq!(draw(100), draw(500));
        assert_ne!(draw(100), computer_lines(&theme(), busy(Work::Typing), 0));
    }
}
