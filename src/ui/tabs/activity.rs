use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::beacon::{Note, TimelineEntry};
use crate::i18n::{self, Key, Lang};
use crate::theme::Theme;
use crate::ui::format;
use crate::ui::widgets::panel;

/// Registro completo de actividad, lo más nuevo arriba. ↑↓ desplaza.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;

    let count = i18n::t(lang, Key::ActivityCountTemplate).replacen("{}", &app.hub.timeline.len().to_string(), 1);
    let block = panel::block(theme, super::Tab::Activity.title(lang), theme.tab_accent(3), false)
        .title_bottom(panel::readout(theme, &count, theme.fg_muted));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let entries: Vec<&TimelineEntry> = app.hub.timeline.iter().rev().skip(app.activity_scroll).collect();
    frame.render_widget(Paragraph::new(lines(theme, lang, &entries, inner.width as usize)), inner);
}

/// Filas del registro. La columna de proyecto sólo aparece si hay más de uno.
pub fn lines(theme: &Theme, lang: Lang, entries: &[&TimelineEntry], width: usize) -> Vec<Line<'static>> {
    if entries.is_empty() {
        return vec![Line::from(Span::styled(i18n::t(lang, Key::ActivityEmpty), theme.label_style()))];
    }
    let multi = entries.iter().any(|e| e.project != entries[0].project);
    let prefix = 9 + if multi { 15 } else { 0 } + 2;
    entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let mut spans = vec![Span::styled(format!("{} ", format::clock(e.ts)), theme.label_style())];
            if multi {
                spans.push(Span::styled(format!("{:<15}", format::fit(&e.project, 14)), theme.style_for_color(theme.violet)));
            }
            spans.push(Span::styled(format!("{} ", e.note.icon()), theme.style_for_color(theme.coral)));
            let failed = matches!(e.note, Note::Tool { ok: false, .. });
            let style = if failed {
                theme.style_for_color(theme.error)
            } else if i == 0 {
                theme.style_for_bold()
            } else {
                theme.value_style()
            };
            spans.push(Span::styled(format::fit(&note_text(lang, &e.note), width.saturating_sub(prefix)), style));
            Line::from(spans)
        })
        .collect()
}

pub fn note_text(lang: Lang, note: &Note) -> String {
    let tpl = |k: Key| i18n::t(lang, k);
    match note {
        Note::SessionStarted { model } => tpl(Key::NoteSessionStartedTemplate).replacen("{}", model, 1),
        Note::SessionEnded { reason } => tpl(Key::NoteSessionEndedTemplate).replacen("{}", reason, 1),
        Note::Prompt(p) => tpl(Key::NotePromptTemplate).replacen("{}", p, 1),
        Note::NewTurn => tpl(Key::NoteNewTurn).to_string(),
        Note::Question(q) => tpl(Key::NoteQuestionTemplate).replacen("{}", q, 1),
        Note::PlanMode => tpl(Key::NotePlanMode).to_string(),
        Note::PlanReady => tpl(Key::NotePlanReady).to_string(),
        Note::Tool { agent, tool, detail, ok } => {
            let who = agent.as_ref().map(|a| format!("[{a}] ")).unwrap_or_default();
            let mark = if *ok { "" } else { " ✗" };
            format!("{who}{tool} {detail}{mark}")
        }
        Note::TaskDone(t) => tpl(Key::NoteTaskDoneTemplate).replacen("{}", t, 1),
        Note::TaskStarted(t) => tpl(Key::NoteTaskStartedTemplate).replacen("{}", t, 1),
        Note::AgentStarted { kind, description } => i18n::fill(tpl(Key::NoteAgentStartedTemplate), &[kind, description]),
        Note::AgentDone(d) => tpl(Key::NoteAgentDoneTemplate).replacen("{}", d, 1),
        Note::TurnDone { secs, aborted: false } => tpl(Key::NoteTurnDoneTemplate).replacen("{}", &format::dur(*secs), 1),
        Note::TurnDone { secs, aborted: true } => {
            tpl(Key::NoteTurnAbortedTemplate).replacen("{}", &format::dur(*secs), 1)
        }
    }
}
