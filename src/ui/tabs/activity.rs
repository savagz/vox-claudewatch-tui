use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::beacon::{Note, NoteKind, TimelineEntry};
use crate::i18n::{self, Key, Lang};
use crate::theme::Theme;
use crate::ui::format;
use crate::ui::widgets::panel;

/// Registro completo de actividad, lo más nuevo arriba. ↑↓ desplaza; `/`,
/// `f`, `p` y Espacio filtran.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;
    let view = &app.activity;

    let filtered = app.activity_entries();
    let total = app.hub.timeline.len().to_string();
    let count = if view.is_filtered() {
        i18n::fill(i18n::t(lang, Key::ActivityFilteredTemplate), &[&filtered.len().to_string(), &total])
    } else {
        i18n::t(lang, Key::ActivityCountTemplate).replacen("{}", &total, 1)
    };
    let block = panel::block(theme, super::Tab::Activity.title(lang), theme.tab_accent(3), false)
        .title_bottom(panel::readout(theme, &count, theme.fg_muted));
    let mut inner = block.inner(area);
    frame.render_widget(block, area);

    if view.editing || view.is_filtered() {
        let bar = Rect { height: 1.min(inner.height), ..inner };
        frame.render_widget(Paragraph::new(filter_line(app)), bar);
        inner.y += bar.height;
        inner.height -= bar.height;
    }

    if filtered.is_empty() && !app.hub.timeline.is_empty() {
        let line = Line::from(Span::styled(i18n::t(lang, Key::ActivityNoMatch), theme.label_style()));
        frame.render_widget(Paragraph::new(line), inner);
        return;
    }
    let entries: Vec<&TimelineEntry> = filtered.into_iter().skip(app.activity_scroll).collect();
    frame.render_widget(Paragraph::new(lines(theme, lang, &entries, inner.width as usize)), inner);
}

/// Línea de filtros activos: `/ texto_ · tipo: errores · proyecto: web · PAUSA`.
fn filter_line(app: &App) -> Line<'static> {
    let theme = &app.theme;
    let lang = app.lang;
    let view = &app.activity;
    let sep = || Span::styled("  ·  ", theme.label_style());

    let mut spans = Vec::new();
    if view.editing || !view.query.is_empty() {
        let cursor = if view.editing { "_" } else { "" };
        spans.push(Span::styled("/ ", theme.style_for_color(theme.coral)));
        spans.push(Span::styled(format!("{}{cursor}", view.query), theme.style_for_bold()));
    }
    let mut labelled = |label: Key, value: String| {
        if !spans.is_empty() {
            spans.push(sep());
        }
        spans.push(Span::styled(format!("{}: ", i18n::t(lang, label)), theme.label_style()));
        spans.push(Span::styled(value, theme.style_for_color(theme.violet)));
    };
    if let Some(kind) = view.kind {
        labelled(Key::ActivityKindLabel, kind_label(lang, kind).to_string());
    }
    if let Some(project) = &view.project {
        labelled(Key::ActivityProjectLabel, project.clone());
    }
    if view.paused_at.is_some() {
        if !spans.is_empty() {
            spans.push(sep());
        }
        spans.push(Span::styled(
            format!(" {} ", i18n::t(lang, Key::ActivityPaused)),
            theme.plate_style(theme.accent_quaternary),
        ));
    }
    Line::from(spans)
}

pub fn kind_label(lang: Lang, kind: NoteKind) -> &'static str {
    i18n::t(
        lang,
        match kind {
            NoteKind::Session => Key::KindSession,
            NoteKind::Prompt => Key::KindPrompt,
            NoteKind::Tool => Key::KindTool,
            NoteKind::Error => Key::KindError,
            NoteKind::Agent => Key::KindAgent,
            NoteKind::Task => Key::KindTask,
            NoteKind::Plan => Key::KindPlan,
        },
    )
}

/// Ancho máximo de la columna de proyecto.
const PROJECT_MAX: usize = 16;

/// Filas del registro, cada una con el proyecto al que pertenece. La columna
/// se ajusta al nombre más largo de lo visible, hasta `PROJECT_MAX`.
pub fn lines(theme: &Theme, lang: Lang, entries: &[&TimelineEntry], width: usize) -> Vec<Line<'static>> {
    if entries.is_empty() {
        return vec![Line::from(Span::styled(i18n::t(lang, Key::ActivityEmpty), theme.label_style()))];
    }
    let project_width = entries.iter().map(|e| e.project.chars().count()).max().unwrap_or(0).min(PROJECT_MAX);
    let prefix = 9 + if project_width > 0 { project_width + 1 } else { 0 } + 2;
    entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let mut spans = vec![Span::styled(format!("{} ", format::clock(e.ts)), theme.label_style())];
            if project_width > 0 {
                spans.push(Span::styled(
                    format!("{:<w$} ", format::fit(&e.project, project_width), w = project_width),
                    theme.style_for_color(theme.violet),
                ));
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
