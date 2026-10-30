pub mod config_form;
pub mod format;
pub mod setup;
pub mod tabs;
pub mod widgets;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::Color,
    widgets::Block,
};

use crate::app::App;
use crate::beacon::State;
use crate::theme::Theme;
use tabs::Tab;

const SIDEBAR_WIDTH: u16 = 22;
const HEADER_HEIGHT: u16 = 3;

pub fn draw(frame: &mut Frame, app: &App) {
    // Fondo de chapa uniforme.
    frame.render_widget(Block::default().style(app.theme.base_style()), frame.area());

    let help_bar_height = widgets::help_bar::height_for(app, frame.area().width);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(HEADER_HEIGHT),
            Constraint::Min(0),
            Constraint::Length(help_bar_height),
        ])
        .split(frame.area());

    widgets::header::render(frame, rows[0], app);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(0)])
        .split(rows[1]);

    widgets::sidebar::render(frame, cols[0], app);
    draw_body(frame, cols[1], app);
    widgets::help_bar::render(frame, rows[2], app);

    if let Some(form) = &app.config_form {
        config_form::render(frame, frame.area(), form, &app.theme);
    }
    if app.help_open {
        widgets::help::render(frame, frame.area(), app);
    }
}

fn draw_body(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    match app.active_tab {
        Tab::Now => tabs::now::render(frame, area, app),
        Tab::Sessions => tabs::sessions::render(frame, area, app),
        Tab::Stats => tabs::stats::render(frame, area, app),
        Tab::Activity => tabs::activity::render(frame, area, app),
        Tab::SystemCheck => tabs::syscheck::render(frame, area, app),
    }
}

/// Color de cada estado de Claude, con la semántica del tablero: ámbar
/// pensando, cian leyendo/buscando, verde ejecutando/terminado, magenta
/// esperándote, violeta delegando, rojo error.
pub fn state_color(theme: &Theme, state: State) -> Color {
    use State::*;
    match state {
        Thinking | Organizing => theme.accent_primary,
        Answering | Working => theme.coral,
        Reading | Searching | Web => theme.accent_secondary,
        Editing | Running | Done => theme.ok,
        Delegating => theme.violet,
        Asking | Planning | PlanReady => theme.magenta,
        Mcp | Tool => theme.accent_quaternary,
        Error => theme.error,
        Idle | Sleeping | Offline => theme.fg_muted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{app, with_sessions};
    use crate::mascot::Mascot;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn dibuja_el_tablero_en_varios_tamanos() {
        let mut app = app();
        for populated in [false, true] {
            if populated {
                with_sessions(&mut app);
            }
            for (w, h) in [(160, 50), (120, 40), (80, 24), (40, 12), (20, 6)] {
                let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                terminal.draw(|frame| draw(frame, &app)).unwrap();
            }
        }
    }

    #[test]
    fn dibuja_todas_las_pestanas_con_cada_mascota() {
        let mut app = app();
        with_sessions(&mut app);
        for mascot in Mascot::ALL {
            app.config.ui.mascot = mascot;
            for tab in Tab::ALL {
                app.active_tab = tab;
                for (w, h) in [(120, 40), (60, 20)] {
                    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                    terminal.draw(|frame| draw(frame, &app)).unwrap();
                }
            }
        }
    }

    #[test]
    fn dibuja_los_overlays() {
        let mut app = app();
        app.help_open = true;
        app.config_form = Some(crate::config::ConfigForm::new(&app.config));
        for (w, h) in [(120, 40), (80, 24), (40, 12)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|frame| draw(frame, &app)).unwrap();
        }
    }

    #[test]
    fn dibuja_el_wizard_de_arranque() {
        let app = app();
        let theme = crate::theme::Theme::default();
        for (w, h) in [(120, 40), (60, 20)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|frame| setup::render_gate(frame, frame.area(), &app.system_check, &theme)).unwrap();
        }
    }
}
