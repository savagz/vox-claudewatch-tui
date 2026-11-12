use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use crate::app::App;
use crate::beacon::measure::{Context, Limit};
use crate::beacon::{self, Session, State, TimelineEntry};
use crate::i18n::{self, Key};
use crate::mascot;
use crate::theme::{glyph, segmented_bar};
use crate::ui::widgets::{gauge, panel};
use crate::ui::{format, state_color};

use super::activity;

/// Ancho de la columna de la mascota: el dibujo más el marco y un respiro.
const MASCOT_WIDTH: u16 = mascot::W as u16 + 4;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let current = app.current();
    render_gauges(frame, rows[0], app, current);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(MASCOT_WIDTH), Constraint::Min(0)])
        .split(rows[1]);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(mascot::ROWS + 2), Constraint::Min(0)])
        .split(cols[0]);
    render_mascot(frame, left[0], app, current);
    render_status(frame, left[1], app, current);

    // Tareas, subagentes y actividad reciente se reparten el alto a partes iguales.
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Fill(1), Constraint::Fill(1)])
        .split(cols[1]);
    render_tasks(frame, right[0], app, current);
    render_agents(frame, right[1], app, current);
    render_recent(frame, right[2], app);
}

/// Límites de la sesión en foco o, si no hay, la última lectura conocida.
pub fn limits(app: &App, current: Option<&Session>) -> (Option<Limit>, Option<Limit>, Option<Context>, bool) {
    let last = &app.hub.stats.last;
    let m = current.map(|s| s.measure).unwrap_or_default();
    let context_fallback = m.context.is_none() && last.context.is_some();
    (
        m.five_hour.or(last.five_hour),
        m.seven_day.or(last.seven_day),
        m.context.or(last.context),
        context_fallback,
    )
}

fn render_gauges(frame: &mut Frame, area: Rect, app: &App, current: Option<&Session>) {
    let theme = &app.theme;
    let lang = app.lang;
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(34), Constraint::Percentage(33), Constraint::Percentage(33)])
        .split(area);

    let (five, seven, context, fallback) = limits(app, current);
    let reset = |l: Option<Limit>| {
        l.and_then(|l| l.resets_at).map(|t| format!("↻ {}", format::reset_at(lang, t))).unwrap_or_default()
    };

    gauge::render(frame, cols[0], theme, i18n::t(lang, Key::GaugeFiveHour), &reset(five), five.and_then(|l| l.percent));
    gauge::render(frame, cols[1], theme, i18n::t(lang, Key::GaugeSevenDay), &reset(seven), seven.and_then(|l| l.percent));

    let mut detail = context
        .and_then(|c| Some(format!("{}k/{}k", c.tokens? / 1000, c.window.unwrap_or(0) / 1000)))
        .unwrap_or_default();
    if fallback {
        let last = &app.hub.stats.last;
        let when = last.context_at.map(|t| format::ago(lang, t, beacon::now())).unwrap_or_default();
        detail = format!("{} · {when}", format::fit(&last.context_project, 14));
    }
    gauge::render(frame, cols[2], theme, i18n::t(lang, Key::GaugeContext), &detail, context.and_then(|c| c.percent));
}

fn render_mascot(frame: &mut Frame, area: Rect, app: &App, current: Option<&Session>) {
    let theme = &app.theme;
    let state = current.map_or(State::Offline, |s| s.visible_state(beacon::now()));
    let mascot = app.config.ui.mascot;

    let block = panel::block(theme, mascot.name(), state_color(theme, state), false);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = mascot::render(mascot, state, app.tick, theme.bg_panel);
    let x = inner.x + inner.width.saturating_sub(mascot::W as u16) / 2;
    let y = inner.y + inner.height.saturating_sub(mascot::ROWS) / 2;
    let target = Rect {
        x,
        y,
        width: (mascot::W as u16).min(inner.right().saturating_sub(x)),
        height: mascot::ROWS.min(inner.bottom().saturating_sub(y)),
    };
    frame.render_widget(Paragraph::new(lines), target);
}

fn render_status(frame: &mut Frame, area: Rect, app: &App, current: Option<&Session>) {
    let theme = &app.theme;
    let lang = app.lang;
    let now = beacon::now();

    let live = app.hub.live(now);
    let mut block = panel::block(theme, i18n::t(lang, Key::NowStatusTitle), theme.coral, false);
    if let Some(s) = current {
        let index = live.iter().position(|x| x.sid == s.sid).map_or(1, |i| i + 1);
        let mode = if app.auto { Key::NowFollowAuto } else { Key::NowFollowPinned };
        let readout = format!("{index}/{} · {}", live.len(), i18n::t(lang, mode));
        block = block.title_bottom(panel::readout(theme, &readout, theme.fg_muted));
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(s) = current else {
        let lines = vec![
            Line::from(Span::styled(i18n::t(lang, Key::NowNoSessions), theme.style_for_bold())),
            Line::from(""),
            Line::from(Span::styled(
                i18n::t(lang, Key::NowWaitingTemplate).replacen("{}", &app.config.server.port.to_string(), 1),
                theme.label_style(),
            )),
        ];
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
        return;
    };

    let state = s.visible_state(now);
    let width = inner.width as usize;
    let field = |label: Key, value: String, style| {
        Line::from(vec![
            Span::styled(format!("{:<15}", i18n::t(lang, label)), theme.label_style()),
            Span::styled(format::fit(&value, width.saturating_sub(15)), style),
        ])
    };

    let spinner = if state.is_busy() { format!("{} ", glyph::spinner(app.tick)) } else { String::new() };
    let mut lines = vec![Line::from(vec![
        Span::styled(spinner, theme.style_for_color(state_color(theme, state))),
        Span::styled(i18n::state_label(lang, state), theme.style_for_color(state_color(theme, state)).add_modifier(ratatui::style::Modifier::BOLD)),
        Span::styled(format!("  {}", format::dur(now - s.state_since)), theme.label_style()),
    ])];
    if !s.detail.is_empty() && state.is_busy() {
        lines.push(Line::from(Span::styled(s.detail.clone(), theme.value_style())));
    }
    lines.push(Line::from(""));
    if let Some(started) = s.turn_started {
        lines.push(field(Key::NowTurnRunning, format::dur(now - started), theme.readout_style()));
    } else if let Some(last) = s.last_turn {
        lines.push(field(Key::NowLastTurn, format::dur(last), theme.value_style()));
    }
    lines.push(field(Key::NowProject, s.project.clone(), theme.style_for_bold()));
    lines.push(field(Key::NowModel, if s.model.is_empty() { "?".into() } else { s.model.clone() }, theme.value_style()));
    lines.push(field(Key::NowCost, format::money(s.usd), theme.readout_style()));
    if !s.prompt.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(i18n::t(lang, Key::NowPrompt), theme.plate_style(theme.accent_secondary))));
        lines.push(Line::from(Span::styled(s.prompt.clone(), theme.value_style().add_modifier(ratatui::style::Modifier::ITALIC))));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn render_tasks(frame: &mut Frame, area: Rect, app: &App, current: Option<&Session>) {
    let theme = &app.theme;
    let lang = app.lang;
    let tasks = current.map(|s| s.tasks.as_slice()).unwrap_or_default();

    let mut block = panel::block(theme, i18n::t(lang, Key::NowTasksTitle), theme.accent_primary, false);
    let done = tasks.iter().filter(|t| t.status == "completed").count();
    if !tasks.is_empty() {
        let readout = i18n::t(lang, Key::NowTasksDoneTemplate)
            .replacen("{}", &done.to_string(), 1)
            .replacen("{}", &tasks.len().to_string(), 1);
        block = block.title_bottom(panel::readout(theme, &readout, theme.fg_muted));
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if tasks.is_empty() {
        frame.render_widget(Paragraph::new(Span::styled(i18n::t(lang, Key::NowTasksEmpty), theme.label_style())), inner);
        return;
    }

    let pct = done as f64 / tasks.len() as f64 * 100.0;
    let (filled, empty) = segmented_bar(pct, (inner.width as usize).saturating_sub(6).min(30));
    let mut lines = vec![
        Line::from(vec![
            Span::styled(filled, theme.style_for_color(theme.ok)),
            Span::styled(empty, theme.style_for_color(theme.fg_dim)),
            Span::styled(format!(" {pct:>3.0}%"), theme.readout_style()),
        ]),
        Line::from(""),
    ];
    let width = (inner.width as usize).saturating_sub(2);
    for status in ["in_progress", "pending", "completed"] {
        for t in tasks.iter().filter(|t| t.status == status) {
            let (mark, mark_style, text_style) = match status {
                "in_progress" => ("▶ ", theme.style_for_color(theme.accent_primary), theme.style_for_bold()),
                "pending" => ("○ ", theme.label_style(), theme.value_style()),
                _ => (
                    "✔ ",
                    theme.style_for_color(theme.ok),
                    theme.label_style().add_modifier(ratatui::style::Modifier::CROSSED_OUT),
                ),
            };
            lines.push(Line::from(vec![Span::styled(mark, mark_style), Span::styled(format::fit(&t.subject, width), text_style)]));
        }
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_agents(frame: &mut Frame, area: Rect, app: &App, current: Option<&Session>) {
    let theme = &app.theme;
    let lang = app.lang;
    let now = beacon::now();
    let (active, recent) = current.map_or((Vec::new(), Vec::new()), |s| (s.active_agents(now), s.recent_agents(3)));

    let mut block = panel::block(theme, i18n::t(lang, Key::NowAgentsTitle), theme.violet, false);
    if !active.is_empty() {
        let readout = i18n::t(lang, Key::NowAgentsActiveTemplate).replacen("{}", &active.len().to_string(), 1);
        block = block.title_bottom(panel::readout(theme, &readout, theme.violet));
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if active.is_empty() && recent.is_empty() {
        frame.render_widget(Paragraph::new(Span::styled(i18n::t(lang, Key::NowAgentsEmpty), theme.label_style())), inner);
        return;
    }
    let width = inner.width as usize;
    let mut lines = Vec::new();
    for a in active {
        lines.push(Line::from(vec![
            Span::styled("● ", theme.style_for_color(theme.violet)),
            Span::styled(format!("{} ", a.kind), theme.style_for_bold()),
            Span::styled(format!("{}  ", format::dur(now - a.started)), theme.label_style()),
            Span::styled(a.description.clone(), theme.value_style()),
        ]));
        if !a.tool.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("   └ {}", format::short_path(&a.tool, width.saturating_sub(5))),
                theme.label_style(),
            )));
        }
    }
    for a in recent {
        let took = format::dur(a.finished.unwrap_or(a.started) - a.started);
        lines.push(Line::from(Span::styled(format!("✔ {} {}  {took}", a.kind, a.description), theme.label_style())));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_recent(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;
    let block = panel::block(theme, i18n::t(lang, Key::NowRecentTitle), theme.accent_quaternary, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let entries: Vec<&TimelineEntry> = app.hub.timeline.iter().rev().take(inner.height as usize).collect();
    frame.render_widget(Paragraph::new(activity::lines(theme, lang, &entries, inner.width as usize)), inner);
}
