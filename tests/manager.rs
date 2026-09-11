use serde_json::{Value, json};
use std::{collections::VecDeque, path::Path};
use tokio::sync::mpsc;
use x_bot_follower_remover::{
    app::{App, Batch, Mode},
    manager::{Action, Query},
    model::{Account, Policy, now_ms},
    protocol::{Command, Work},
    store::Store,
};
fn fixture() -> (App, mpsc::Receiver<Value>) {
    let mut a = App::new(Store::open(Path::new(":memory:")).unwrap(), true).unwrap();
    a.owner = "1".into();
    a.handle = "example".into();
    a.session = "session".into();
    a.policy = Policy::cleanup();
    a.mode = Mode::Browse;
    let (tx, rx) = mpsc::channel(64);
    a.sender = Some(tx);
    (a, rx)
}
fn follower(id: &str) -> Account {
    Account {
        id: id.into(),
        handle: format!("account_{id}"),
        name: format!("Name {id}"),
        follows_me: Some(true),
        i_follow: Some(false),
        verified: Some(false),
        protected: Some(false),
        posts: Some(5),
        last_activity_ms: Some(now_ms() - 60 * 86_400_000),
        coverage_since_ms: Some(now_ms() - 31 * 86_400_000),
        checked_at_ms: Some(now_ms()),
        ..Default::default()
    }
}
async fn request(
    a: &mut App,
    rx: &mut mpsc::Receiver<Value>,
    owner: &str,
    action: Action,
) -> Value {
    a.manager_request("request".into(), owner.into(), action, true)
        .await
        .unwrap();
    loop {
        let v = rx.recv().await.unwrap();
        if v["type"] == "manager_result" {
            return v;
        }
    }
}
#[tokio::test]
async fn decisions_separate_unknown_candidates_queued_and_confirmed_receipts() {
    let (mut app, _rx) = fixture();
    for id in ["2", "3", "4", "5", "6"] {
        app.accounts.insert(id.into(), follower(id));
    }
    app.accounts.get_mut("3").unwrap().verified = Some(true);
    app.accounts.get_mut("4").unwrap().coverage_since_ms = None;
    app.accounts.get_mut("4").unwrap().last_activity_ms = None;
    app.accounts.get_mut("5").unwrap().i_follow = Some(true);
    app.accounts.get_mut("6").unwrap().follows_me = Some(false);
    let work = Work {
        command_id: "receipt".into(),
        owner_id: "1".into(),
        command: Command::RemoveFollower {
            target_id: "6".into(),
            batch_id: "batch".into(),
            policy: app.policy.clone(),
            deadline_ms: now_ms() + 30000,
            approved_account: None,
        },
    };
    app.store
        .run(move |s| {
            s.start_action(&work)?;
            s.finish_action("receipt", "verified_removed", "confirmed")
        })
        .await
        .unwrap();
    let snapshot = app.manager_snapshot(Query::default()).await.unwrap();
    assert_eq!(
        snapshot["counts"],
        json!({"all":5,"keep":2,"remove":1,"review":1,"removed":1,"queue":0})
    );
    app.accounts.get_mut("6").unwrap().follows_me = None;
    assert_eq!(
        app.manager_snapshot(Query::default()).await.unwrap()["counts"]["removed"],
        1
    );
    assert!(app.pending.is_none());
    assert!(app.auto_policy.is_none());
    app.batch = Some(Batch {
        id: "q".into(),
        ids: VecDeque::from(["2".into()]),
        policy: app.policy.clone(),
    });
    let snapshot = app
        .manager_snapshot(Query {
            filter: "queue".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(snapshot["total"], 1);
    assert_eq!(snapshot["rows"][0]["account"]["id"], "2");
}
#[tokio::test]
async fn keep_is_durable_and_prunes_queue_but_cannot_claim_to_stop_dispatched_removal() {
    let (mut app, mut rx) = fixture();
    let a = follower("2");
    let copy = a.clone();
    app.store
        .run(move |s| s.save_page("1", &[copy], "fixture", &true))
        .await
        .unwrap();
    app.accounts.insert("2".into(), a);
    app.batch = Some(Batch {
        id: "q".into(),
        ids: VecDeque::from(["2".into()]),
        policy: app.policy.clone(),
    });
    assert_eq!(
        request(
            &mut app,
            &mut rx,
            "1",
            Action::Keep {
                target_id: "2".into(),
                kept: true
            }
        )
        .await["ok"],
        true
    );
    assert!(app.batch.as_ref().unwrap().ids.is_empty());
    assert!(
        app.store
            .run(|s| Ok(s.accounts("1")?[0].kept))
            .await
            .unwrap()
    );
    app.pending = Some(Work {
        command_id: "pending".into(),
        owner_id: "1".into(),
        command: Command::RemoveFollower {
            target_id: "2".into(),
            batch_id: "q".into(),
            policy: app.policy.clone(),
            deadline_ms: now_ms() + 30000,
            approved_account: None,
        },
    });
    assert_eq!(
        request(
            &mut app,
            &mut rx,
            "1",
            Action::Keep {
                target_id: "2".into(),
                kept: false
            }
        )
        .await["ok"],
        false
    );
    assert!(app.accounts["2"].kept);
}
#[tokio::test]
async fn controls_require_live_owner_and_explicit_cleanup_confirmation() {
    let (mut app, mut rx) = fixture();
    assert_eq!(
        request(
            &mut app,
            &mut rx,
            "wrong",
            Action::Start { confirmed: true }
        )
        .await["ok"],
        false
    );
    assert_eq!(
        request(&mut app, &mut rx, "1", Action::Start { confirmed: false }).await["ok"],
        false
    );
    assert!(app.auto_policy.is_none());
    assert_eq!(
        request(&mut app, &mut rx, "1", Action::Collect).await["ok"],
        true
    );
    assert!(app.auto_policy.is_none());
    assert!(app.batch.is_none());
    assert_eq!(app.scan.phase, "following");
    app.scan.phase = "review".into();
    app.mode = Mode::Setup;
    app.scan.following_complete = true;
    assert_eq!(
        request(&mut app, &mut rx, "1", Action::Inspect).await["ok"],
        true
    );
    assert_eq!(app.scan.phase, "inspect");
    assert_eq!(app.mode, Mode::Browse);
    assert!(app.auto_policy.is_none());
    app.handle.clear();
    assert_eq!(
        request(&mut app, &mut rx, "1", Action::Resume).await["ok"],
        false
    );
}
#[tokio::test]
async fn search_and_pagination_are_bounded_and_mutation_cannot_relax_rules() {
    let (mut app, mut rx) = fixture();
    for id in 10..140 {
        let a = follower(&id.to_string());
        app.accounts.insert(a.id.clone(), a);
    }
    let s = app
        .manager_snapshot(Query {
            page: 1,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(s["rows"].as_array().unwrap().len(), 50);
    assert_eq!(s["total"], 130);
    let s = app
        .manager_snapshot(Query {
            search: "NAME 139".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(s["total"], 1);
    let mut policy = app.policy.clone();
    policy.skip_verified = false;
    policy.inactive_days = 1;
    policy.delay_seconds = 15;
    assert_eq!(
        request(&mut app, &mut rx, "1", Action::Settings { policy }).await["ok"],
        true
    );
    assert_eq!(app.policy.inactive_days, 30);
    assert!(app.policy.skip_verified);
    assert_eq!(app.policy.delay_seconds, 15);
}
#[tokio::test]
async fn advanced_switch_restores_inventory_without_changing_the_cleanup_rule() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (mut app, _rx) = fixture();
    app.advanced = false;
    app.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))
        .await
        .unwrap();
    assert!(app.advanced);
    assert_eq!(app.policy, Policy::cleanup());
    let backend = ratatui::backend::TestBackend::new(120, 34);
    let mut term = ratatui::Terminal::new(backend).unwrap();
    term.draw(|f| x_bot_follower_remover::ui::render(f, &mut app, 0))
        .unwrap();
    let text: String = term
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(text.contains("FOLLOWER INVENTORY"));
}

#[tokio::test]
async fn reload_from_completed_inventory_restarts_collection_and_advanced_rules_are_valid() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (mut app, mut rx) = fixture();
    app.scan.adapter_revision = 2;
    app.scan.phase = "review".into();
    assert_eq!(
        request(&mut app, &mut rx, "1", Action::Collect).await["ok"],
        true
    );
    assert_eq!(app.scan.phase, "following");
    app.advanced = true;
    app.mode = Mode::Browse;
    app.key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE))
        .await
        .unwrap();
    app.key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))
        .await
        .unwrap();
    assert!(!app.policy.simple_cleanup);
    app.mode = Mode::Browse;
    app.key(KeyEvent::new(KeyCode::Char('F'), KeyModifiers::NONE))
        .await
        .unwrap();
    assert!(app.policy.simple_cleanup);
    assert_eq!(app.policy.inactive_days, 30);
    assert!(app.policy.skip_verified && app.policy.skip_following);
}
#[test]
fn worker_animation_uses_confirmed_tags_and_freezes_on_pause() {
    use x_bot_follower_remover::{background::Status, ritual::Animation, ui};
    let mut status:Status=serde_json::from_value(json!({"handle":"example","state":"Cooling down","remaining":2,"removed":7,"uncertain":0,"wait_seconds":60,"message":"rest","has_job":true,"paused":false})).unwrap();
    status
        .rows
        .push(("list_only_profile".into(), "Never".into(), "Waiting".into()));
    let mut animation = Animation::default();
    animation.removed("confirmed_account".into());
    let draw = |s: &Status, a: &Animation| {
        let mut t = ratatui::Terminal::new(ratatui::backend::TestBackend::new(140, 42)).unwrap();
        t.draw(|f| ui::render_worker_with_animation(f, s, None, false, false, Some(a)))
            .unwrap();
        t.backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>()
    };
    let top = draw(&status, &animation);
    animation.advance(std::time::Duration::from_millis(1000));
    let down = draw(&status, &animation);
    assert!(top.contains("THE CLEANSE"));
    assert!(!top.contains("list_only_profile"));
    assert!(top.contains("Reseting followers: 7"));
    assert!(down.find("@confirmed_account").unwrap() > top.find("@confirmed_account").unwrap());
    status.paused = true;
    assert!(draw(&status, &animation).contains("Paused · water stopped"));
}

#[test]
fn pouring_scene_survives_real_terminal_sizes_and_tags_keep_moving() {
    use x_bot_follower_remover::{background::Status, ritual::Animation, ui};
    let status: Status = serde_json::from_value(json!({"handle":"example","state":"Cooling down","remaining":1,"removed":51,"uncertain":0,"wait_seconds":2,"message":"Confirmed","has_job":true,"paused":false})).unwrap();
    for (width, height) in [(120, 30), (100, 28), (80, 26), (140, 42)] {
        let draw = |animation: &Animation| {
            let mut t =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
            t.draw(|f| {
                ui::render_worker_with_animation(f, &status, None, false, false, Some(animation))
            })
            .unwrap();
            t.backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>()
        };
        let mut animation = Animation::default();
        let first = draw(&animation);
        if width >= 112 {
            assert!(first.contains("BRAIN / VISUALIZATION"));
            assert!(first.contains("FLY 01"));
            assert!(
                first
                    .chars()
                    .any(|c| ('\u{2801}'..='\u{28ff}').contains(&c))
            );
        }
        assert!(
            first.contains("━━━━━━━━"),
            "ladle missing at {width}x{height}"
        );
        assert!(
            first.matches("███").count() >= 5,
            "X logo missing at {width}x{height}"
        );
        animation.advance(std::time::Duration::from_millis(150));
        assert_ne!(first, draw(&animation), "water must flow during cooldown");
        animation.removed("departed".into());
        let top = draw(&animation);
        animation.advance(std::time::Duration::from_secs(1));
        animation.advance(std::time::Duration::from_secs(1));
        let bottom = draw(&animation);
        assert!(bottom.find("@departed").unwrap() > top.find("@departed").unwrap());
    }
}
