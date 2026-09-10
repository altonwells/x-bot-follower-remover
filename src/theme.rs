//! Shared terminal palette. Standard Unicode; no patched font required.
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Padding},
};

pub const BG: Color = Color::Rgb(11, 17, 24);
pub const PANEL: Color = Color::Rgb(15, 25, 35);
pub const RAISED: Color = Color::Rgb(20, 35, 46);
pub const FOCUS: Color = Color::Rgb(27, 55, 62);
pub const BORDER: Color = Color::Rgb(42, 63, 77);
pub const TEXT: Color = Color::Rgb(222, 234, 239);
pub const MUTED: Color = Color::Rgb(132, 155, 170);
pub const MINT: Color = Color::Rgb(124, 231, 196);
pub const ICE: Color = Color::Rgb(134, 190, 229);
pub const AMBER: Color = Color::Rgb(235, 192, 118);
pub const RED: Color = Color::Rgb(243, 144, 147);

pub fn fg(color: Color) -> Style {
    Style::default().fg(color)
}
pub fn bold(color: Color) -> Style {
    fg(color).add_modifier(Modifier::BOLD)
}
pub fn panel(title: impl Into<Line<'static>>) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(fg(BORDER))
        .style(fg(TEXT).bg(PANEL))
        .title(title.into().style(fg(MUTED)))
        .padding(Padding::horizontal(1))
}
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}
pub fn keys(items: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::with_capacity(items.len() * 2);
    for (key, label) in items {
        spans.push(Span::styled(format!(" {key} "), bold(ICE).bg(RAISED)));
        spans.push(Span::styled(format!(" {label}  "), fg(MUTED)));
    }
    Line::from(spans)
}
