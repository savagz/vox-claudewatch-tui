use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::app::App;
use crate::beacon;
use crate::i18n::{self, Key};
use crate::theme::glyph;
use crate::ui::tabs::Tab;

/// Columna izquierda: botonera del tablero. Cada sección es un pulsador con su
/// LED de identidad; el activo queda "hundido" (fondo en relieve y marco propio).
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let mut constraints = Vec::with_capacity(Tab::ALL.len() + 1);
    for _ in Tab::ALL {
        constraints.push(Constraint::Length(3));
    }
    constraints.push(Constraint::Min(0));

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for tab in Tab::ALL {
        render_button(frame, rows[tab.index()], tab, app);
    }

    render_legend(frame, rows[Tab::ALL.len()], app);
}

fn has_alarm(tab: Tab, app: &App) -> bool {
    match tab {
        Tab::SystemCheck => !app.system_check.report.all_ok(),
        _ => false,
    }
}

fn is_live(tab: Tab, app: &App) -> bool {
    let now = beacon::now();
    match tab {
        Tab::Now => app.hub.live(now).iter().any(|s| s.visible_state(now).is_busy()),
        Tab::Sessions => !app.hub.live(now).is_empty(),
        Tab::Stats => app.hub.stats.day.turns > 0 || app.hub.stats.day.tools_total() > 0,
        Tab::Activity => !app.hub.timeline.is_empty(),
        Tab::SystemCheck => true,
        Tab::Office => app.current().is_some_and(|s| !s.active_agents(now).is_empty()),
    }
}

fn render_button(frame: &mut Frame, area: Rect, tab: Tab, app: &App) {
    let theme = &app.theme;
    let selected = tab == app.active_tab;
    let alarm = has_alarm(tab, app);
    let led_color = if alarm { theme.error } else { theme.tab_accent(tab.index()) };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(if selected { BorderType::Thick } else { BorderType::Plain })
        .border_style(theme.border_style(selected))
        .style(if selected { theme.raised_style() } else { theme.panel_style() });

    let label_style = if selected { theme.style_for_bold() } else { theme.tab_inactive_style() };

    let glyph = if alarm || selected || is_live(tab, app) {
        glyph::LED_ON
    } else {
        glyph::LED_OFF
    };

    let line = Line::from(vec![
        Span::styled(format!(" {glyph} "), theme.style_for_color(led_color)),
        Span::styled(tab.title(app.lang).to_uppercase(), label_style),
    ]);

    frame.render_widget(Paragraph::new(line).block(block), area);
}

/// Pie de la botonera: recordatorio de atajos siempre visible.
fn render_legend(frame: &mut Frame, area: Rect, app: &App) {
    if area.height < 3 {
        return;
    }
    let theme = &app.theme;
    let lang = app.lang;
    let mode = if app.demo { Key::ModeDemo } else { Key::ModeLive };

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(" [C] ", theme.plate_style(theme.accent_primary)),
            Span::styled(format!(" {}", i18n::t(lang, Key::SidebarConfig)), theme.label_style()),
        ]),
        Line::from(vec![
            Span::styled(" [H] ", theme.plate_style(theme.accent_secondary)),
            Span::styled(format!(" {}", i18n::t(lang, Key::SidebarHelp)), theme.label_style()),
        ]),
        Line::from(vec![
            Span::styled(" [M] ", theme.plate_style(theme.coral)),
            Span::styled(format!(" {}", app.config.ui.mascot.name()), theme.label_style()),
        ]),
        Line::from(Span::styled(format!("     {}", i18n::t(lang, mode)), theme.label_style())),
    ];

    frame.render_widget(Paragraph::new(lines).style(theme.base_style()), area);
}
