use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::theme::{Theme, segmented_bar};

use super::panel;

/// Instrumento de aguja segmentada. Ocupa 3 filas (marco + fila de barra) y se
/// usa para los límites de la cuenta y el contexto. Sin lectura, la barra
/// queda apagada y marca `--`.
pub fn render(frame: &mut Frame, area: Rect, theme: &Theme, label: &str, detail: &str, pct: Option<f64>) {
    let color = pct.map_or(theme.fg_muted, |p| theme.color_for(theme.status_for_pct(p)));

    let mut block = panel::block(theme, label, color, false);
    if !detail.is_empty() {
        block = block.title_bottom(panel::readout(theme, detail, theme.fg_muted));
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let bar_width = inner.width.saturating_sub(8).max(1) as usize;
    let (filled, empty) = segmented_bar(pct.unwrap_or(0.0), bar_width);
    let reading = pct.map_or("    --".to_string(), |p| format!(" {p:>5.1}%"));

    let line = Line::from(vec![
        Span::styled(filled, theme.style_for_color(color)),
        Span::styled(empty, theme.style_for_color(theme.fg_dim)),
        Span::styled(reading, theme.readout_style()),
    ]);

    frame.render_widget(Paragraph::new(line), inner);
}
