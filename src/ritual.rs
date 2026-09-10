//! A display-only animation. It never advances work or invents progress.
use crate::{
    app::{App, Mode},
    model::clean,
    protocol::Command,
    theme::*,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::Line,
    widgets::{Paragraph, Wrap},
};

pub fn visible(app: &App) -> bool {
    app.batch.is_some() && app.mode == Mode::Browse
}
pub fn animating(app: &App) -> bool {
    visible(app) && !app.paused && app.sender.is_some()
}

pub fn render(frame: &mut Frame, app: &App, area: Rect, tick: u64) {
    let active = animating(app);
    let phase = if active { (tick % 16) as usize } else { 0 };
    let block = panel(" THE CLEANSE ")
        .title_top(Line::styled(" ONE ACCOUNT AT A TIME ", fg(MUTED)).right_aligned());
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
    if inner.height < 15 || inner.width < 75 {
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(format!("{counter} · {owner}"), bold(MINT)),
                Line::styled(
                    format!(
                        "--(_)> {} X  {target}",
                        if active && phase % 2 == 0 { ":" } else { "." }
                    ),
                    fg(ICE),
                ),
                Line::styled(state, fg(if active { MUTED } else { AMBER })),
            ]),
            inner,
        );
        return;
    }
    let stage = centered(inner, 91, 19);
    let [art_area, detail] = Layout::horizontal([Constraint::Length(39), Constraint::Min(30)])
        .spacing(3)
        .areas(stage);
    let pouring = active && phase >= 3;
    let mut art = vec![
        Line::styled("          ╲", fg(MUTED)),
        Line::styled("           ╲", fg(MUTED)),
        Line::styled("            ╲_________", fg(TEXT)),
        Line::styled(
            if pouring {
                "             ╲________╲"
            } else {
                "             ╲________/"
            },
            fg(TEXT),
        ),
    ];
    for row in 0..3 {
        art.push(Line::styled(
            if pouring {
                match (phase + row) % 4 {
                    0 => "                      ╎",
                    1 => "                     ·┊",
                    2 => "                      ┊·",
                    _ => "                     ╎╎",
                }
            } else {
                ""
            },
            fg(ICE),
        ));
    }
    for line in [
        "              ██       ██",
        "               ██     ██",
        "                ██   ██",
        "                 ██ ██",
        "                  ███",
        "                 ██ ██",
        "                ██   ██",
        "               ██     ██",
        "              ██       ██",
    ] {
        art.push(Line::styled(line, bold(TEXT)));
    }
    art.push(Line::styled(
        if pouring {
            if phase % 2 == 0 {
                "          ·  ˙  ~  ·  ~  ˙  ·"
            } else {
                "            ~  ·  ˙  ~  ·  ~"
            }
        } else {
            ""
        },
        fg(ICE),
    ));
    art.push(Line::styled("           ─────────────────", fg(BORDER)));
    frame.render_widget(Paragraph::new(art), art_area);
    let remaining = app.batch.as_ref().map_or(0, |b| b.ids.len());
    let cadence = app
        .batch
        .as_ref()
        .map_or(app.policy.delay_seconds, |b| b.policy.delay_seconds);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::styled("A FRESH START", fg(MUTED)),
            Line::styled(owner, bold(ICE)),
            Line::from(""),
            Line::styled(counter, bold(MINT)),
            Line::styled("Verified removals · account total", fg(MUTED)),
            Line::from(""),
            Line::styled(
                format!("{remaining} queued  /  {cadence}s interval"),
                fg(TEXT),
            ),
            Line::styled(target, fg(ICE)),
            Line::from(""),
            Line::styled(state, fg(if active { MINT } else { AMBER })),
            Line::from(""),
            Line::styled("A little less noise.", fg(MUTED)),
            Line::styled("A little more you.", fg(MUTED)),
        ])
        .wrap(Wrap { trim: false }),
        detail,
    );
}
