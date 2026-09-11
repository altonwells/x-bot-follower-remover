use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use forgive_me::{
    app::{App, Batch, Mode},
    bridge::BridgeEvent,
    model::{Account, Policy, now_ms},
    protocol::{Command, Work},
    store::Store,
};
use std::{collections::VecDeque, path::Path};
use tokio::sync::mpsc;
fn fixture() -> App {
    let mut store = Store::open(Path::new(":memory:")).unwrap();
    store.set("last_owner", &"1").unwrap();
    let mut a: Account = serde_json::from_value(
        serde_json::from_str::<serde_json::Value>(include_str!("../protocol/fixtures/policy.json"))
            .unwrap()["account"]
            .clone(),
    )
    .unwrap();
    a.id = "2".into();
    a.checked_at_ms = Some(now_ms());
    a.posts = Some(0);
    store
        .save_page("1", &[a], "scan:1", &serde_json::json!({}))
        .unwrap();
    // Avoid deserializing an incomplete scan fixture.
    store
        .set("scan:1", &forgive_me::app::Scan::default())
        .unwrap();
    let mut app = App::new(store, false).unwrap();
    app.capabilities = vec!["remove_follower".into(), "adapter:2".into()];
    app
}
fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}
#[tokio::test]
async fn confirmation_defaults_to_cancel_and_disconnect_invalidates_it() {
    let mut app = fixture();
    app.selected.insert("2".into());
    app.key(key('d')).await.unwrap();
    assert_eq!(app.mode, Mode::Confirm);
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await
        .unwrap();
    assert!(app.batch.is_none());
    app.key(key('d')).await.unwrap();
    app.bridge_event(BridgeEvent::Disconnected("closed".into()))
        .await
        .unwrap();
    assert_eq!(app.mode, Mode::Browse);
    assert!(app.confirmation.is_empty());
    app.key(key('y')).await.unwrap();
    assert!(app.batch.is_none());
}
#[tokio::test]
async fn reconnect_invalidates_old_owner_confirmation() {
    let mut app = fixture();
    app.mode = Mode::Confirm;
    app.confirmation = vec!["2".into()];
    let (tx, mut rx) = mpsc::channel(8);
    app.bridge_event(BridgeEvent::Connected {
        session_id: "new".into(),
        sender: tx,
    })
    .await
    .unwrap();
    assert_eq!(app.mode, Mode::Browse);
    assert!(app.confirmation.is_empty());
    assert!(app.paused);
    assert_eq!(
        rx.recv().await.unwrap()["work"]["command"]["kind"],
        "get_session"
    );
}
#[tokio::test]
async fn modal_stop_controls_remain_available() {
    let mut app = fixture();
    for mode in [
        Mode::Search,
        Mode::Details,
        Mode::Filters,
        Mode::Help,
        Mode::Confirm,
    ] {
        app.mode = mode;
        app.paused = false;
        app.key(key('p')).await.unwrap();
        assert!(app.paused);
    }
    app.batch = Some(Batch {
        id: "b".into(),
        ids: VecDeque::from(["2".into()]),
        policy: Policy::default(),
    });
    app.mode = Mode::Details;
    app.key(key('c')).await.unwrap();
    assert!(app.batch.is_none());
}
#[tokio::test]
async fn finalized_attempt_is_not_replayed_after_queue_checkpoint_loss() {
    let mut app = fixture();
    let policy = Policy::default();
    let work = Work {
        command_id: "a".repeat(64),
        owner_id: "1".into(),
        command: Command::RemoveFollower {
            target_id: "2".into(),
            batch_id: "b".into(),
            policy: policy.clone(),
            deadline_ms: now_ms() + 120_000,
        },
    };
    app.store
        .run(move |s| {
            s.start_action(&work)?;
            s.finish_action(&work.command_id, "verified_removed", "done")?;
            s.finish_action(&work.command_id, "uncertain", "stale recovery")?;
            assert!(s.unresolved("1")?.is_empty());
            Ok(())
        })
        .await
        .unwrap();
    app.batch = Some(Batch {
        id: "b".into(),
        ids: VecDeque::from(["2".into()]),
        policy,
    });
    app.paused = false;
    let (tx, mut rx) = mpsc::channel(8);
    app.sender = Some(tx);
    app.tick().await.unwrap();
    assert!(app.batch.is_none());
    assert!(app.pending.is_none());
    assert!(rx.try_recv().is_err());
}
#[test]
fn focused_target_matches_clamped_highlight() {
    let mut app = fixture();
    app.focus = 100;
    assert_eq!(app.focused().as_deref(), Some("2"));
}
#[tokio::test]
async fn old_adapter_scans_refresh_before_resuming_and_keep_exceptions() {
    let mut app = fixture();
    app.scan.phase = "followers".into();
    app.scan.following_complete = true;
    app.accounts.get_mut("2").unwrap().kept = true;
    let (tx, mut rx) = mpsc::channel(8);
    app.sender = Some(tx);
    app.paused = false;
    app.tick().await.unwrap();
    assert!(app.paused);
    assert!(rx.try_recv().is_err());
    app.key(key('s')).await.unwrap();
    assert_eq!(app.scan.adapter_revision, 2);
    assert_eq!(app.scan.phase, "following");
    assert!(app.accounts["2"].kept);
    assert_eq!(app.accounts["2"].follows_me, None);
    assert_eq!(app.accounts["2"].i_follow, None);
    assert_eq!(rx.recv().await.unwrap()["type"], "resume");
}
#[test]
fn unknown_verification_is_inspected_but_never_selected() {
    let mut app = fixture();
    let account = app.accounts.get_mut("2").unwrap();
    account.verified = None;
    assert!(account.basic_candidate(&app.policy));
    assert_eq!(
        account.reason(&app.policy, now_ms()),
        Err("Verification not checked")
    );
}
#[tokio::test]
async fn an_outdated_browser_cannot_resume_saved_work_or_start_a_scan() {
    let mut app = fixture();
    app.capabilities.clear();
    app.scan.adapter_revision = 2;
    app.scan.phase = "followers".into();
    app.paused = false;
    let (tx, mut rx) = mpsc::channel(8);
    app.sender = Some(tx);
    app.tick().await.unwrap();
    assert!(app.paused);
    assert!(app.key(key('s')).await.is_err());
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn collection_stops_before_activity_and_i_explicitly_starts_checks() {
    use forgive_me::protocol::{ClientMessage, WorkResult};
    let mut app = fixture();
    app.scan.phase = "followers".into();
    let work = Work {
        command_id: "read".into(),
        owner_id: "1".into(),
        command: Command::ScanPage {
            list: "followers".into(),
            cursor: None,
        },
    };
    app.pending = Some(work.clone());
    app.paused = false;
    app.bridge_event(BridgeEvent::Message(ClientMessage::Result {
        session_id: "s".into(),
        command_id: work.command_id,
        result: WorkResult::Page {
            list: "followers".into(),
            accounts: vec![],
            next_cursor: None,
            complete: true,
        },
    }))
    .await
    .unwrap();
    assert_eq!(app.scan.phase, "review");
    assert!(app.paused);
    app.key(key('i')).await.unwrap();
    assert_eq!(app.scan.phase, "inspect");
    assert!(!app.paused);
}

#[tokio::test]
async fn approval_keeps_every_selected_target_instead_of_truncating_to_hourly_budget() {
    let mut app = fixture();
    let mut other = app.accounts["2"].clone();
    other.id = "3".into();
    app.accounts.insert("3".into(), other);
    app.policy.batch_limit = 1;
    app.selected.extend(["2".into(), "3".into()]);
    app.key(key('d')).await.unwrap();
    assert_eq!(app.confirmation.len(), 2);
    app.key(key('y')).await.unwrap();
    assert_eq!(app.batch.as_ref().unwrap().ids.len(), 2);
}

#[tokio::test]
async fn deferred_removal_stays_queued_cooldown_survives_reload_and_is_not_unresolved() {
    use forgive_me::protocol::{ClientMessage, WorkResult};
    let mut app = fixture();
    app.batch = Some(Batch {
        id: "b".into(),
        ids: VecDeque::from(["2".into()]),
        policy: app.policy.clone(),
    });
    let work = Work {
        command_id: "d".repeat(64),
        owner_id: "1".into(),
        command: Command::RemoveFollower {
            target_id: "2".into(),
            batch_id: "b".into(),
            policy: app.policy.clone(),
            deadline_ms: now_ms() + 120_000,
        },
    };
    let copy = work.clone();
    app.store.run(move |s| s.start_action(&copy)).await.unwrap();
    app.pending = Some(work.clone());
    app.paused = false;
    let until = now_ms() + 300_000;
    app.bridge_event(BridgeEvent::Message(ClientMessage::Result {
        session_id: "s".into(),
        command_id: work.command_id,
        result: WorkResult::Deferred {
            target_id: "2".into(),
            code: "rate_limited".into(),
            message: "wait".into(),
            retry_at_ms: until,
        },
    }))
    .await
    .unwrap();
    assert!(!app.paused);
    assert_eq!(app.uncertain, 0);
    assert_eq!(app.batch.as_ref().unwrap().ids.front().unwrap(), "2");
    assert!(app.pacing.until_ms >= until);
    app.store
        .run(move |s| {
            assert!(s.finished_targets("b")?.is_empty());
            assert!(s.unresolved("1")?.is_empty());
            assert!(
                s.get::<forgive_me::pacing::Pacing>("pacing:1")?
                    .unwrap()
                    .until_ms
                    >= until
            );
            Ok(())
        })
        .await
        .unwrap();
}

#[test]
fn rolling_hourly_budget_and_server_cooldown_survive_serialization() {
    let mut pace = forgive_me::pacing::Pacing::default();
    assert!(pace.reserve(1));
    assert!(!pace.reserve(1));
    assert!(pace.remaining_seconds() >= 3599);
    let until = now_ms() + 7_200_000;
    pace.retry(Some(until), "rate_limited");
    let restored: forgive_me::pacing::Pacing =
        serde_json::from_str(&serde_json::to_string(&pace).unwrap()).unwrap();
    assert_eq!(restored.attempts.len(), 1);
    assert!(restored.until_ms >= until);
}
