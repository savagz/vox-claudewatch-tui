use ratatui::{
    Frame,
    layout::Rect,
    style::Color,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use crate::app::App;
use crate::i18n::{self, Key};
use crate::theme::Theme;
use crate::ui::tabs::Tab;

/// Barra inferior: teclas grabadas como capuchones, contextuales según la
/// sección activa. Un aviso reciente ocupa la barra hasta la próxima tecla.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;

    if let Some((message, status)) = &app.toast {
        let line = Line::from(vec![
            Span::styled(i18n::t(lang, Key::HbNotice), theme.plate_style(theme.color_for(*status))),
            Span::styled(format!(" {message}"), theme.style_for(*status)),
        ]);
        frame.render_widget(Paragraph::new(line).style(theme.base_style()), area);
        return;
    }

    let spans = build_spans(app);
    frame.render_widget(
        Paragraph::new(Line::from(spans)).wrap(Wrap { trim: true }).style(theme.base_style()),
        area,
    );
}

/// Cuántas filas necesita la barra para no cortar atajos.
pub fn height_for(app: &App, width: u16) -> u16 {
    if app.toast.is_some() {
        return 1;
    }
    let spans = build_spans(app);
    let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let usable = width.max(1) as usize;
    let lines = total.saturating_add(usable - 1) / usable;
    (lines.max(1) as u16).min(3)
}

fn build_spans(app: &App) -> Vec<Span<'static>> {
    let theme = &app.theme;
    let lang = app.lang;

    let mut spans = vec![
        key(theme, "TAB", theme.accent_secondary),
        label(theme, i18n::t(lang, Key::HbView)),
        key(theme, "1-5", theme.accent_secondary),
        label(theme, i18n::t(lang, Key::HbJump)),
    ];

    spans.extend(match app.active_tab {
        Tab::Now => vec![
            key(theme, "S", theme.accent_secondary),
            label(theme, i18n::t(lang, Key::HbNextSession)),
            key(theme, "A", theme.ok),
            label(theme, i18n::t(lang, Key::HbAuto)),
            key(theme, "D", theme.info),
            label(theme, i18n::t(lang, Key::HbDemo)),
        ],
        Tab::Sessions => vec![
            key(theme, "↑↓", theme.accent_secondary),
            label(theme, i18n::t(lang, Key::HbNavigate)),
            key(theme, "Enter", theme.ok),
            label(theme, i18n::t(lang, Key::HbPin)),
            key(theme, "A", theme.ok),
            label(theme, i18n::t(lang, Key::HbAuto)),
        ],
        Tab::Activity => vec![
            key(theme, "↑↓", theme.accent_secondary),
            label(theme, i18n::t(lang, Key::HelpScroll)),
        ],
        Tab::SystemCheck => vec![
            key(theme, "↑↓", theme.accent_secondary),
            label(theme, i18n::t(lang, Key::HbNavigate)),
            key(theme, "R", theme.accent_secondary),
            label(theme, i18n::t(lang, Key::HbRecheck)),
        ],
        Tab::Stats => vec![
            key(theme, "S", theme.accent_secondary),
            label(theme, i18n::t(lang, Key::HbNextSession)),
        ],
    });

    spans.push(key(theme, "M", theme.coral));
    spans.push(label(theme, i18n::t(lang, Key::HbMascot)));
    spans.push(key(theme, "V", theme.accent_quaternary));
    spans.push(label(theme, i18n::t(lang, Key::HbSound)));
    spans.push(key(theme, "C", theme.accent_primary));
    spans.push(label(theme, i18n::t(lang, Key::HbConfig)));
    spans.push(key(theme, "H", theme.accent_secondary));
    spans.push(label(theme, i18n::t(lang, Key::SidebarHelp)));
    spans.push(key(theme, "Q", theme.error));
    spans.push(label(theme, i18n::t(lang, Key::HbQuit)));

    spans
}

fn key(theme: &Theme, text: &str, color: Color) -> Span<'static> {
    Span::styled(format!(" {text} "), theme.plate_style(color))
}

fn label(theme: &Theme, text: &str) -> Span<'static> {
    Span::styled(format!(" {text}   "), theme.label_style())
}
