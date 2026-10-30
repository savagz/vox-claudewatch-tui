use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
};

use crate::i18n::{self, Key};
use crate::setup::SystemCheckState;
use crate::theme::{Status, Theme, glyph, hazard_stripe};

use super::widgets::panel;

/// Wizard a pantalla completa: corre antes que exista ningún tablero detrás.
pub fn render_gate(frame: &mut Frame, area: Rect, state: &SystemCheckState, theme: &Theme) {
    frame.render_widget(Block::default().style(theme.base_style()), area);
    render_frame(frame, area, state, theme, true);
}

/// Mismo cuerpo, embebido como pestaña del tablero (`VERIFICACIÓN`).
pub fn render_panel(frame: &mut Frame, area: Rect, state: &SystemCheckState, theme: &Theme) {
    render_frame(frame, area, state, theme, false);
}

fn render_frame(frame: &mut Frame, area: Rect, state: &SystemCheckState, theme: &Theme, gate: bool) {
    let lang = state.lang;
    let all_ok = state.report.all_ok();
    let accent = if all_ok { theme.ok } else { theme.warning };
    let title = if gate {
        i18n::t(lang, Key::SetupGateTitle)
    } else {
        i18n::t(lang, Key::SetupPanelTitle)
    };

    let status_text = if all_ok {
        i18n::t(lang, Key::SetupStatusAllOk)
    } else {
        i18n::t(lang, Key::SetupStatusActionRequired)
    };
    let block = panel::block(theme, title, accent, true)
        .title_bottom(panel::readout(theme, status_text, accent));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(inner);

    render_stripe(frame, rows[0], theme, all_ok, gate, lang);
    render_checks(frame, rows[1], state, theme);
    render_detail(frame, rows[2], state, theme);
    render_keys(frame, rows[3], theme, gate, lang);
}

fn render_stripe(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    all_ok: bool,
    gate: bool,
    lang: crate::i18n::Lang,
) {
    let text = if all_ok {
        i18n::t(lang, Key::SetupStripeAllOk)
    } else if gate {
        i18n::t(lang, Key::SetupStripeGateBlocking)
    } else {
        i18n::t(lang, Key::SetupStripePanelBlocking)
    };
    let color = if all_ok { theme.ok } else { theme.warning };
    let stripe_width = area.width.saturating_sub(text.chars().count() as u16 + 2).min(8) as usize;

    let line = Line::from(vec![
        Span::styled(hazard_stripe(stripe_width), theme.style_for_color(color)),
        Span::styled(format!(" {text} "), theme.style_for_color(color)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_checks(frame: &mut Frame, area: Rect, state: &SystemCheckState, theme: &Theme) {
    let lines: Vec<Line> = state
        .report
        .checks
        .iter()
        .enumerate()
        .map(|(i, check)| {
            let selected = i == state.selected;
            let (led, color) = match check.status {
                Status::Ok => (glyph::LED_ON, theme.ok),
                Status::Warning => (glyph::LED_ON, theme.warning),
                Status::Error => (glyph::LED_ON, theme.error),
                Status::Info => (glyph::LED_OFF, theme.fg_muted),
            };
            let marker = if selected { "▌" } else { " " };
            let label_style = if selected { theme.style_for_bold() } else { theme.label_style() };
            Line::from(vec![
                Span::styled(marker, theme.style_for_color(theme.accent_primary)),
                Span::styled(format!("{led} "), theme.style_for_color(color)),
                Span::styled(format!("{:<20}", check.label), label_style),
                Span::styled(check.detail.clone(), theme.value_style()),
            ])
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_detail(frame: &mut Frame, area: Rect, state: &SystemCheckState, theme: &Theme) {
    let text = state
        .report
        .get(state.selected)
        .map(|c| c.detail.clone())
        .unwrap_or_default();
    frame.render_widget(
        Paragraph::new(text).style(theme.readout_style()).wrap(Wrap { trim: true }),
        area,
    );
}

fn render_keys(frame: &mut Frame, area: Rect, theme: &Theme, gate: bool, lang: crate::i18n::Lang) {
    let mut spans = vec![
        Span::styled(" ↑↓ ", theme.plate_style(theme.accent_secondary)),
        Span::styled(i18n::t(lang, Key::SetupKeyChoose), theme.label_style()),
        Span::styled(" R ", theme.plate_style(theme.accent_secondary)),
        Span::styled(i18n::t(lang, Key::SetupKeyRecheck), theme.label_style()),
    ];
    if gate {
        spans.push(Span::styled(" A ", theme.plate_style(theme.warning)));
        spans.push(Span::styled(i18n::t(lang, Key::SetupKeyContinueAnyway), theme.label_style()));
        spans.push(Span::styled(" ESC ", theme.plate_style(theme.error)));
        spans.push(Span::styled(i18n::t(lang, Key::SetupKeyExit), theme.label_style()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
