//! Render deterministic, fictional UI fixtures without a terminal or X connection.
//! cargo run --example preview > /tmp/forgive-me-preview.json
use forgive_me::{
    app::{App, Batch, Mode},
    config::Config,
    model::{Account, now_ms},
    protocol::{Command, Work},
    setup::Step,
    store::Store,
    ui,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color, Modifier},
};
use serde_json::{Value, json};
use std::{collections::VecDeque, path::Path};

fn color(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => panic!("Preview must use the explicit theme palette: {c:?}"),
    }
}
fn capture(app: &mut App, name: &str, w: u16, h: u16) -> Value {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| ui::render(f, app, 8)).unwrap();
    let b = terminal.backend().buffer();
    json!({ "name": name, "width": w, "height": h, "cells": b.content.iter().map(|c| json!({
        "text": c.symbol(), "fg": color(c.fg), "bg": color(c.bg), "bold": c.modifier.contains(Modifier::BOLD),
    })).collect::<Vec<_>>() })
}
fn main() {
    let mut app = App::new(Store::open(Path::new(":memory:")).unwrap(), true).unwrap();
    let (tx, _rx) = tokio::sync::mpsc::channel(8);
    app.sender = Some(tx);
    app.handle = "demo_account".into();
    app.owner = "demo".into();
    app.scan.phase = "done".into();
    app.notice = "Activity checked. a selects removal candidates; d reviews the full queue.".into();
    let now = now_ms();
    for (i, name) in [
        "afterglow",
        "atlas_studio",
        "blue_example",
        "copper_echo",
        "distant_signal",
        "empty_orbit",
        "field_notes",
        "grey_morning",
        "hidden_summer",
        "idle_satellite",
        "juniper",
        "kindred",
        "low_tide",
        "mutual_friend",
        "night_window",
        "paper_moon",
        "quiet_reader",
        "river_delta",
        "soft_static",
        "tide_pool",
        "unknown_activity",
        "violet_hour",
        "willow",
        "winter",
        "woodland",
        "yellow",
        "yesterday",
        "zenith",
        "zinc",
        "zodiac",
    ]
    .iter()
    .enumerate()
    {
        let days = if i == 0 {
            657
        } else if i % 4 == 0 {
            2
        } else {
            240
        };
        let a = Account {
            id: i.to_string(),
            handle: name.to_string(),
            name: name.replace('_', " "),
            bio: "Fictional preview account. No real X account is represented.".into(),
            followers: Some(12 + i as u64 * 87),
            following_count: Some(1400 + i as u64),
            posts: Some(if i == 0 {
                2
            } else if i == 5 {
                0
            } else {
                42
            }),
            verified: Some(i == 2),
            protected: Some(false),
            follows_me: Some(true),
            i_follow: Some(i == 13),
            last_activity_ms: if i == 20 {
                None
            } else {
                Some(now - days * 86_400_000)
            },
            coverage_since_ms: if i == 0 || i == 20 {
                None
            } else {
                Some(now - 365 * 86_400_000)
            },
            checked_at_ms: Some(now),
            observed_at_ms: now,
            activity_note: if i == 0 {
                "Replies coverage incomplete. Old observed post; sparse rule qualifies."
            } else if i == 20 {
                "Activity could not be established."
            } else {
                "Latest post, reply and repost checked. Simulated evidence."
            }
            .into(),
            kept: i == 11,
        };
        app.accounts.insert(a.id.clone(), a);
    }
    app.selected.extend(["3", "5", "9"].map(String::from));
    app.focus = 5;
    let mut scenes = vec![
        capture(&mut app, "dashboard", 140, 42),
        capture(&mut app, "compact", 80, 24),
        capture(&mut app, "minimum", 52, 12),
    ];
    app.mode = Mode::Confirm;
    app.confirmation = app.selected.iter().cloned().collect();
    scenes.push(capture(&mut app, "review", 120, 34));
    scenes.push(capture(&mut app, "review-minimum", 52, 12));
    app.focus = 0;
    app.mode = Mode::Browse;
    scenes.push(capture(&mut app, "sparse-review", 140, 42));
    app.focus = 5;
    app.mode = Mode::Filters;
    scenes.push(capture(&mut app, "policy", 120, 34));
    app.mode = Mode::Browse;
    app.batch = Some(Batch {
        id: "preview".into(),
        ids: VecDeque::from_iter(app.selected.iter().cloned()),
        policy: app.policy.clone(),
    });
    app.paused = false;
    app.removed = 27;
    app.notice = "Removal batch approved. p pauses; c cancels remaining work.".into();
    app.pending = Some(Work {
        command_id: "preview-work".into(),
        owner_id: app.owner.clone(),
        command: Command::RemoveFollower {
            target_id: "5".into(),
            batch_id: "preview".into(),
            policy: app.policy.clone(),
            approved_account: None,
            deadline_ms: now + 120_000,
        },
    });
    scenes.push(capture(&mut app, "queue-list", 140, 42));
    app.show_queue_list = false;
    scenes.push(capture(&mut app, "cleanse", 140, 42));
    app.configure_setup(&Config::default());
    app.mode = Mode::Setup;
    app.setup.as_mut().unwrap().go(Step::Pair);
    app.sender = None;
    app.handle.clear();
    app.notice = "Pair with your Chrome extension to continue.".into();
    scenes.push(capture(&mut app, "pairing", 120, 34));
    scenes.push(capture(&mut app, "pairing-minimum", 52, 12));
    let (tx, _rx) = tokio::sync::mpsc::channel(8);
    app.sender = Some(tx);
    app.setup.as_mut().unwrap().go(Step::Connect);
    app.setup.as_mut().unwrap().account_error =
        Some("signing_unavailable: X request signing could not be prepared.".into());
    app.notice = "Chrome is paired. Retry the X check after refreshing discovery.".into();
    scenes.push(capture(&mut app, "account-error", 120, 34));
    println!("{}", serde_json::to_string(&scenes).unwrap());
}
