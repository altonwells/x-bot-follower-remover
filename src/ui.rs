use crate::{
    app::{App, Mode},
    model::{Account, clean, now_ms, post_volume},
    theme::{self, *},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    text::{Line, Span},
    widgets::{
        Block, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table,
        TableState, Wrap,
    },
};

pub const HELP: &[(&str, &str)] = &[
    ("↑ ↓ / j k / PgUp PgDn", "Navigate"),
    ("s", "1. Collect followers (no activity check)"),
    ("i", "2. Check activity from the top of the list"),
    ("f", "Removal rules: choose candidates and protections"),
    ("/", "Search"),
    ("m", "View candidates for removal / all followers"),
    ("Space", "Select / deselect by basic rules"),
    ("a", "Select checked removal candidates in this view"),
    (
        "Shift+A",
        "Select basic matches; i clears activity before queueing",
    ),
    ("K", "Keep / unkeep"),
    ("Enter", "Account details"),
    ("o", "Open X profile"),
    ("d", "4. Review and approve bulk removal"),
    ("b", "Run approved queue in background"),
    ("v", "Queue: switch list / pouring animation"),
    ("p", "Pause / resume"),
    ("c", "Cancel pending removals"),
    ("r", "Reconcile one uncertain action"),
    ("Shift+P", "Open pairing guide (pauses work)"),
    ("q / Ctrl-C", "Pause, save and quit"),
];

pub fn render(frame: &mut Frame, app: &mut App, tick: u64) {
    let area = frame.area();
    frame.render_widget(Block::default().style(fg(TEXT).bg(BG)), area);
    if area.width < 52 || area.height < 12 {
        frame.render_widget(
            Paragraph::new("forgive-me\nResize to at least 52 × 12.\nCtrl-C pauses and quits.")
                .style(fg(MUTED))
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }
    if app.mode == Mode::Setup {
        crate::setup::render(frame, app);
        return;
    }
    let spacious = area.height >= 28 && area.width >= 80;
    let area = area.inner(Margin::new(1, 0));
    let [header, metrics, progress, body, footer] = Layout::vertical([
        Constraint::Length(if spacious { 3 } else { 2 }),
        Constraint::Length(if spacious { 5 } else { 2 }),
        Constraint::Length(if spacious { 3 } else { 1 }),
        Constraint::Min(3),
        Constraint::Length(if spacious { 4 } else { 3 }),
    ])
    .areas(area);
    render_header(frame, app, header);
    render_metrics(frame, app, metrics, spacious);
    render_progress(frame, app, progress, spacious);
    if crate::ritual::visible(app) {
        crate::ritual::render(frame, app, body, tick);
    } else if area.width >= 110 && body.height >= 12 {
        let [inventory, evidence] =
            Layout::horizontal([Constraint::Min(65), Constraint::Length(35)])
                .spacing(1)
                .areas(body);
        let rows = app.visible();
        render_inventory(frame, app, inventory, &rows);
        render_evidence(
            frame,
            app,
            evidence,
            rows.get(app.focus.min(rows.len().saturating_sub(1)))
                .copied(),
        );
    } else {
        render_inventory(frame, app, body, &app.visible());
    }
    render_footer(frame, app, footer);
    render_modal(frame, app);
}

fn status(app: &App) -> (&'static str, ratatui::style::Color) {
    if app.sender.is_none() {
        ("OFFLINE", AMBER)
    } else if app.handle.is_empty() {
        ("CONNECTING", ICE)
    } else if app.uncertain > 0 {
        ("NEEDS REVIEW", RED)
    } else if app.paused {
        ("PAUSED", AMBER)
    } else if app.batch.is_some() {
        ("REMOVING", MINT)
    } else if matches!(
        app.scan.phase.as_str(),
        "following" | "followers" | "inspect"
    ) {
        ("SCANNING", ICE)
    } else {
        ("READY", MINT)
    }
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let [brand, session] =
        Layout::horizontal([Constraint::Min(25), Constraint::Length(23)]).areas(area);
    let identity = if app.demo {
        "DEMO / no X access".into()
    } else if app.handle.is_empty() {
        "Your account, with intention.".into()
    } else {
        format!("@{} / follower control", clean(&app.handle))
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(" ◈ ", bold(MINT)),
                Span::styled("forgive-me", bold(TEXT)),
                Span::styled("  /  X", fg(MUTED)),
            ]),
            Line::styled(format!(" {identity}"), fg(MUTED)),
        ]),
        brand,
    );
    let (label, color) = status(app);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(format!(" ● {label} "), bold(color).bg(RAISED))),
            Line::styled(
                format!("LOCAL / v{} ", env!("CARGO_PKG_VERSION")),
                fg(MUTED),
            ),
        ])
        .right_aligned(),
        session,
    );
}

fn render_metrics(frame: &mut Frame, app: &App, area: Rect, spacious: bool) {
    let items = if let Some(batch) = app.batch.as_ref().filter(|_| crate::ritual::visible(app)) {
        // Animated frames only read queue counters, never walk the follower inventory.
        [
            ("QUEUED", batch.ids.len(), "remaining in batch", TEXT),
            (
                "INTERVAL",
                batch.policy.delay_seconds as usize,
                "seconds per removal",
                ICE,
            ),
            ("SELECTED", app.selected.len(), "selected overall", ICE),
            ("REMOVED", app.removed, "verified total", MINT),
            (
                "UNCERTAIN",
                app.uncertain,
                "need reconciliation",
                if app.uncertain > 0 { RED } else { MUTED },
            ),
        ]
    } else {
        let followers = app
            .accounts
            .values()
            .filter(|a| a.follows_me == Some(true))
            .count();
        [
            ("FOLLOWERS", followers, "collected", TEXT),
            ("CANDIDATES", app.matches(), "eligible for removal", MINT),
            ("SELECTED", app.selected.len(), "ready for review", ICE),
            ("REMOVED", app.removed, "verified total", MINT),
            (
                "UNCERTAIN",
                app.uncertain,
                "need reconciliation",
                if app.uncertain > 0 { RED } else { MUTED },
            ),
        ]
    };
    let columns = Layout::horizontal([Constraint::Ratio(1, 5); 5])
        .spacing(u16::from(spacious))
        .split(area);
    for ((label, value, caption, color), column) in items.into_iter().zip(columns.iter()) {
        if spacious {
            let caption = if column.width < 24 {
                match label {
                    "FOLLOWERS" => "collected",
                    "CANDIDATES" => "to remove",
                    "SELECTED" => "to review",
                    "REMOVED" => "verified",
                    "UNCERTAIN" => "to check",
                    "INTERVAL" => "seconds",
                    _ => "remaining",
                }
            } else {
                caption
            };
            frame.render_widget(
                Paragraph::new(vec![
                    Line::styled(number(value as u64), bold(color)),
                    Line::styled(caption, fg(MUTED)),
                ])
                .block(panel(format!(" {label} "))),
                *column,
            );
        } else {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::styled(label, fg(MUTED)),
                    Line::styled(number(value as u64), bold(color)),
                ]),
                *column,
            );
        }
    }
}

fn render_progress(frame: &mut Frame, app: &App, area: Rect, spacious: bool) {
    let current = if app.batch.is_some() {
        "4 / REMOVAL QUEUE"
    } else {
        match app.scan.phase.as_str() {
            "following" => "1 / COLLECT: people you follow",
            "followers" => "1 / COLLECT: your followers",
            "review" => "2 / READY TO CHECK ACTIVITY: press i",
            "inspect" => "2 / CHECKING ACTIVITY",
            "done" => "3 / REVIEW CANDIDATES: a selects, d starts review",
            _ => "1 / COLLECT FOLLOWERS: press s",
        }
    };
    let mut lines = vec![Line::styled(format!(" {current}"), bold(ICE))];
    if spacious {
        let rule = format!(
            " REMOVE candidates: inactive {}d{} · {} · {}",
            app.policy.inactive_days,
            if app.policy.include_zero_posts {
                " OR zero posts"
            } else {
                ""
            },
            if app.policy.skip_verified {
                "unverified only"
            } else {
                "any verification"
            },
            if app.policy.skip_following {
                "you don't follow"
            } else {
                "any following"
            }
        );
        lines.push(Line::styled(rule, fg(MUTED)));
        lines.push(Line::styled(if app.pacing.remaining_seconds() > 0 && !app.paused {
            format!(" NEXT TASK in {}s · {}", app.pacing.remaining_seconds(), app.pacing.reason)
        } else { format!(" Sparse + old: {} · REVIEW = incomplete check · KEEP = protected · f changes rules", if app.policy.sparse_old_max_posts == 0 { "off".into() } else { format!("≤{} posts", app.policy.sparse_old_max_posts) }) }, fg(MUTED)));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_inventory(frame: &mut Frame, app: &App, area: Rect, accounts: &[&Account]) {
    let focused = app.focus.min(accounts.len().saturating_sub(1));
    let search = if app.mode == Mode::Search {
        format!(" / {}▏ ", clean(&app.query))
    } else if !app.query.is_empty() {
        format!(" / {} ", clean(&app.query))
    } else if app.only_matching {
        " REMOVAL CANDIDATES ".into()
    } else {
        " ALL FOLLOWERS ".into()
    };
    let block = panel(" FOLLOWER INVENTORY ")
        .title_top(
            Line::styled(
                search,
                fg(if app.mode == Mode::Search { MINT } else { ICE }),
            )
            .right_aligned(),
        )
        .title_bottom(
            Line::styled(
                format!(
                    " {} / {}  ·  ↑ ↓ navigate ",
                    if accounts.is_empty() { 0 } else { focused + 1 },
                    accounts.len()
                ),
                fg(MUTED),
            )
            .right_aligned(),
        );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if accounts.is_empty() {
        let (title, hint) = if !app.query.is_empty() || app.only_matching {
            (
                "No accounts in this view",
                "Use / to edit search or m to show all followers.",
            )
        } else if app.sender.is_none() {
            (
                "Your next chapter starts here",
                "Press Shift+P to pair Chrome, then s to scan.",
            )
        } else if !app.paused && app.scan.phase == "following" {
            (
                "Learning who you follow",
                "Followers appear in the next scan stage.",
            )
        } else if !app.paused && app.scan.phase == "followers" {
            (
                "Collecting your followers",
                "Results appear as Chrome finishes each page.",
            )
        } else {
            (
                "A little room for a fresh start",
                "Press s to collect followers, then i to check activity.",
            )
        };
        let height = if inner.height >= 5 { 5 } else { 2 };
        let mut lines = vec![Line::styled(title, bold(TEXT))];
        if height == 5 {
            lines.push(Line::from(""));
        }
        lines.push(Line::styled(hint, fg(MUTED)));
        frame.render_widget(
            Paragraph::new(lines).centered().wrap(Wrap { trim: false }),
            centered(inner, inner.width, height),
        );
        return;
    }
    let full = inner.width >= 88;
    let row_height = if inner.height >= 20 { 2 } else { 1 };
    let header_height = if inner.height >= 8 { 2 } else { 1 };
    let capacity = usize::from(inner.height.saturating_sub(header_height) / row_height).max(1);
    let offset = focused.saturating_sub(capacity - 1);
    let now = now_ms();
    let rows = accounts
        .iter()
        .skip(offset)
        .take(capacity)
        .enumerate()
        .map(|(i, a)| {
            let decision = a.reason(&app.policy, now);
            let active = app.active_target().filter(|(id, _)| *id == a.id);
            let check_first = app.selected.contains(&a.id)
                && a.basic_reason(&app.policy).is_ok()
                && decision.is_err();
            let color = if active.is_some() {
                ICE
            } else if a.kept {
                MUTED
            } else if decision.is_ok() {
                MINT
            } else {
                TEXT
            };
            let mark = if active.is_some() {
                "▶"
            } else if a.kept {
                "◆"
            } else if app.selected.contains(&a.id) {
                "✓"
            } else {
                "·"
            };
            let mut cells = vec![
                Cell::from(mark).style(fg(if active.is_some() {
                    ICE
                } else if app.selected.contains(&a.id) {
                    MINT
                } else {
                    MUTED
                })),
                Cell::from(format!("@{}", clean(&a.handle))).style(fg(color)),
                Cell::from(if active.is_some() {
                    "Working…".into()
                } else {
                    activity(a, now)
                })
                .style(fg(if active.is_some() { ICE } else { MUTED })),
            ];
            if full {
                cells.push(
                    Cell::from(format!(
                        "{} / {}",
                        count(a.followers),
                        count(a.following_count)
                    ))
                    .style(fg(MUTED)),
                );
            }
            cells.push(
                Cell::from(if let Some((_, action)) = active {
                    action.to_string()
                } else {
                    format!(
                        "{}: {}",
                        if check_first {
                            "CHECK FIRST"
                        } else if decision.is_ok() {
                            "REMOVE"
                        } else if a.basic_reason(&app.policy).is_ok()
                            && decision != Err("Recently active")
                        {
                            "REVIEW"
                        } else {
                            "KEEP"
                        },
                        decision.unwrap_or_else(|s| s)
                    )
                })
                .style(fg(if active.is_some() {
                    ICE
                } else if check_first {
                    AMBER
                } else if decision.is_ok() {
                    MINT
                } else {
                    MUTED
                })),
            );
            Row::new(cells).height(row_height).style(fg(TEXT).bg(
                if (offset + i).is_multiple_of(2) {
                    PANEL
                } else {
                    RAISED
                },
            ))
        });
    let mut columns = vec![
        Constraint::Length(2),
        Constraint::Percentage(if full { 27 } else { 36 }),
        Constraint::Length(12),
    ];
    let mut labels = vec!["", "ACCOUNT", "ACTIVITY"];
    if full {
        columns.push(Constraint::Length(17));
        labels.push("FANS / FOLLOWING");
    }
    columns.push(Constraint::Min(10));
    labels.push("DECISION");
    let table = Table::new(rows, columns)
        .header(Row::new(labels).style(fg(MUTED)).height(header_height))
        .column_spacing(1)
        .row_highlight_style(bold(TEXT).bg(FOCUS))
        .highlight_symbol("▎");
    let mut state = TableState::default().with_selected(Some(focused - offset));
    frame.render_stateful_widget(table, inner, &mut state);
    if accounts.len() > capacity {
        let mut scroll =
            ScrollbarState::new(accounts.len().saturating_sub(capacity)).position(offset);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .thumb_symbol("┃")
                .track_symbol(Some("│"))
                .thumb_style(fg(ICE))
                .track_style(fg(BORDER)),
            area.inner(Margin::new(0, 1)),
            &mut scroll,
        );
    }
}

fn render_evidence(frame: &mut Frame, app: &App, area: Rect, account: Option<&Account>) {
    let block = panel(" ACCOUNT EVIDENCE ");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(a) = account else {
        frame.render_widget(
            Paragraph::new("Focus an account to inspect its relationships and activity.")
                .style(fg(MUTED))
                .wrap(Wrap { trim: false }),
            inner,
        );
        return;
    };
    let decision = a.reason(&app.policy, now_ms());
    let [heading, facts, footer] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .areas(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(format!("@{}", clean(&a.handle)), bold(ICE)),
            Line::styled(clean(&a.name), fg(MUTED)),
            Line::styled(
                if let Some((_, action)) = app.active_target().filter(|(id, _)| *id == a.id) {
                    action
                } else if app.selected.contains(&a.id)
                    && a.basic_reason(&app.policy).is_ok()
                    && decision.is_err()
                {
                    "✓ SELECTED / CHECK FIRST"
                } else if decision.is_ok() {
                    "✓ REMOVAL CANDIDATE"
                } else if a.basic_reason(&app.policy).is_ok() && decision != Err("Recently active")
                {
                    "— REVIEW / INCOMPLETE CHECK"
                } else {
                    "— PROTECTED FROM REMOVAL"
                },
                bold(if decision.is_ok() { MINT } else { AMBER }),
            ),
            Line::styled(decision.unwrap_or_else(|s| s), fg(MUTED)),
        ]),
        heading,
    );
    let mut lines = vec![
        fact("Follows you", yes_no(a.follows_me)),
        fact("You follow", yes_no(a.i_follow)),
        fact("Verified", yes_no(a.verified)),
        fact("Protected", yes_no(a.protected)),
        fact("Followers", count(a.followers)),
        fact("Following", count(a.following_count)),
        fact("Current posts", count(a.posts)),
        fact("Observed post", activity(a, now_ms())),
        fact(
            "Coverage",
            if a.coverage_since_ms.is_some() {
                "Established"
            } else {
                "Incomplete"
            },
        ),
    ];
    if facts.height >= 12 {
        lines.push(Line::from(""));
        lines.push(Line::styled("EVIDENCE", fg(MUTED)));
        lines.push(Line::styled(clean(&a.activity_note), fg(TEXT)));
        if let Some(posts) = a.posts {
            lines.push(Line::styled("POST VOLUME / COLLECTED FOLLOWERS", fg(MUTED)));
            lines.push(Line::styled(
                post_volume(app.accounts.values(), posts),
                fg(ICE),
            ));
        }
        let digits = a.handle.chars().filter(|c| c.is_ascii_digit()).count();
        if digits >= 5 {
            lines.push(Line::styled(
                format!("Handle: {digits} digits (weak signal)"),
                fg(MUTED),
            ));
        }
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), facts);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                if a.kept {
                    "◆ Kept · excluded from removal"
                } else if app.selected.contains(&a.id) {
                    "✓ Selected for review"
                } else {
                    "Not selected"
                },
                fg(if a.kept { AMBER } else { MINT }),
            ),
            theme::keys(&[("Enter", "details"), ("o", "profile")]),
        ]),
        footer,
    );
}

fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let notice_color = if app.uncertain > 0 {
        RED
    } else if app.paused || app.sender.is_none() {
        AMBER
    } else {
        MUTED
    };
    let (notice, controls) = if area.height >= 4 {
        (
            Rect::new(area.x, area.y, area.width, 2),
            Rect::new(area.x, area.y + 2, area.width, 2),
        )
    } else {
        (
            Rect::new(area.x, area.y, area.width, 1),
            Rect::new(area.x, area.y + 1, area.width, 2),
        )
    };
    frame.render_widget(
        Paragraph::new(Line::styled(
            format!(" {}", clean(&app.notice)),
            fg(notice_color),
        ))
        .wrap(Wrap { trim: false }),
        notice,
    );
    let lines = if app.batch.is_some() && app.mode == Mode::Browse {
        vec![
            theme::keys(&[
                ("p", "pause / resume"),
                ("c", "cancel queue"),
                ("b", "background"),
                ("q", "pause & quit"),
            ]),
            theme::keys(&[
                ("v", "list / animation"),
                ("Enter", "details"),
                ("?", "help"),
            ]),
        ]
    } else if app.mode == Mode::Search {
        vec![
            theme::keys(&[("Type", "search accounts"), ("Enter / Esc", "finish")]),
            theme::keys(&[("p", "pause"), ("c", "cancel queue")]),
        ]
    } else if area.width < 90 {
        vec![
            theme::keys(&[
                ("s", "collect"),
                ("i", "activity"),
                ("A", "basic select"),
                ("d", "review"),
            ]),
            theme::keys(&[
                ("f", "filters"),
                ("p", "pause"),
                ("?", "keys"),
                ("q", "quit"),
            ]),
        ]
    } else {
        vec![
            theme::keys(&[
                ("s", "collect"),
                ("i", "activity"),
                ("f", "rules"),
                ("Space", "select"),
                ("a", "checked"),
                ("A", "basic rules"),
                ("d", "review removal"),
            ]),
            theme::keys(&[
                ("K", "keep"),
                ("m", "candidates / all"),
                ("p", "pause"),
                ("c", "cancel"),
                ("r", "reconcile"),
                ("P", "pair"),
                ("?", "help"),
                ("q", "quit"),
            ]),
        ]
    };
    frame.render_widget(Paragraph::new(lines), controls);
}

fn render_modal(frame: &mut Frame, app: &mut App) {
    match app.mode {
        Mode::Help => {
            app.modal_scroll = popup(
                frame,
                " KEYBOARD / MOUSE WHEEL ",
                HELP.iter()
                    .map(|(k, v)| {
                        Line::from(vec![
                            Span::styled(format!("{k:25}"), fg(ICE)),
                            Span::styled(*v, fg(TEXT)),
                        ])
                    })
                    .collect(),
                (84, 27),
                "↑ ↓ scroll  ·  Esc / Enter close",
                app.modal_scroll,
                ICE,
            );
        }
        Mode::Filters => {
            let labels = [
                (
                    "REMOVE: inactive at least",
                    format!("{} days", app.policy.inactive_days),
                ),
                (
                    "PROTECT: verified accounts",
                    on(app.policy.skip_verified).into(),
                ),
                (
                    "PROTECT: people you follow",
                    on(app.policy.skip_following).into(),
                ),
                (
                    "REMOVE: zero posts too",
                    on(app.policy.include_zero_posts).into(),
                ),
                (
                    "Minimum removal interval",
                    format!("{} seconds", app.policy.delay_seconds),
                ),
                (
                    "Hourly attempt budget",
                    format!("{} / hour", app.policy.batch_limit),
                ),
                (
                    "REMOVE: sparse + old",
                    if app.policy.sparse_old_max_posts == 0 {
                        "off".into()
                    } else {
                        format!("≤{} posts", app.policy.sparse_old_max_posts)
                    },
                ),
            ];
            let mut lines: Vec<Line> = labels
                .into_iter()
                .enumerate()
                .map(|(i, (label, value))| {
                    Line::styled(
                        format!(
                            "{} {label:26} {value}",
                            if i == app.filter_row { "›" } else { " " }
                        ),
                        if i == app.filter_row {
                            bold(MINT).bg(FOCUS)
                        } else {
                            fg(TEXT)
                        },
                    )
                })
                .collect();
            lines.push(Line::from(""));
            lines.push(Line::styled(
                "Protections always apply. Removal rules are alternatives.",
                fg(MUTED),
            ));
            lines.push(Line::styled(
                "Sparse + old accepts incomplete coverage, but needs an old observed post.",
                fg(MUTED),
            ));
            lines.push(Line::styled(
                "a selects checked candidates. Shift+A selects basic matches.",
                fg(ICE),
            ));
            popup(
                frame,
                " REMOVAL RULES / WHO CAN BE REMOVED ",
                lines,
                (82, 18),
                "↑ ↓ choose · ← → adjust · Enter saves",
                0,
                MINT,
            );
        }
        Mode::Details => {
            let rows = app.visible();
            if let Some(a) = rows.get(app.focus.min(rows.len().saturating_sub(1))) {
                let decision = a.reason(&app.policy, now_ms());
                app.modal_scroll = popup(
                    frame,
                    " ACCOUNT EVIDENCE ",
                    vec![
                        Line::styled(
                            format!("@{} / {}", clean(&a.handle), clean(&a.name)),
                            bold(ICE),
                        ),
                        Line::styled(format!("ID {}", a.id), fg(MUTED)),
                        Line::from(""),
                        fact("Follows you", yes_no(a.follows_me)),
                        fact("You follow", yes_no(a.i_follow)),
                        fact("Verified", yes_no(a.verified)),
                        fact("Protected", yes_no(a.protected)),
                        fact("Followers", count(a.followers)),
                        fact("Following", count(a.following_count)),
                        fact("Current posts", count(a.posts)),
                        fact("Observed post", activity(a, now_ms())),
                        fact(
                            "Coverage",
                            if a.coverage_since_ms.is_some() {
                                "Established"
                            } else {
                                "Incomplete"
                            },
                        ),
                        Line::from(""),
                        Line::styled(
                            decision.unwrap_or_else(|s| s),
                            bold(if decision.is_ok() { MINT } else { AMBER }),
                        ),
                        Line::styled(clean(&a.activity_note), fg(MUTED)),
                        Line::styled(
                            a.posts
                                .map(|posts| post_volume(app.accounts.values(), posts))
                                .unwrap_or_else(|| "Post count unknown".into()),
                            fg(ICE),
                        ),
                        Line::from(""),
                        Line::styled(clean(&a.bio), fg(TEXT)),
                    ],
                    (76, 25),
                    "↑ ↓ scroll · Esc / Enter close",
                    app.modal_scroll,
                    ICE,
                );
            }
        }
        Mode::Confirm => {
            // Warnings and default-cancel controls stay visible even at 52 × 12.
            let compact = frame.area().height < 20;
            let mut lines = vec![
                Line::styled(
                    format!(
                        "Remove {} cleared followers from @{}?",
                        app.confirmation.len(),
                        clean(&app.handle)
                    ),
                    bold(TEXT),
                ),
                Line::styled("Uses saved activity results. No activity rescan.", fg(TEXT)),
                Line::styled("There is no restore-followers action.", bold(RED)),
                Line::styled(
                    format!(
                        "One at a time · at least {}s · automatic cooldowns",
                        app.policy.delay_seconds
                    ),
                    fg(MUTED),
                ),
            ];
            if app.policy.sparse_old_max_posts > 0 {
                lines[3] = Line::styled(
                    format!(
                        "Sparse + old: ≤{} posts; coverage may be incomplete.",
                        app.policy.sparse_old_max_posts
                    ),
                    fg(AMBER),
                );
            }
            if !compact {
                lines.push(Line::styled(
                    "Identity and relationship protections are still checked.",
                    fg(ICE),
                ));
                lines.push(Line::styled(
                    "b sends the approved queue to background; p pauses.",
                    fg(ICE),
                ));
                lines.extend(
                    app.confirmation
                        .iter()
                        .take(4)
                        .filter_map(|id| app.accounts.get(id))
                        .map(|a| Line::styled(format!("  @{}", clean(&a.handle)), fg(ICE))),
                );
                if app.confirmation.len() > 4 {
                    lines.push(Line::styled(
                        format!("  + {} more selected", app.confirmation.len() - 4),
                        fg(MUTED),
                    ));
                }
            }
            popup(
                frame,
                " APPROVE REMOVAL QUEUE ",
                lines,
                (68, if compact { 10 } else { 17 }),
                "Enter / n / Esc cancels  ·  y confirms",
                0,
                RED,
            );
        }
        _ => {}
    }
}

fn popup(
    frame: &mut Frame,
    title: &str,
    lines: Vec<Line<'static>>,
    size: (u16, u16),
    hint: &str,
    scroll: u16,
    color: ratatui::style::Color,
) -> u16 {
    // Dim the dashboard without clearing it; the modal remains the only active surface.
    let area = frame.area();
    frame.buffer_mut().set_style(area, fg(BORDER).bg(BG));
    let r = centered(area.inner(Margin::new(1, 1)), size.0, size.1);
    let block = panel(title.to_string()).border_style(fg(color));
    let inner = block.inner(r);
    frame.render_widget(Clear, r);
    frame.render_widget(block, r);
    let [body, footer] = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(inner);
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    let max_scroll = paragraph
        .line_count(body.width)
        .saturating_sub(usize::from(body.height))
        .min(u16::MAX as usize) as u16;
    let scroll = scroll.min(max_scroll);
    frame.render_widget(paragraph.scroll((scroll, 0)), body);
    frame.render_widget(
        Paragraph::new(hint)
            .style(fg(color))
            .wrap(Wrap { trim: false }),
        footer,
    );
    scroll
}

fn fact(label: &str, value: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:15}"), fg(MUTED)),
        Span::styled(value.into(), fg(TEXT)),
    ])
}
fn yes_no(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "Yes",
        Some(false) => "No",
        None => "Unknown",
    }
}
fn activity(a: &Account, now: i64) -> String {
    if a.checked_at_ms.is_none() {
        "Not checked".into()
    } else if a.posts == Some(0) {
        "Zero posts".into()
    } else if let Some(t) = a.last_activity_ms {
        format!("Seen {}d", ((now - t) / 86_400_000).max(0))
    } else {
        "Unknown".into()
    }
}
fn count(n: Option<u64>) -> String {
    n.map(number).unwrap_or_else(|| "?".into())
}
fn on(b: bool) -> &'static str {
    if b { "on" } else { "off" }
}
fn number(n: u64) -> String {
    let s = n.to_string();
    s.chars()
        .enumerate()
        .fold(String::new(), |mut out, (i, c)| {
            if i > 0 && (s.len() - i).is_multiple_of(3) {
                out.push(',');
            }
            out.push(c);
            out
        })
}
