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
    (app.batch.is_some() || app.auto_policy.is_some())
        && !app.show_queue_list
        && matches!(app.mode, Mode::Browse | Mode::Settings)
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

pub struct Scene<'a> {
    pub handle: &'a str,
    pub removed: usize,
    pub active: bool,
    pub motion: crate::insect::Motion,
    pub state: String,
    pub target_label: String,
    pub removing: Option<String>,
    pub policy: &'a crate::model::Policy,
    pub remaining: usize,
    pub animation: &'a Animation,
}
pub fn render(frame: &mut Frame, app: &App, area: Rect, tick: u64) {
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
    let scene = Scene {
        handle: &app.handle,
        removed: app.removed,
        active: animating(app),
        motion: crate::insect::Motion::from_work(
            animating(app),
            app.pacing.remaining_seconds() > 0,
            &app.scan.phase,
            target.is_some_and(|(label, _)| label == "Removing"),
        ),
        state,
        target_label,
        removing: if target.is_some_and(|(label, _)| label == "Removing") {
            handle
        } else {
            None
        },
        policy: app
            .batch
            .as_ref()
            .map(|b| &b.policy)
            .or(app.auto_policy.as_ref())
            .unwrap_or(&app.policy),
        remaining: app.batch.as_ref().map_or(0, |b| b.ids.len()),
        animation: &app.animation,
    };
    render_scene(frame, &scene, area, tick);
}
pub fn render_scene(frame: &mut Frame, scene: &Scene<'_>, area: Rect, tick: u64) {
    let active = scene.active;
    let phase = if active { tick as usize } else { 0 };
    if area.width >= 112 && area.height >= 16 {
        frame.render_widget(
            ratatui::widgets::Block::default().style(fg(TEXT).bg(PANEL)),
            area,
        );
        let [left, right] = Layout::horizontal([
            Constraint::Length((area.width / 3).clamp(43, 56)),
            Constraint::Min(60),
        ])
        .spacing(1)
        .areas(area);
        let block = panel(" THE CLEANSE ");
        let inner = block.inner(left);
        frame.render_widget(block, left);
        let art = centered(
            Rect::new(
                inner.x,
                inner.y,
                inner.width,
                inner.height.saturating_sub(2),
            ),
            40,
            25,
        );
        render_water(frame, scene, art, tick);
        ink(
            frame,
            inner,
            0,
            inner.height.saturating_sub(2),
            &format!("Reseting followers: {}", scene.removed),
            bold(MINT),
        );
        ink(
            frame,
            inner,
            0,
            inner.height.saturating_sub(1),
            &scene.state,
            fg(MUTED),
        );
        let age = scene
            .animation
            .departures
            .back()
            .map(|d| scene.animation.clock_ms.saturating_sub(d.at_ms));
        crate::insect::render(frame, right, scene.motion, scene.animation.clock_ms, age);
        return;
    }
    let block = panel(" THE CLEANSE ")
        .title_top(Line::styled(" , SYSTEM SETTINGS ", fg(ICE)).right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let owner = format!("@{}", clean(scene.handle));
    let counter = format!("Reseting followers: {}", scene.removed);
    let state = scene.state.clone();
    let target_label = scene.target_label.clone();
    let policy = scene.policy;
    let remaining = scene.remaining;
    if inner.height < 12 || inner.width < 64 {
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
                    scene
                        .animation
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
    let art_width = if stage.width >= 83 { 45 } else { 36 };
    let [art, detail] = Layout::horizontal([Constraint::Length(art_width), Constraint::Min(24)])
        .spacing(4)
        .areas(stage);
    render_water(frame, scene, art, tick);
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
    if let Some(last) = scene.animation.departures.back() {
        lines.extend([
            Line::from(""),
            Line::styled(format!("✓ Removed @{}", last.handle), bold(MINT)),
        ]);
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), detail);
}

fn render_water(frame: &mut Frame, scene: &Scene<'_>, art: Rect, tick: u64) {
    let active = scene.active;
    let phase = if active { tick as usize } else { 0 };
    let now = if active {
        tick.saturating_mul(50)
    } else {
        scene.animation.clock_ms
    };
    let tall = art.height >= 24;
    let medium = art.height >= 17;
    // Keep the original v0.1.11 composition; shorten its vertical spacing at smaller sizes.
    // The ladle lip and stream stay aligned at column 23 in every layout.
    let ladle: &[&str] = if tall {
        &[
            "     ╲╲",
            "       ╲╲",
            "         ╲╲",
            "           ╲╲━━━━━━━━━━╮",
            "            ╲≋≋≋≋≋≋≋≋╲",
            "             ╰━━━━━━━━╲",
        ]
    } else if medium {
        &[
            "         ╲╲",
            "           ╲╲━━━━━━━━━━╮",
            "            ╲≋≋≋≋≋≋≋≋╲",
            "             ╰━━━━━━━━╲",
        ]
    } else {
        &[
            "           ╲╲━━━━━━━━━━╮",
            "            ╲≋≋≋≋≋≋≋≋╲",
            "             ╰━━━━━━━━╲",
        ]
    };
    let logo: &[&str] = if tall {
        &[
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
    } else if medium {
        &[
            "███           ███",
            "  ███       ███",
            "    ███   ███",
            "      █████",
            "    ███   ███",
            "  ███       ███",
            "███           ███",
        ]
    } else {
        &[
            "███           ███",
            "   ███     ███",
            "      █████",
            "   ███     ███",
            "███           ███",
        ]
    };
    let stream_start = ladle.len() as u16;
    let logo_y = if tall {
        12
    } else if medium {
        7
    } else {
        4
    };
    let splash_y = logo_y + logo.len() as u16 + u16::from(tall || !medium);
    for (row, line) in ladle.iter().enumerate() {
        ink(
            frame,
            art,
            0,
            row as u16,
            line,
            bold(if row + 2 == ladle.len() { ICE } else { TEXT }),
        );
    }
    if active {
        for row in stream_start..splash_y {
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
            splash_y,
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
            splash_y + 1,
            if phase % 6 < 3 {
                "──────≈≈≈────────≈≈≈──────"
            } else {
                "───≈≈≈────────────≈≈≈─────"
            },
            fg(BORDER),
        );
    } else {
        ink(
            frame,
            art,
            9,
            splash_y + 1,
            "────────────────────────",
            fg(BORDER),
        );
    }
    for (row, line) in logo.iter().enumerate() {
        ink(
            frame,
            art,
            15,
            logo_y + row as u16,
            line,
            bold(if active && (phase / 2 + row) % 6 < 2 {
                ICE
            } else {
                TEXT
            }),
        );
    }
    for departure in &scene.animation.departures {
        let age = now.saturating_sub(departure.at_ms);
        if age < 8000 {
            tag(
                frame,
                art,
                stream_start + (age * u64::from(splash_y - stream_start - 1) / 8000) as u16,
                &departure.handle,
                true,
            );
        }
    }
    if let Some(handle) = &scene.removing {
        tag(frame, art, stream_start, handle, false);
    }
}
