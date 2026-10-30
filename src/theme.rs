use ratatui::style::{Color, Modifier, Style};

/// Paleta "industrial": chapa oscura, ámbar de señalización, cian de instrumento.
/// Mismo lenguaje visual que el resto de la familia VOXPBX: panel de control de
/// sala de máquinas, contraste alto y semántica de tablero (verde marcha / ámbar
/// aviso / rojo alarma).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warning,
    Error,
    Info,
}

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub bg_panel: Color,
    pub bg_raised: Color,
    pub bg_selected: Color,

    pub fg: Color,
    pub fg_muted: Color,
    pub fg_dim: Color,

    pub border: Color,
    pub border_focused: Color,

    pub ok: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,

    /// Ámbar de señalización: color de identidad del producto.
    pub accent_primary: Color,
    /// Cian de instrumento: lecturas y medidas.
    pub accent_secondary: Color,
    /// Azul de servicio.
    pub accent_quaternary: Color,

    /// Coral de Pixo: identidad de Claude (respondiendo, trabajando).
    pub coral: Color,
    /// Violeta: subagentes y delegación.
    pub violet: Color,
    /// Magenta: Claude espera una decisión tuya (pregunta, plan).
    pub magenta: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Rgb(0x0b, 0x0d, 0x0f),
            bg_panel: Color::Rgb(0x12, 0x16, 0x1a),
            bg_raised: Color::Rgb(0x1b, 0x21, 0x27),
            bg_selected: Color::Rgb(0x2b, 0x35, 0x3f),

            fg: Color::Rgb(0xc8, 0xd2, 0xdc),
            fg_muted: Color::Rgb(0x74, 0x82, 0x8e),
            fg_dim: Color::Rgb(0x39, 0x43, 0x4d),

            border: Color::Rgb(0x2c, 0x35, 0x3e),
            border_focused: Color::Rgb(0xf2, 0xa2, 0x1c),

            ok: Color::Rgb(0x3f, 0xbf, 0x63),
            warning: Color::Rgb(0xf2, 0xa2, 0x1c),
            error: Color::Rgb(0xe0, 0x45, 0x3a),
            info: Color::Rgb(0x45, 0xa8, 0xc9),

            accent_primary: Color::Rgb(0xf2, 0xa2, 0x1c),
            accent_secondary: Color::Rgb(0x45, 0xa8, 0xc9),
            accent_quaternary: Color::Rgb(0x6f, 0x8f, 0xd9),

            coral: Color::Rgb(0xe8, 0x87, 0x5f),
            violet: Color::Rgb(0xb3, 0x92, 0xf0),
            magenta: Color::Rgb(0xd6, 0x5d, 0xb1),
        }
    }
}

impl Theme {
    pub fn color_for(&self, status: Status) -> Color {
        match status {
            Status::Ok => self.ok,
            Status::Warning => self.warning,
            Status::Error => self.error,
            Status::Info => self.info,
        }
    }

    pub fn style_for(&self, status: Status) -> Style {
        Style::default().fg(self.color_for(status))
    }

    /// Umbrales de los gauges de límites y contexto: <70% ok, 70-90% warning, >90% error.
    pub fn status_for_pct(&self, pct: f64) -> Status {
        if pct >= 90.0 {
            Status::Error
        } else if pct >= 70.0 {
            Status::Warning
        } else {
            Status::Ok
        }
    }

    pub fn base_style(&self) -> Style {
        Style::default().fg(self.fg).bg(self.bg)
    }

    pub fn panel_style(&self) -> Style {
        Style::default().fg(self.fg).bg(self.bg_panel)
    }

    pub fn raised_style(&self) -> Style {
        Style::default().fg(self.fg).bg(self.bg_raised)
    }

    pub fn border_style(&self, focused: bool) -> Style {
        let color = if focused {
            self.border_focused
        } else {
            self.border
        };
        Style::default().fg(color)
    }

    /// Título de panel: placa metálica grabada (mayúsculas, en relieve).
    pub fn plate_style(&self, color: Color) -> Style {
        Style::default()
            .fg(color)
            .bg(self.bg_raised)
            .add_modifier(Modifier::BOLD)
    }

    pub fn table_header_style(&self) -> Style {
        Style::default()
            .fg(self.fg_muted)
            .bg(self.bg_raised)
            .add_modifier(Modifier::BOLD)
    }

    pub fn label_style(&self) -> Style {
        Style::default().fg(self.fg_muted)
    }

    pub fn value_style(&self) -> Style {
        Style::default().fg(self.fg)
    }

    /// Lectura de instrumento: números y medidas, en cian.
    pub fn readout_style(&self) -> Style {
        Style::default()
            .fg(self.accent_secondary)
            .add_modifier(Modifier::BOLD)
    }

    pub fn tab_inactive_style(&self) -> Style {
        Style::default().fg(self.fg_muted)
    }

    pub fn style_for_bold(&self) -> Style {
        Style::default().fg(self.fg).add_modifier(Modifier::BOLD)
    }

    pub fn style_for_color(&self, color: Color) -> Style {
        Style::default().fg(color)
    }

    /// Color de identidad fijo por sección del menú (LED de nav):
    /// Ahora=coral, Sesiones=ámbar, Estadísticas=cian, Actividad=azul, Verificación=rojo.
    pub fn tab_accent(&self, index: usize) -> Color {
        match index {
            0 => self.coral,
            1 => self.accent_primary,
            2 => self.accent_secondary,
            3 => self.accent_quaternary,
            4 => self.error,
            _ => self.info,
        }
    }

    pub fn selected_row_style(&self) -> Style {
        Style::default()
            .bg(self.bg_selected)
            .fg(self.fg)
            .add_modifier(Modifier::BOLD)
    }
}

/// Glifos del lenguaje visual industrial: LEDs, segmentos de gauge y franja de
/// peligro. Centralizados para que todos los paneles usen exactamente los mismos.
pub mod glyph {
    pub const LED_ON: &str = "◉";
    pub const LED_OFF: &str = "○";
    pub const SEG_FULL: char = '█';
    pub const SEG_EMPTY: char = '─';
    pub const HAZARD: &str = "╱";
    /// Cuadros del indicador de actividad (spinner), animados por tick.
    pub const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    /// Devuelve el cuadro del spinner correspondiente al tick dado.
    pub fn spinner(tick: u64) -> &'static str {
        SPINNER[(tick as usize) % SPINNER.len()]
    }
}

/// Barra segmentada de instrumento: `width` celdas, llenas según `pct`.
pub fn segmented_bar(pct: f64, width: usize) -> (String, String) {
    let ratio = (pct / 100.0).clamp(0.0, 1.0);
    let filled = ((width as f64) * ratio).round() as usize;
    let filled = filled.min(width);
    (
        glyph::SEG_FULL.to_string().repeat(filled),
        glyph::SEG_EMPTY.to_string().repeat(width - filled),
    )
}

/// Franja de peligro (rayado diagonal) usada en cabeceras de alarma y en el
/// asistente de configuración.
pub fn hazard_stripe(width: usize) -> String {
    glyph::HAZARD.repeat(width)
}

/// Gráfico de barras de una línea (sparkline) escalado al máximo de la serie.
pub fn sparkline(values: &[u32]) -> String {
    const LEVELS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let top = values.iter().copied().max().unwrap_or(0).max(1) as f64;
    values
        .iter()
        .map(|&v| {
            if v == 0 {
                ' '
            } else {
                LEVELS[((v as f64 / top * 7.999) as usize).min(7)]
            }
        })
        .collect()
}
