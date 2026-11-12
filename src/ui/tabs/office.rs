use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::App;
use crate::beacon::{self, Agent, Session, State, tool_icon};
use crate::i18n::{self, Key};
use crate::office::{self, Desk, PC_ROWS, PC_W, STATS_ROWS};
use crate::ui::format;
use crate::ui::widgets::panel;

const HUD_ROWS: u16 = 3;

/// Sala de máquinas de los subagentes de la sesión en foco. Empieza vacía y
/// cada subagente aparece como una tarjeta con su computadora dentro (la
/// pantalla según la herramienta que usa) y sus datos en la parte inferior.
/// Las de los terminados se apagan. Abajo, un marcador con los totales.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let lang = app.lang;
    let now = beacon::now();
    let current = app.current();

    let mut block = panel::block(theme, super::Tab::Office.title(lang), theme.tab_accent(5), false);
    let full = block.inner(area);
    // Marcador abajo si hay sitio; la sala ocupa el resto.
    let hud_rows = if full.height >= 12 { HUD_ROWS } else { 0 };
    let inner = Rect { height: full.height - hud_rows, ..full };
    let hud = Rect { y: inner.bottom(), height: hud_rows, ..full };
    let cards = office::layout(inner.width, inner.height);

    let active: Vec<&Agent> = current.map(|s| s.active_agents(now)).unwrap_or_default();
    let finished: Vec<&Agent> = current.map(|s| s.recent_agents(cards.len())).unwrap_or_default();

    if let Some(s) = current {
        let mut readout = i18n::fill(
            i18n::t(lang, Key::OfficeWorkingTemplate),
            &[&active.len().to_string(), &s.agents.iter().filter(|a| a.finished.is_some()).count().to_string()],
        );
        readout = format!("{} · {readout}", s.project);
        if active.len() > cards.len() {
            let more = (active.len() - cards.len()).to_string();
            readout.push_str(&format!(" · {}", i18n::t(lang, Key::OfficeMoreTemplate).replacen("{}", &more, 1)));
        }
        block = block.title_bottom(panel::readout(theme, &readout, theme.fg_muted));
    }
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(office::background(theme, inner.width, inner.height)), inner);

    // Activos primero y después los terminados más recientes.
    let occupants: Vec<(&Agent, bool)> =
        active.iter().map(|a| (*a, true)).chain(finished.iter().map(|a| (*a, false))).collect();
    for (card, (agent, busy)) in cards.iter().zip(&occupants) {
        let rect = Rect { x: inner.x + card.x, y: inner.y + card.y, ..*card };
        render_card(frame, rect, app, agent, *busy, now);
    }

    if occupants.is_empty() && inner.height > 2 {
        let text = i18n::t(lang, Key::OfficeEmpty);
        let width = (text.chars().count() as u16 + 2).min(inner.width);
        let row = Rect { x: inner.x + (inner.width - width) / 2, y: inner.y + inner.height / 2, width, height: 1 };
        frame.render_widget(Clear, row);
        frame.render_widget(Paragraph::new(format!(" {text} ")).style(theme.plate_style(theme.fg_muted)), row);
    }

    if hud.height > 0 {
        render_hud(frame, hud, app, current, active.len(), now);
    }
}

/// Tarjeta de un subagente: panel con su tipo en la placa y el tiempo en el
/// pie; dentro, la computadora y debajo estado, herramienta y descripción.
fn render_card(frame: &mut Frame, area: Rect, app: &App, a: &Agent, busy: bool, now: f64) {
    let theme = &app.theme;
    let lang = app.lang;
    let accent = office::shirt(&a.kind);
    let elapsed = format::dur(a.finished.unwrap_or(now) - a.started);
    let block = panel::block(theme, &a.kind, if busy { accent } else { theme.fg_muted }, busy)
        .title_bottom(panel::readout(theme, &elapsed, if busy { theme.accent_secondary } else { theme.fg_muted }));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let desk = if busy {
        Desk::Busy { work: office::work_for(&a.tool), accent }
    } else {
        let since = (now - a.finished.unwrap_or(now)).max(0.0);
        Desk::Gone { accent, since_ds: (since * 10.0).min(1e6) as u32 }
    };
    let pc = Rect {
        x: inner.x + inner.width.saturating_sub(PC_W) / 2,
        y: inner.y,
        width: PC_W.min(inner.width),
        height: PC_ROWS.min(inner.height),
    };
    frame.render_widget(Paragraph::new(office::computer_lines(theme, desk, app.tick)), pc);

    let stats = Rect { y: inner.y + PC_ROWS, height: inner.height.saturating_sub(PC_ROWS).min(STATS_ROWS), ..inner };
    let width = stats.width as usize;
    let lines = if busy {
        let work = office::work_for(&a.tool);
        let detail = if a.tool.is_empty() {
            "--".to_string()
        } else {
            let tool = a.tool.split_whitespace().next().unwrap_or("");
            format!("{} {}", tool_icon(tool), a.tool)
        };
        vec![
            Line::from(vec![
                Span::styled("● ", theme.style_for_color(work.led(theme))),
                Span::styled(i18n::state_label(lang, work.state()).to_uppercase(), theme.style_for_bold()),
            ]),
            Line::from(Span::styled(format::fit(&detail, width), theme.readout_style())),
            Line::from(Span::styled(format::fit(&a.description, width), theme.label_style())),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled("✔ ", theme.style_for_color(theme.ok)),
                Span::styled(i18n::state_label(lang, State::Done).to_uppercase(), theme.label_style()),
            ]),
            Line::from(Span::styled(format::fit(&a.description, width), theme.style_for_color(theme.fg_dim))),
        ]
    };
    frame.render_widget(Paragraph::new(lines).style(theme.panel_style()), stats);
}

/// Marcador de totales: proyecto, subagentes, tareas, coste y tiempo del turno.
fn render_hud(frame: &mut Frame, area: Rect, app: &App, current: Option<&Session>, active: usize, now: f64) {
    let theme = &app.theme;
    let lang = app.lang;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_style(false))
        .style(theme.raised_style())
        .title(panel::plate(theme, i18n::t(lang, Key::OfficeHudTitle), theme.accent_primary));
    let bg = Style::default().bg(theme.bg_raised);
    let label = |text: &str| Span::styled(format!("{text} "), theme.label_style().patch(bg));
    let value = |text: String| Span::styled(text, theme.readout_style().patch(bg));
    let sep = || Span::styled(" │ ", theme.style_for_color(theme.fg_dim).patch(bg));

    let spans = match current {
        Some(s) => {
            let done = s.tasks.iter().filter(|t| t.status == "completed").count();
            let time = match (s.turn_started, s.last_turn) {
                (Some(start), _) => format::dur(now - start),
                (None, Some(last)) => format::dur(last),
                _ => "--".into(),
            };
            vec![
                Span::styled("▶ ", theme.style_for_color(theme.accent_primary).patch(bg)),
                Span::styled(s.project.to_uppercase(), theme.style_for_bold().patch(bg)),
                sep(),
                label(i18n::t(lang, Key::OfficeHudAgents)),
                value(format!("{active:02}/{:02}", s.agents.len())),
                sep(),
                label(i18n::t(lang, Key::OfficeHudTasks)),
                value(format!("{done:02}/{:02}", s.tasks.len())),
                sep(),
                label(i18n::t(lang, Key::OfficeHudCost)),
                value(format::money(s.usd)),
                sep(),
                label(i18n::t(lang, Key::OfficeHudTime)),
                value(time),
            ]
        }
        None => vec![label(i18n::t(lang, Key::OfficeEmpty))],
    };
    frame.render_widget(Paragraph::new(Line::from(spans)).alignment(Alignment::Center).block(block), area);
}

#[cfg(test)]
mod tests {
    use crate::app::tests::app;
    use crate::event::AppEvent;
    use ratatui::{Terminal, backend::TestBackend};
    use serde_json::json;

    fn screen(app: &crate::app::App, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| super::render(f, f.area(), app)).unwrap();
        let buf = t.backend().buffer().clone();
        (0..h).map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n").collect()
    }

    fn with_agents(n: usize) -> crate::app::App {
        let mut a = app();
        let mut events = vec![json!({"sid": "s", "project": "web", "kind": "session_start", "state": "delegating"})];
        let tools = ["Grep fn main", "Read src/lib.rs", "Edit a.rs", "Bash cargo test", "WebFetch https://x", ""];
        for i in 0..n {
            let id = format!("ag{i}");
            events.push(json!({"sid": "s", "project": "web", "kind": "agent_start", "agentId": id, "type": format!("Tipo{i}"), "description": format!("tarea {i}")}));
            let tool = tools[i % tools.len()];
            if let Some((name, detail)) = tool.split_once(' ') {
                events.push(json!({"sid": "s", "project": "web", "kind": "agent_tool", "agentId": id, "tool": name, "detail": detail}));
            }
        }
        a.on_event(AppEvent::Beacon(events));
        a
    }

    #[test]
    fn sin_subagentes_muestra_la_oficina_vacia() {
        let a = with_agents(0);
        assert!(screen(&a, 80, 24).contains("Sin subagentes"));
    }

    #[test]
    fn cada_subagente_tiene_su_etiqueta() {
        let a = with_agents(3);
        let out = screen(&a, 120, 50);
        for i in 0..3 {
            assert!(out.contains(&format!("TIPO{i}")), "falta TIPO{i}\n{out}");
        }
        assert!(out.contains("Grep fn main"));
        assert!(out.contains("trabajando: 3"));
    }

    #[test]
    fn avisa_de_los_que_no_caben() {
        let a = with_agents(8);
        let out = screen(&a, 80, 24);
        assert!(out.contains("más"), "{out}");
    }

    #[test]
    fn los_terminados_dejan_el_puesto_vacio() {
        let mut a = with_agents(2);
        a.on_event(AppEvent::Beacon(vec![json!({"sid": "s", "project": "web", "kind": "agent_done", "agentId": "ag1"})]));
        let out = screen(&a, 120, 50);
        assert!(out.contains("TIPO1") && out.contains("✔"));
        assert!(out.contains("trabajando: 1 · terminados: 1"));
    }

    #[test]
    fn areas_minimas_sin_panico() {
        let a = with_agents(4);
        for (w, h) in [(1, 1), (10, 3), (30, 8), (160, 50)] {
            screen(&a, w, h);
        }
    }
}
