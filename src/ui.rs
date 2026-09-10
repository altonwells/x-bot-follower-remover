use crate::{
    app::{App, Mode},
    model::{clean, now_ms},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap},
};

pub const HELP: &[(&str, &str)] = &[
    ("↑ ↓ / j k / PgUp PgDn", "Navigate"),
    ("s", "Scan or resume scan"),
    ("f", "Filters and pacing"),
    ("/", "Search"),
    ("m", "Toggle matching-only view"),
    ("Space", "Toggle eligible account"),
    ("a", "Select matching results"),
    ("K", "Keep / unkeep"),
    ("Enter", "Account details"),
    ("o", "Open X profile"),
    ("d", "Review removal batch"),
    ("p", "Pause / resume"),
    ("c", "Cancel pending removals"),
    ("r", "Reconcile one uncertain action"),
    ("Shift+P", "Open pairing guide (pauses work)"),
    ("q / Ctrl-C", "Pause, save and quit"),
];
const ACCENT: Color = Color::Rgb(194, 165, 255);
const MUTED: Color = Color::Rgb(140, 148, 160);
pub fn render(frame: &mut Frame, app: &App, tick: u64) {
    let area = frame.area();
    if area.width < 52 || area.height < 12 {
        frame.render_widget(
            Paragraph::new("forgive-me\nResize to at least 52 × 12. q quits.")
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }
    if app.mode == Mode::Setup {
        crate::setup::render(frame, app);
        return;
    }
    let layout = Layout::vertical([
        Constraint::Length(6),
        Constraint::Min(3),
        Constraint::Length(4),
    ])
    .split(area);
    let connected = app.sender.is_some();
    let status = if !connected {
        "Disconnected"
    } else if app.paused {
        "Paused"
    } else if app.batch.is_some() {
        "Removing"
    } else {
        "Scanning"
    };
    let ritual_visible = crate::ritual::visible(app);
    let header = vec![
        Line::from(vec![
            Span::styled(
                " forgive-me ",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(if app.demo {
                " / demo · no X access".into()
            } else {
                format!(
                    " / @{}",
                    if app.handle.is_empty() {
                        "not connected"
                    } else {
                        &app.handle
                    }
                )
            }),
            Span::styled(
                format!("    {status}"),
                Style::default().fg(if connected {
                    Color::Green
                } else {
                    Color::Yellow
                }),
            ),
        ]),
        if ritual_visible {
            Line::from(format!(
                " {} selected  ·  {} verified removals  ·  {} uncertain",
                app.selected.len(),
                app.removed,
                app.uncertain
            ))
        } else {
            Line::from(format!(
                " {} known followers  ·  {} match  ·  {} selected  ·  {} removed  ·  {} uncertain",
                app.accounts
                    .values()
                    .filter(|a| a.follows_me == Some(true))
                    .count(),
                app.matches(),
                app.selected.len(),
                app.removed,
                app.uncertain
            ))
        },
        Line::from(Span::styled(
            format!(
                " {} days inactive / zero posts: {}  ·  skip verified: {}  ·  skip following: {}",
                app.policy.inactive_days,
                on(app.policy.include_zero_posts),
                on(app.policy.skip_verified),
                on(app.policy.skip_following)
            ),
            Style::default().fg(MUTED),
        )),
        Line::from(format!(
            " {}{}  {}",
            if app.mode == Mode::Search {
                "Search: "
            } else {
                "Filter: "
            },
            app.query,
            if app.only_matching {
                "[matching only]"
            } else {
                "[all collected followers]"
            }
        )),
        Line::from(Span::styled(
            format!(
                " Scan: {}",
                match app.scan.phase.as_str() {
                    "following" => "collecting accounts you follow · follower inventory incomplete",
                    "followers" => "collecting followers · inventory incomplete",
                    "inspect" => "checking activity · follower inventory collected",
                    "done" => "complete",
                    _ => "not started",
                }
            ),
            Style::default().fg(MUTED),
        )),
    ];
    frame.render_widget(Paragraph::new(header), layout[0]);
    let rows = if ritual_visible {
        vec![]
    } else {
        app.visible()
    };
    let now = now_ms();
    let focused = app.focus.min(rows.len().saturating_sub(1));
    let height = usize::from(layout[1].height.saturating_sub(4)).max(1);
    let offset = focused.saturating_sub(height - 1);
    if ritual_visible {
        crate::ritual::render(frame, app, layout[1], tick);
    } else {
        let table_rows = rows.iter().skip(offset).take(height).map(|a| {
            let activity = if a.checked_at_ms.is_none() {
                "Not checked".into()
            } else if a.posts == Some(0) {
                "Zero posts".into()
            } else if let Some(t) = a.last_activity_ms {
                format!("{}d ago", ((now - t) / 86_400_000).max(0))
            } else {
                "Unknown".into()
            };
            let reason = a.reason(&app.policy, now);
            let style = Style::default().fg(if a.kept {
                MUTED
            } else if reason.is_ok() {
                Color::Green
            } else {
                Color::Reset
            });
            Row::new(vec![
                Cell::from(if a.kept {
                    "keep"
                } else if app.selected.contains(&a.id) {
                    "[x]"
                } else {
                    "[ ]"
                }),
                Cell::from(format!("@{}", clean(&a.handle))),
                Cell::from(activity),
                Cell::from(format!(
                    "{} / {}",
                    count(a.followers),
                    count(a.following_count)
                )),
                Cell::from(reason.unwrap_or_else(|s| s)),
            ])
            .style(style)
        });
        let table = Table::new(
            table_rows,
            [
                Constraint::Length(5),
                Constraint::Percentage(25),
                Constraint::Length(13),
                Constraint::Length(17),
                Constraint::Min(10),
            ],
        )
        .header(
            Row::new(["", "Account", "Activity", "Fans / Following", "Reason"])
                .style(Style::default().fg(MUTED))
                .bottom_margin(1),
        )
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::BOTTOM)
                .border_style(Style::default().fg(MUTED)),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::Rgb(45, 37, 63))
                .add_modifier(Modifier::BOLD),
        );
        let mut state =
            TableState::default().with_selected((!rows.is_empty()).then_some(focused - offset));
        frame.render_stateful_widget(table, layout[1], &mut state);
        if rows.is_empty() {
            let r = layout[1];
            frame.render_widget(
                Paragraph::new(
                    "\n  No followers here yet. Connect the Chrome extension and press s.",
                ),
                Rect::new(r.x, r.y + 1, r.width, r.height.saturating_sub(2)),
            );
        }
    }
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                format!(" {}", app.notice),
                Style::default().fg(Color::Yellow),
            )),
            Line::from(if ritual_visible {
                " p pause/resume   c cancel   q quit"
            } else {
                " s scan   f filters   Space select   a matching   K keep   d remove"
            }),
            Line::from(if ritual_visible {
                " Enter details   ? help"
            } else {
                " p pause   c cancel   r reconcile   P pair   ? help   q quit"
            }),
        ])
        .wrap(Wrap { trim: false }),
        layout[2],
    );
    match app.mode {
        Mode::Help => popup(
            frame,
            " Keys ",
            HELP.iter().map(|(k, v)| format!("{k:25} {v}")).collect(),
            65,
            22,
        ),
        Mode::Filters => {
            let labels = [
                format!("Inactive days          {}", app.policy.inactive_days),
                format!("Skip verified          {}", on(app.policy.skip_verified)),
                format!("Skip people I follow   {}", on(app.policy.skip_following)),
                format!(
                    "Include zero posts     {}",
                    on(app.policy.include_zero_posts)
                ),
                format!(
                    "Delay between removes  {} seconds",
                    app.policy.delay_seconds
                ),
                format!("Maximum batch          {} accounts", app.policy.batch_limit),
            ];
            let mut lines: Vec<_> = labels
                .iter()
                .enumerate()
                .map(|(i, s)| format!("{} {s}", if app.filter_row == i { ">" } else { " " }))
                .collect();
            lines.push("".into());
            lines.push("↑ ↓ choose · ← → adjust · Enter saves".into());
            popup(frame, " Cleanup policy ", lines, 56, 12);
        }
        Mode::Details => {
            if let Some(a) = rows.get(focused) {
                popup(
                    frame,
                    " Account evidence ",
                    vec![
                        format!("@{} · {}", clean(&a.handle), clean(&a.name)),
                        format!("ID: {}", a.id),
                        clean(&a.bio),
                        format!(
                            "Follows me: {:?} / I follow: {:?}",
                            a.follows_me, a.i_follow
                        ),
                        format!("Verified: {:?} / Protected: {:?}", a.verified, a.protected),
                        format!("Current posts: {}", count(a.posts)),
                        format!("Evidence: {}", clean(&a.activity_note)),
                        format!(
                            "Decision: {}",
                            a.reason(&app.policy, now).unwrap_or_else(|s| s).to_string()
                        ),
                        "".into(),
                        "Esc closes · o opens profile from the main table".into(),
                    ],
                    75,
                    17,
                );
            }
        }
        Mode::Confirm => {
            let mut lines = vec![
                format!(
                    "Remove {} followers from @{}?",
                    app.confirmation.len(),
                    app.handle
                ),
                "".into(),
                "These accounts will stop following you.".into(),
                "There is no restore-followers action.".into(),
                format!("One at a time · {}s delay", app.policy.delay_seconds),
                "".into(),
            ];
            lines.extend(
                app.confirmation
                    .iter()
                    .take(4)
                    .map(|id| format!("  @{}", clean(&app.accounts[id].handle))),
            );
            lines.push("".into());
            lines.push("y confirms · Enter / n / Esc cancels".into());
            popup(frame, " Confirm removal ", lines, 62, 17);
        }
        _ => {}
    }
}
fn count(n: Option<u64>) -> String {
    n.map(|n| n.to_string()).unwrap_or_else(|| "?".into())
}
fn on(b: bool) -> &'static str {
    if b { "on" } else { "off" }
}
fn popup(frame: &mut Frame, title: &str, lines: Vec<String>, width: u16, height: u16) {
    let a = frame.area();
    let w = width.min(a.width.saturating_sub(2));
    let h = height.min(a.height.saturating_sub(2));
    let r = Rect::new(a.x + (a.width - w) / 2, a.y + (a.height - h) / 2, w, h);
    frame.render_widget(Clear, r);
    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .wrap(Wrap { trim: false })
            .block(
                Block::bordered()
                    .title(title)
                    .border_style(Style::default().fg(ACCENT)),
            ),
        r,
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use ratatui::{Terminal, backend::TestBackend};
    use std::path::Path;
    #[test]
    fn empty_and_narrow_states_render_without_panicking() {
        for (w, h) in [(100, 30), (52, 12), (30, 8)] {
            let app = App::new(Store::open(Path::new(":memory:")).unwrap(), true).unwrap();
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| render(f, &app, 0)).unwrap();
        }
    }
}
