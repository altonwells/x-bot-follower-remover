//! A display-only animation. It never advances work or invents progress.
use crate::{
    app::{App, Mode},
    model::clean,
    protocol::Command,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Paragraph, Wrap},
};

pub fn visible(app: &App) -> bool {
    app.batch.is_some() && app.mode == Mode::Browse
}
pub fn animating(app: &App) -> bool {
    visible(app) && !app.paused && app.sender.is_some()
}

pub fn render(frame: &mut Frame, app: &App, area: Rect, tick: u64) {
    let active = animating(app);
    let phase = if active { (tick % 12) as usize } else { 0 };
    let block = Block::bordered()
        .title(" forgive-me · the cleanse ")
        .border_style(Style::default().fg(Color::Rgb(100, 85, 125)));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let owner = format!("@{}", clean(&app.handle));
    let counter = format!("Reseting followers: {}", app.removed);
    let state = if app.sender.is_none() {
        "Disconnected · work stopped"
    } else if app.paused {
        "Paused · water off"
    } else {
        "Pouring one out for the timeline."
    };
    let target = match app.pending.as_ref().map(|w| &w.command) {
        Some(Command::RemoveFollower { target_id, .. }) => Some(("Rinsing", target_id)),
        _ => app
            .batch
            .as_ref()
            .and_then(|b| b.ids.front())
            .map(|id| ("Next", id)),
    };
    let target = target
        .map(|(label, id)| {
            format!(
                "{label}: @{}",
                app.accounts
                    .get(id)
                    .map(|a| clean(&a.handle))
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| id.clone())
            )
        })
        .unwrap_or_else(|| "Finishing this batch…".into());
    if inner.height < 12 || inner.width < 58 {
        // A compact presentation keeps controls and real progress visible on small terminals.
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(format!("{counter} · {owner}")),
                Line::from(format!(
                    "--(_)> {} X  {target}",
                    if active && phase % 2 == 0 { ":" } else { "." }
                )),
                Line::from(state),
            ]),
            inner,
        );
        return;
    }
    let columns = Layout::horizontal([Constraint::Length(27), Constraint::Min(25)]).split(inner);
    let pouring = active && phase >= 2;
    let ladle = if pouring {
        [r"    \", r"     \", r"      \________", r"       \_______\"]
    } else {
        [r"    \", r"     \", r"      \________", r"       \______/ "]
    };
    let mut art: Vec<Line> = ladle
        .into_iter()
        .map(|s| Line::styled(s, Style::default().fg(Color::Rgb(217, 194, 152))))
        .collect();
    for row in 0..2 {
        let water = if pouring && (phase + row) % 3 != 0 {
            "              :"
        } else {
            "               "
        };
        art.push(Line::styled(water, Style::default().fg(Color::Cyan)));
    }
    for line in [
        r"          \\   /",
        r"           \\ /",
        r"            XX",
        r"           / \\",
        r"          /   \\",
    ] {
        art.push(Line::styled(
            line,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
    }
    let splash = if pouring && phase % 2 == 0 {
        "       .  ~  .  ~  ."
    } else {
        "          ~     ~"
    };
    art.push(Line::styled(
        if active { splash } else { "" },
        Style::default().fg(Color::Cyan),
    ));
    frame.render_widget(Paragraph::new(art), columns[0]);
    let remaining = app.batch.as_ref().map_or(0, |b| b.ids.len());
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::styled(
                owner,
                Style::default()
                    .fg(Color::Rgb(194, 165, 255))
                    .add_modifier(Modifier::BOLD),
            ),
            Line::from(""),
            Line::styled(
                counter,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::from("Verified removals · total for this account"),
            Line::from(format!("{remaining} remaining in this batch")),
            Line::from(""),
            Line::from(target),
            Line::from(""),
            Line::from(state),
        ])
        .wrap(Wrap { trim: false }),
        columns[1],
    );
}
