use chrono::Local;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::beacon::{self, server};
use crate::i18n::{self, Key};
use crate::theme::{Status, glyph};

use super::panel;

/// Franja superior: identificación del producto y dirección del servidor a la
/// izquierda; pilotos de sesiones, eventos y reloj a la derecha.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;

    let block = panel::block(theme, i18n::t(lang, Key::HeaderTitle), theme.accent_primary, true);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(inner);

    let (label, status) = if app.server_ok() {
        (Key::HeaderListening, Status::Ok)
    } else {
        (Key::HeaderServerDown, Status::Error)
    };
    let mut left = vec![
        Span::styled(format!("{} ", glyph::LED_ON), theme.style_for(status)),
        Span::styled(i18n::t(lang, label), theme.label_style()),
        Span::styled(server::address(&app.config), theme.readout_style()),
    ];
    if app.demo {
        left.push(Span::styled(format!("  {}", i18n::t(lang, Key::ModeDemo)), theme.style_for(Status::Info)));
    }
    frame.render_widget(Paragraph::new(Line::from(left)), cols[0]);

    let now = beacon::now();
    let live = app.hub.live(now);
    let working = live.iter().filter(|s| s.visible_state(now).is_busy()).count();
    let busy = if working > 0 { Status::Warning } else { Status::Ok };

    let pilots = Line::from(vec![
        Span::styled(format!("{} ", glyph::LED_ON), theme.style_for(busy)),
        Span::styled(
            i18n::t(lang, Key::HeaderSessionsTemplate)
                .replacen("{}", &live.len().to_string(), 1)
                .replacen("{}", &working.to_string(), 1),
            theme.style_for(busy),
        ),
        Span::styled(
            i18n::t(lang, Key::HeaderEventsTemplate).replacen("{}", &app.hub.received.to_string(), 1),
            theme.label_style(),
        ),
        Span::styled(Local::now().format("%H:%M:%S").to_string(), theme.readout_style()),
    ]);
    frame.render_widget(Paragraph::new(pilots).alignment(Alignment::Right), cols[1]);
}
