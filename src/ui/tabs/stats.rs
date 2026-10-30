use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::beacon;
use crate::beacon::measure::Limit;
use crate::i18n::{self, Key};
use crate::theme::{segmented_bar, sparkline};
use crate::ui::format;
use crate::ui::widgets::panel;

use super::now::limits;

/// Límites, coste, contadores del día, herramientas y actividad por minuto.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(cols[0]);
    render_limits(frame, left[0], app);
    render_today(frame, left[1], app);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(4), Constraint::Length(4)])
        .split(cols[1]);
    render_cost(frame, right[0], app);
    render_tools(frame, right[1], app);
    render_activity(frame, right[2], app);
}

fn label_width(app: &App) -> usize {
    match app.lang {
        crate::i18n::Lang::Es => 13,
        crate::i18n::Lang::En => 15,
    }
}

fn render_limits(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;
    let mut block = panel::block(theme, i18n::t(lang, Key::StatsLimitsTitle), theme.accent_primary, false);
    if let Some(at) = app.hub.stats.last.limits_at {
        let readout = i18n::t(lang, Key::StatsUpdatedTemplate).replacen("{}", &format::ago(lang, at, beacon::now()), 1);
        block = block.title_bottom(panel::readout(theme, &readout, theme.fg_muted));
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let (five, seven, context, _) = limits(app, app.current());
    let lw = label_width(app);
    let bar_w = (inner.width as usize).saturating_sub(lw + 18).clamp(4, 24);
    let row = |label: Key, pct: Option<f64>, extra: String| {
        let color = pct.map_or(theme.fg_dim, |p| theme.color_for(theme.status_for_pct(p)));
        let (filled, empty) = segmented_bar(pct.unwrap_or(0.0), bar_w);
        Line::from(vec![
            Span::styled(format!("{:<lw$}", i18n::t(lang, label)), theme.label_style()),
            Span::styled(filled, theme.style_for_color(color)),
            Span::styled(empty, theme.style_for_color(theme.fg_dim)),
            Span::styled(pct.map_or("   --".into(), |p| format!(" {p:>3.0}%")), theme.readout_style()),
            Span::styled(format!(" {extra}"), theme.label_style()),
        ])
    };
    let reset = |l: Option<Limit>| {
        l.and_then(|l| l.resets_at).map(|t| format!("↻ {}", format::reset_at(lang, t))).unwrap_or_default()
    };
    let tokens = context
        .and_then(|c| Some(format!("{}k/{}k", c.tokens? / 1000, c.window.unwrap_or(0) / 1000)))
        .unwrap_or_default();
    let lines = vec![
        row(Key::GaugeFiveHour, five.and_then(|l| l.percent), reset(five)),
        row(Key::GaugeSevenDay, seven.and_then(|l| l.percent), reset(seven)),
        row(Key::GaugeContext, context.and_then(|c| c.percent), tokens),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_cost(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;
    let stats = &app.hub.stats;
    let mut block = panel::block(theme, i18n::t(lang, Key::StatsCostTitle), theme.ok, false);
    if stats.last.five_hour.is_some() {
        block = block.title_bottom(panel::readout(theme, i18n::t(lang, Key::StatsCostApiNote), theme.fg_muted));
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let current = app.current();
    let session = current.or_else(|| app.hub.latest());
    let lw = label_width(app);
    let row = |label: Key, value: String, bold: bool| {
        Line::from(vec![
            Span::styled(format!("{:<lw$}", i18n::t(lang, label)), theme.label_style()),
            Span::styled(value, if bold { theme.readout_style() } else { theme.value_style() }),
        ])
    };
    let mut session_cost = format::money(session.and_then(|s| s.usd));
    if let (None, Some(s)) = (current, session) {
        session_cost.push_str(&format!("  {}", format::fit(&s.project, 20)));
    }
    let mut lines = vec![
        row(Key::StatsCostSession, session_cost, false),
        row(Key::StatsCostTurn, format::money(session.and_then(|s| s.turn_cost)), false),
        row(Key::StatsCostToday, format::money(Some(stats.day_cost())), true),
    ];
    for (project, v) in stats.cost_by_project().into_iter().take(3) {
        let name = if project == "?" { i18n::t(lang, Key::StatsCostEarlier).to_string() } else { project };
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<w$}", format::fit(&name, lw - 2), w = lw - 2), theme.label_style()),
            Span::styled(format::money(Some(v)), theme.label_style()),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_today(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;
    let d = &app.hub.stats.day;
    let block = panel::block(theme, i18n::t(lang, Key::StatsTodayTitle), theme.accent_secondary, false)
        .title_bottom(panel::readout(theme, &d.date, theme.fg_muted));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let tpl = |k: Key| i18n::t(lang, k);
    let tools_total = d.tools_total();
    let err_pct = if tools_total > 0 { d.tool_errors as f64 / tools_total as f64 * 100.0 } else { 0.0 };
    let mut turns = d.turns.to_string();
    if d.aborted > 0 {
        turns.push_str(&format!("  · {}", tpl(Key::StatsAbortedTemplate).replacen("{}", &d.aborted.to_string(), 1)));
    }
    let rows = [
        (Key::StatsTurns, turns),
        (Key::StatsAvgTurn, if d.turns > 0 { format::dur(d.turn_ms as f64 / 1000.0 / d.turns as f64) } else { "--".into() }),
        (Key::StatsActive, format::dur(d.turn_ms as f64 / 1000.0)),
        (Key::StatsWaiting, format::dur(d.waiting_ms as f64 / 1000.0)),
        (
            Key::StatsTokens,
            tpl(Key::StatsTokensTemplate).replacen("{}", &format::kilo(d.tokens_in), 1).replacen("{}", &format::kilo(d.tokens_out), 1),
        ),
        (
            Key::StatsCache,
            d.cache_hit().map_or("--".into(), |h| tpl(Key::StatsCacheTemplate).replacen("{}", &format!("{h:.0}"), 1)),
        ),
        (
            Key::StatsFiles,
            tpl(Key::StatsFilesTemplate)
                .replacen("{}", &d.files_read.len().to_string(), 1)
                .replacen("{}", &d.files_edited.len().to_string(), 1),
        ),
        (Key::StatsCommands, d.bash.to_string()),
        (Key::StatsErrors, format!("{} ({err_pct:.0}%)", d.tool_errors)),
        (
            Key::StatsQuestions,
            tpl(Key::StatsQuestionsTemplate).replacen("{}", &d.questions.to_string(), 1).replacen("{}", &d.plans.to_string(), 1),
        ),
        (Key::StatsAgents, d.agents.to_string()),
        (Key::StatsTasks, tpl(Key::StatsTasksTemplate).replacen("{}", &d.tasks_done.to_string(), 1)),
    ];
    let lw = label_width(app);
    let lines: Vec<Line> = rows
        .into_iter()
        .map(|(k, v)| {
            Line::from(vec![
                Span::styled(format!("{:<lw$}", tpl(k)), theme.label_style()),
                Span::styled(v, theme.value_style()),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_tools(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;
    let d = &app.hub.stats.day;
    let block = panel::block(theme, i18n::t(lang, Key::StatsToolsTitle), theme.coral, false)
        .title_bottom(panel::readout(theme, &d.tools_total().to_string(), theme.fg_muted));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let top = d.top_tools(inner.height as usize);
    if top.is_empty() {
        frame.render_widget(Paragraph::new(Span::styled(i18n::t(lang, Key::StatsToolsEmpty), theme.label_style())), inner);
        return;
    }
    let peak = top[0].1.max(1) as f64;
    let bar_w = (inner.width as usize).saturating_sub(20).max(1) as f64;
    let lines: Vec<Line> = top
        .into_iter()
        .map(|(name, n)| {
            let len = ((n as f64 / peak * bar_w).round() as usize).max(1);
            Line::from(vec![
                Span::styled(format!("{:<13}", format::fit(name, 12)), theme.label_style()),
                Span::styled("■".repeat(len), theme.style_for_color(theme.coral)),
                Span::styled(format!(" {n}"), theme.readout_style()),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_activity(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;
    let block = panel::block(theme, i18n::t(lang, Key::StatsActivityTitle), theme.accent_quaternary, false);
    let inner = block.inner(area);
    let values = app.hub.stats.activity((inner.width as usize).min(60), beacon::now());
    let peak = values.iter().copied().max().unwrap_or(0);
    let block = block.title_bottom(panel::readout(
        theme,
        &i18n::t(lang, Key::StatsActivityPeakTemplate).replacen("{}", &peak.to_string(), 1),
        theme.fg_muted,
    ));
    frame.render_widget(block, area);
    let line = Line::from(Span::styled(sparkline(&values), theme.style_for_color(theme.coral)));
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);
    frame.render_widget(Paragraph::new(line), rows[1]);
}
