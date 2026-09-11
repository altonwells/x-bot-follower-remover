//! Display-only water and receipt tags. This clock never advances browser work.
use crate::{
    app::{App, Mode},
    model::clean,
    protocol::Command,
    theme::*,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Paragraph, Wrap},
};
use std::{collections::VecDeque, time::Duration};

#[derive(Default)]
pub struct Animation {
    pub clock_ms: u64,
    pub departures: VecDeque<Departure>,
}
pub struct Departure {
    pub handle: String,
    pub at_ms: u64,
}
impl Animation {
    pub fn advance(&mut self, elapsed: Duration) {
        let ms = elapsed.as_millis().min(1000) as u64;
        self.clock_ms = self.clock_ms.saturating_add(ms);
        self.departures
            .retain(|d| self.clock_ms.saturating_sub(d.at_ms) < 8000);
    }
    pub fn removed(&mut self, handle: String) {
        if self.departures.len() == 8 {
            self.departures.pop_front();
        }
        self.departures.push_back(Departure {
            handle: clean(&handle),
            at_ms: self.clock_ms,
        });
    }
}
pub fn visible(app: &App) -> bool {
    app.batch.is_some() && !app.show_queue_list && matches!(app.mode, Mode::Browse | Mode::Settings)
}
pub fn animating(app: &App) -> bool {
    visible(app) && !app.paused && app.sender.is_some()
}
fn ink(frame: &mut Frame, area: Rect, x: u16, y: u16, text: &str, style: Style) {
    if x < area.width && y < area.height {
        frame.buffer_mut().set_stringn(
            area.x + x,
            area.y + y,
            text,
            usize::from(area.width - x),
            style,
        );
    }
}
fn tag(frame: &mut Frame, area: Rect, row: u16, handle: &str, confirmed: bool) {
    let label = if confirmed { "✓ REMOVED" } else { "REMOVING" };
    let color = if confirmed { MINT } else { ICE };
    let handle: String = clean(handle).chars().take(20).collect();
    ink(
        frame,
        area,
        10,
        row,
        &format!("╭─ {label:─<21}─╮"),
        fg(color).bg(RAISED),
    );
    ink(
        frame,
        area,
        10,
        row + 1,
        &format!("│ @{handle:<20}  │"),
        bold(TEXT).bg(RAISED),
    );
    ink(
        frame,
        area,
        10,
        row + 2,
        "╰────────────────────────╯",
        fg(color).bg(RAISED),
    );
}

pub fn render(frame: &mut Frame, app: &App, area: Rect, tick: u64) {
    let active = animating(app);
    let phase = if active { tick as usize } else { 0 };
    let now = if active {
        tick.saturating_mul(50)
    } else {
        app.animation.clock_ms
    };
    let block = panel(" THE CLEANSE ")
        .title_top(Line::styled(" , SYSTEM SETTINGS ", fg(ICE)).right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let owner = format!("@{}", clean(&app.handle));
    let counter = format!("Reseting followers: {}", app.removed);
    let state = if app.sender.is_none() {
        "Disconnected · work stopped".into()
    } else if app.paused {
        "Paused · water stopped".into()
    } else if app.pacing.remaining_seconds() > 0 {
        format!(
            "Next attempt in {}s · water keeps flowing",
            app.pacing.remaining_seconds()
        )
    } else {
        "Queue running · removals confirmed by X".into()
    };
    let target = match app.pending.as_ref().map(|w| &w.command) {
        Some(Command::RemoveFollower { target_id, .. }) => Some(("Removing", target_id)),
        Some(Command::InspectAccount { target_id, .. }) => Some(("Checking", target_id)),
        _ => app
            .batch
            .as_ref()
            .and_then(|b| b.ids.front())
            .map(|id| ("Next", id)),
    };
    let handle = target.map(|(_, id)| {
        app.accounts
            .get(id)
            .map(|a| clean(&a.handle))
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| id.clone())
    });
    let target_label = target
        .zip(handle.as_ref())
        .map(|((label, _), handle)| format!("{label}: @{handle}"))
        .unwrap_or_else(|| {
            if app.auto_policy.is_some() {
                format!("Full Auto: {}", app.scan.phase)
            } else {
                "Queue complete".into()
            }
        });
    if inner.height < 20 || inner.width < 85 {
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(format!("{counter} · {owner}"), bold(MINT)),
                Line::styled(
                    format!(
                        "╲___╲ {} X  {target_label}",
                        if active && phase % 2 == 0 {
                            "≋┊≋"
                        } else {
                            "┊≋┊"
                        }
                    ),
                    fg(ICE),
                ),
                Line::styled(state, fg(if active { MINT } else { AMBER })),
                Line::styled(
                    app.animation
                        .departures
                        .back()
                        .map(|d| format!("✓ Removed @{}", d.handle))
                        .unwrap_or_default(),
                    fg(MINT),
                ),
                Line::styled(", system settings", fg(MUTED)),
            ]),
            inner,
        );
        return;
    }
    let stage = centered(inner, 112, 25);
    let [art, detail] = Layout::horizontal([Constraint::Length(45), Constraint::Min(34)])
        .spacing(4)
        .areas(stage);
    // A permanently tipped ladle meets the stream at column 23.
    for (row, line) in [
        "     ╲╲",
        "       ╲╲",
        "         ╲╲",
        "           ╲╲━━━━━━━━━━╮",
        "            ╲≋≋≋≋≋≋≋≋╲",
        "             ╰━━━━━━━━╲",
    ]
    .iter()
    .enumerate()
    {
        ink(
            frame,
            art,
            0,
            row as u16,
            line,
            bold(if row == 4 { ICE } else { TEXT }),
        );
    }
    if active {
        for row in 6..22 {
            let water =
                [" ·│┊│· ", "  ┊┃┊  ", " ˙│┃│˙ ", "  ╎┃╎  "][(phase / 2 + row as usize) % 4];
            ink(
                frame,
                art,
                20,
                row,
                water,
                fg(if row % 3 == phase as u16 % 3 {
                    MINT
                } else {
                    ICE
                }),
            );
        }
        ink(
            frame,
            art,
            9,
            22,
            if phase % 4 < 2 {
                "  ˙  ·  ╱ ≋≋≋≋≋ ╲  ·  ˙"
            } else {
                " ·  ˙  ╱ ≋≋≋≋≋≋≋ ╲  ˙  ·"
            },
            fg(ICE),
        );
        ink(
            frame,
            art,
            7,
            23,
            if phase % 6 < 3 {
                "──────≈≈≈────────≈≈≈──────"
            } else {
                "───≈≈≈────────────≈≈≈─────"
            },
            fg(BORDER),
        );
    } else {
        ink(frame, art, 9, 23, "────────────────────────", fg(BORDER));
    }
    for (row, line) in [
        "███           ███",
        "  ███       ███",
        "    ███   ███",
        "      █████",
        "       ███",
        "      █████",
        "    ███   ███",
        "  ███       ███",
        "███           ███",
    ]
    .iter()
    .enumerate()
    {
        ink(
            frame,
            art,
            15,
            12 + row as u16,
            line,
            bold(if active && (phase / 2 + row) % 6 < 2 {
                ICE
            } else {
                TEXT
            }),
        );
    }
    for departure in &app.animation.departures {
        let age = now.saturating_sub(departure.at_ms);
        if age < 8000 {
            tag(
                frame,
                art,
                6 + (age * 15 / 8000) as u16,
                &departure.handle,
                true,
            );
        }
    }
    if target.is_some_and(|(label, _)| label == "Removing") {
        tag(frame, art, 6, handle.as_deref().unwrap_or("unknown"), false);
    }
    let policy = app
        .batch
        .as_ref()
        .map(|b| &b.policy)
        .or(app.auto_policy.as_ref())
        .unwrap_or(&app.policy);
    let remaining = app.batch.as_ref().map_or(0, |b| b.ids.len());
    let mut lines = vec![
        Line::from(""),
        Line::styled("FOLLOWER CLEANUP", fg(MUTED)),
        Line::styled(owner, bold(ICE)),
        Line::from(""),
        Line::styled(counter, bold(MINT)),
        Line::styled("Confirmed removals · account total", fg(MUTED)),
        Line::from(""),
        Line::styled(
            format!(
                "{} queued · {}s removal interval",
                remaining, policy.delay_seconds
            ),
            fg(TEXT),
        ),
        Line::styled(target_label, fg(ICE)),
        Line::from(""),
        Line::styled(state, fg(if active { MINT } else { AMBER })),
        Line::from(""),
        Line::styled(
            format!(
                "{} attempts / batch · {}s rest",
                policy.rest_every, policy.rest_seconds
            ),
            fg(ICE),
        ),
        Line::styled(
            ", system settings · interval, batch size and rests",
            fg(MUTED),
        ),
    ];
    if let Some(last) = app.animation.departures.back() {
        lines.extend([
            Line::from(""),
            Line::styled(format!("✓ Removed @{}", last.handle), bold(MINT)),
        ]);
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), detail);
}
