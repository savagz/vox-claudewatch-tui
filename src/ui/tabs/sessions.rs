use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use crate::app::App;
use crate::beacon::{self, Session};
use crate::i18n::{self, Key};
use crate::theme::glyph;
use crate::ui::widgets::panel;
use crate::ui::{format, state_color};

/// Sesiones vivas (izquierda) y ficha de la seleccionada (derecha). Enter fija
/// el foco de AHORA en la seleccionada.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;
    let now = beacon::now();
    let live = app.hub.live(now);

    if live.is_empty() {
        let block = panel::block(theme, super::Tab::Sessions.title(lang), theme.tab_accent(1), false);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(i18n::t(lang, Key::SessionsEmpty)).style(theme.label_style()).wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    let row = app.sessions_row.min(live.len() - 1);
    render_list(frame, cols[0], app, &live, row);
    render_detail(frame, cols[1], app, live[row]);
}

fn render_list(frame: &mut Frame, area: Rect, app: &App, live: &[&Session], row: usize) {
    let theme = &app.theme;
    let lang = app.lang;
    let now = beacon::now();
    let focused = app.current().map(|s| s.sid.clone());

    let count = i18n::t(lang, Key::SessionsCountTemplate).replacen("{}", &live.len().to_string(), 1);
    let block = panel::block(theme, super::Tab::Sessions.title(lang), theme.tab_accent(1), false)
        .title_bottom(panel::readout(theme, &count, theme.fg_muted));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Columnas fijas salvo proyecto y estado, que se reparten lo que sobra.
    let fixed = 2 + 2 + 9 + 10;
    let flexible = (inner.width as usize).saturating_sub(fixed);
    let project_w = (flexible * 2 / 5).max(6);
    let state_w = flexible.saturating_sub(project_w).max(6);

    let header = Line::from(Span::styled(
        format!(
            "    {:<project_w$}{:<state_w$}{:>9}{:>10}",
            i18n::t(lang, Key::ColProject),
            i18n::t(lang, Key::ColState),
            i18n::t(lang, Key::ColCost),
            i18n::t(lang, Key::ColSeen),
        ),
        theme.table_header_style(),
    ));
    let mut lines = vec![header];
    for (i, s) in live.iter().enumerate() {
        let state = s.visible_state(now);
        let selected = i == row;
        let is_focus = focused.as_ref() == Some(&s.sid);
        let row_style = if selected { theme.selected_row_style() } else { theme.value_style() };
        lines.push(Line::from(vec![
            Span::styled(if selected { "▌" } else { " " }, theme.style_for_color(theme.accent_primary)),
            Span::styled(
                format!("{} ", if is_focus { glyph::LED_ON } else { glyph::LED_OFF }),
                theme.style_for_color(if is_focus { theme.coral } else { theme.fg_dim }),
            ),
            Span::styled(format!(" {:<w$}", format::fit(&s.project, project_w - 1), w = project_w - 1), row_style),
            Span::styled(
                format!("{:<state_w$}", format::fit(i18n::state_label(lang, state), state_w - 1)),
                theme.style_for_color(state_color(theme, state)),
            ),
            Span::styled(format!("{:>9}", format::money(s.usd)), theme.readout_style()),
            Span::styled(format!("{:>10}", format::fit(&format::ago(lang, s.last_seen, now), 9)), theme.label_style()),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_detail(frame: &mut Frame, area: Rect, app: &App, s: &Session) {
    let theme = &app.theme;
    let lang = app.lang;
    let now = beacon::now();
    let state = s.visible_state(now);

    let block = panel::block(theme, i18n::t(lang, Key::SessionsDetailTitle), theme.accent_secondary, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let width = (inner.width as usize).saturating_sub(13);
    let field = |k: Key, v: String| {
        Line::from(vec![
            Span::styled(format!("{:<13}", i18n::t(lang, k)), theme.label_style()),
            Span::styled(format::fit(&v, width), theme.value_style()),
        ])
    };

    let mut lines = vec![
        field(Key::FieldSession, s.sid.clone()),
        field(Key::NowProject, s.project.clone()),
        Line::from(vec![
            Span::styled(format!("{:<13}", i18n::t(lang, Key::FieldCwd)), theme.label_style()),
            Span::styled(format::short_path(&s.cwd, width), theme.value_style()),
        ]),
        field(Key::NowModel, s.model.clone()),
        Line::from(vec![
            Span::styled(format!("{:<13}", i18n::t(lang, Key::FieldState)), theme.label_style()),
            Span::styled(i18n::state_label(lang, state), theme.style_for_color(state_color(theme, state))),
        ]),
        field(Key::FieldSince, format::dur(now - s.state_since)),
        field(Key::NowCost, format::money(s.usd)),
        field(Key::FieldTurnCost, format::money(s.turn_cost)),
        field(Key::FieldTasks, format!("{}/{}", s.tasks_done(), s.tasks.len())),
        field(Key::FieldAgents, s.active_agents(now).len().to_string()),
    ];
    if !s.prompt.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(i18n::t(lang, Key::NowPrompt), theme.plate_style(theme.accent_secondary))));
        lines.push(Line::from(Span::styled(s.prompt.clone(), theme.value_style())));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}
