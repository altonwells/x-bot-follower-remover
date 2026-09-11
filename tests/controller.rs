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
    app.capabilities = vec![
        "remove_follower".into(),
        "adapter:2".into(),
        "sparse_policy:1".into(),
        "saved_activity:1".into(),
    ];
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
            approved_account: None,
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
            approved_account: None,
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

#[tokio::test]
async fn activity_checks_follow_display_order_and_focus_the_active_account() {
    use forgive_me::protocol::{ClientMessage, WorkResult};
    let mut app = fixture();
    let zulu = app.accounts.get_mut("2").unwrap();
    zulu.handle = "zulu".into();
    zulu.checked_at_ms = None;
    let mut alpha = zulu.clone();
    alpha.id = "99".into();
    alpha.handle = "Alpha".into();
    app.accounts.insert(alpha.id.clone(), alpha);
    app.scan.adapter_revision = 2;
    app.scan.phase = "review".into();
    app.query = "zulu".into();
    app.only_matching = true;
    let (tx, mut rx) = mpsc::channel(8);
    app.sender = Some(tx);
    app.key(key('i')).await.unwrap();
    assert!(app.query.is_empty());
    assert!(!app.only_matching);
    assert_eq!(rx.recv().await.unwrap()["type"], "resume");
    for (id, focus) in [("99", 0), ("2", 1)] {
        app.next_at = std::time::Instant::now();
        app.pacing.until_ms = 0;
        app.tick().await.unwrap();
        let message = rx.recv().await.unwrap();
        assert_eq!(message["work"]["command"]["kind"], "inspect_account");
        assert_eq!(message["work"]["command"]["target_id"], id);
        assert_eq!(app.focus, focus);
        assert_eq!(app.active_target(), Some((id, "Checking activity")));
        let mut account = app.accounts[id].clone();
        account.checked_at_ms = Some(now_ms());
        app.bridge_event(BridgeEvent::Message(ClientMessage::Result {
            session_id: "s".into(),
            command_id: app.pending.as_ref().unwrap().command_id.clone(),
            result: WorkResult::Account { account },
        }))
        .await
        .unwrap();
        assert!(app.active_target().is_none());
        assert_eq!(rx.recv().await.unwrap()["type"], "ack");
    }
    app.next_at = std::time::Instant::now();
    app.pacing.until_ms = 0;
    app.tick().await.unwrap();
    assert_eq!(app.scan.phase, "done");
}

#[tokio::test]
async fn basic_selection_requires_activity_clearance_before_queueing_in_display_order() {
    let mut app = fixture();
    let base = app.accounts.get_mut("2").unwrap();
    base.handle = "zulu".into();
    base.checked_at_ms = None;
    let base = base.clone();
    for (id, handle) in [
        ("99", "alpha"),
        ("3", "verified"),
        ("4", "mutual"),
        ("5", "unknown"),
        ("6", "kept"),
        ("7", "private"),
    ] {
        let mut account = base.clone();
        account.id = id.into();
        account.handle = handle.into();
        match id {
            "3" => account.verified = Some(true),
            "4" => account.i_follow = Some(true),
            "5" => account.verified = None,
            "6" => account.kept = true,
            "7" => account.protected = Some(true),
            _ => {}
        }
        app.accounts.insert(id.into(), account);
    }
    app.key(key('a')).await.unwrap();
    assert!(app.selected.is_empty());
    app.query = "alpha".into();
    app.key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT))
        .await
        .unwrap();
    assert_eq!(app.selected.len(), 1);
    assert!(app.selected.contains("99"));
    app.query.clear();
    app.key(key('A')).await.unwrap();
    assert_eq!(app.selected.len(), 2);
    assert!(
        app.selected
            .iter()
            .all(|id| app.accounts[id].reason(&app.policy, now_ms()).is_err())
    );
    assert!(app.key(key('d')).await.is_err());
    for id in ["99", "2"] {
        app.accounts.get_mut(id).unwrap().checked_at_ms = Some(now_ms());
    }
    app.key(key('d')).await.unwrap();
    assert_eq!(app.confirmation, vec!["99", "2"]);
    app.key(key('y')).await.unwrap();
    assert_eq!(
        app.batch.as_ref().unwrap().ids,
        VecDeque::from(["99".to_string(), "2".to_string()])
    );
    assert!(app.show_queue_list);
    let (tx, mut rx) = mpsc::channel(8);
    app.sender = Some(tx);
    app.tick().await.unwrap();
    assert_eq!(
        rx.recv().await.unwrap()["work"]["command"]["target_id"],
        "99"
    );
    assert_eq!(app.focused().as_deref(), Some("99"));
    assert_eq!(app.active_target(), Some(("99", "Removing")));
}

#[test]
fn new_sparse_defaults_upgrade_future_reviews_but_not_approved_queues() {
    let store = Store::open(Path::new(":memory:")).unwrap();
    let old_policy = serde_json::json!({ "inactive_days": 30, "skip_verified": true, "skip_following": true, "include_zero_posts": true, "delay_seconds": 10, "batch_limit": 50 });
    store.set("policy", &old_policy).unwrap();
    store.set("last_owner", &"1").unwrap();
    store
        .set(
            "batch:1",
            &serde_json::json!({"id":"old", "ids":["2"], "policy":old_policy}),
        )
        .unwrap();
    let app = App::new(store, false).unwrap();
    assert_eq!(app.policy.sparse_old_max_posts, 5);
    assert_eq!(app.policy.inactive_days, 30);
    assert_eq!(app.batch.unwrap().policy.sparse_old_max_posts, 0);
}

#[tokio::test]
async fn sparse_policy_requires_browser_support_before_any_action_is_journaled() {
    let mut app = fixture();
    app.capabilities.retain(|c| c != "saved_activity:1");
    let (tx, mut rx) = mpsc::channel(8);
    app.sender = Some(tx);
    app.batch = Some(Batch {
        id: "b".into(),
        ids: VecDeque::from(["2".into()]),
        policy: app.policy.clone(),
    });
    app.paused = false;
    assert!(app.tick().await.is_err());
    assert!(app.paused);
    assert!(app.pending.is_none());
    assert!(rx.try_recv().is_err());
    app.store
        .run(|s| {
            assert!(s.unresolved("1")?.is_empty());
            Ok(())
        })
        .await
        .unwrap();
}

#[test]
fn volume_statistics_handle_small_equal_and_skewed_cohorts() {
    use forgive_me::model::post_volume;
    let mut cohort = vec![
        Account {
            follows_me: Some(true),
            posts: Some(100),
            ..Default::default()
        };
        29
    ];
    assert!(post_volume(cohort.iter(), 100).contains("Need 30"));
    cohort.push(cohort[0].clone());
    assert!(post_volume(cohort.iter(), 100).contains("counts equal"));
    cohort[0].posts = Some(0);
    assert!(post_volume(cohort.iter(), 0).contains("LOW OUTLIER"));
    cohort.push(Account {
        posts: Some(u64::MAX),
        follows_me: Some(false),
        ..Default::default()
    });
    assert!(post_volume(cohort.iter(), 0).contains("n=30"));
    cohort[1].posts = None;
    assert!(post_volume(cohort.iter(), 0).contains("Need 30"));
}

#[test]
fn batch_rest_is_durable_and_never_shortens_server_cooldowns() {
    use forgive_me::pacing::Pacing;
    let policy = Policy {
        rest_every: 2,
        rest_seconds: 300,
        ..Default::default()
    };
    let mut pace = Pacing::default();
    assert!(pace.reserve(50));
    pace.after_attempt(&policy);
    assert_eq!(pace.until_ms, 0);
    assert!(pace.reserve(50));
    pace.after_attempt(&policy);
    assert!(pace.remaining_seconds() >= 299);
    assert_eq!(pace.reason, "Batch rest");
    let mut restored: Pacing =
        serde_json::from_str(&serde_json::to_string(&pace).unwrap()).unwrap();
    assert_eq!(restored.since_rest, 0);
    assert_eq!(restored.attempts.len(), 2);
    restored.until_ms = now_ms() + 900_000;
    restored.reason = "rate_limited".into();
    restored.since_rest = 2;
    restored.after_attempt(&policy);
    assert!(restored.remaining_seconds() >= 899);
    assert_eq!(restored.reason, "rate_limited");
}

#[tokio::test]
async fn system_settings_update_real_queue_pacing_but_preserve_rules_and_waits() {
    let mut app = fixture();
    let original = app.policy.clone();
    app.batch = Some(Batch {
        id: "b".into(),
        ids: VecDeque::from(["2".into()]),
        policy: original.clone(),
    });
    app.auto_policy = Some(original.clone());
    app.pacing.until_ms = now_ms() + 900_000;
    let until = app.pacing.until_ms;
    app.key(key(',')).await.unwrap();
    assert_eq!(app.mode, Mode::Settings);
    app.key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))
        .await
        .unwrap();
    app.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
        .await
        .unwrap();
    app.key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))
        .await
        .unwrap();
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await
        .unwrap();
    assert_eq!(
        app.batch.as_ref().unwrap().policy.delay_seconds,
        original.delay_seconds - 5
    );
    assert_eq!(
        app.batch.as_ref().unwrap().policy.rest_every,
        original.rest_every - 1
    );
    assert_eq!(
        app.auto_policy.as_ref().unwrap().delay_seconds,
        original.delay_seconds - 5
    );
    assert_eq!(
        app.batch.as_ref().unwrap().policy.inactive_days,
        original.inactive_days
    );
    assert_eq!(app.pacing.until_ms, until);
}

#[tokio::test]
async fn full_auto_collects_checks_removes_only_cleared_accounts_and_finishes() {
    use forgive_me::protocol::{ClientMessage, WorkResult};
    async fn reply(app: &mut App, result: WorkResult, rx: &mut mpsc::Receiver<serde_json::Value>) {
        let id = app.pending.as_ref().unwrap().command_id.clone();
        app.bridge_event(BridgeEvent::Message(ClientMessage::Result {
            session_id: "s".into(),
            command_id: id,
            result,
        }))
        .await
        .unwrap();
        assert_eq!(rx.recv().await.unwrap()["type"], "ack");
        app.next_at = std::time::Instant::now();
        app.pacing.until_ms = 0;
    }
    let mut app = fixture();
    let mut inactive = app.accounts["2"].clone();
    inactive.handle = "alpha".into();
    inactive.checked_at_ms = None;
    let mut active = inactive.clone();
    active.id = "99".into();
    active.handle = "zulu".into();
    active.posts = Some(20);
    active.last_activity_ms = Some(now_ms());
    let mut verified = inactive.clone();
    verified.id = "3".into();
    verified.verified = Some(true);
    let mut mutual = inactive.clone();
    mutual.id = "4".into();
    mutual.i_follow = Some(true);
    app.policy.skip_verified = false;
    app.policy.skip_following = false;
    app.scan.phase = "review".into();
    app.scan.adapter_revision = 2;
    let (tx, mut rx) = mpsc::channel(16);
    app.sender = Some(tx);
    app.key(key('F')).await.unwrap();
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await
        .unwrap();
    assert!(app.auto_policy.is_none());
    app.key(key('F')).await.unwrap();
    app.key(key('y')).await.unwrap();
    assert!(app.auto_policy.as_ref().unwrap().skip_verified);
    assert!(app.auto_policy.as_ref().unwrap().skip_following);
    assert_eq!(rx.recv().await.unwrap()["type"], "resume");
    for (list, accounts) in [
        ("following", vec![mutual.clone()]),
        (
            "followers",
            vec![inactive.clone(), active.clone(), verified, mutual],
        ),
    ] {
        app.tick().await.unwrap();
        assert_eq!(rx.recv().await.unwrap()["work"]["command"]["list"], list);
        reply(
            &mut app,
            WorkResult::Page {
                list: list.into(),
                accounts,
                next_cursor: None,
                complete: true,
            },
            &mut rx,
        )
        .await;
    }
    assert_eq!(app.scan.phase, "inspect");
    assert!(!app.paused);
    app.tick().await.unwrap();
    assert_eq!(
        rx.recv().await.unwrap()["work"]["command"]["target_id"],
        "2"
    );
    inactive.checked_at_ms = Some(now_ms());
    reply(&mut app, WorkResult::Account { account: inactive }, &mut rx).await;
    assert!(app.batch.is_some());
    app.store
        .run(|s| {
            assert!(
                s.accounts("1")?
                    .iter()
                    .any(|a| a.id == "2" && a.checked_at_ms.is_some())
            );
            assert!(s.get::<Option<Batch>>("batch:1")?.flatten().is_some());
            Ok(())
        })
        .await
        .unwrap();
    app.tick().await.unwrap();
    let removal = rx.recv().await.unwrap();
    assert_eq!(removal["work"]["command"]["kind"], "remove_follower");
    assert_eq!(removal["work"]["command"]["target_id"], "2");
    reply(
        &mut app,
        WorkResult::Action {
            target_id: "2".into(),
            status: "verified_removed".into(),
            message: "done".into(),
        },
        &mut rx,
    )
    .await;
    assert_eq!(app.animation.departures.len(), 1);
    assert_eq!(app.animation.departures[0].handle, "alpha");
    app.tick().await.unwrap(); // finish this ready queue and continue Full Auto
    app.tick().await.unwrap();
    assert_eq!(
        rx.recv().await.unwrap()["work"]["command"]["target_id"],
        "99"
    );
    active.checked_at_ms = Some(now_ms());
    reply(&mut app, WorkResult::Account { account: active }, &mut rx).await;
    assert!(app.batch.is_none());
    app.tick().await.unwrap();
    assert!(app.auto_policy.is_none());
    assert!(app.paused);
    assert_eq!(app.removed, 1);
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn cancelling_full_auto_ignores_a_late_activity_result_and_clears_saved_run() {
    use forgive_me::protocol::{ClientMessage, WorkResult};
    let mut app = fixture();
    app.auto_policy = Some(app.policy.clone());
    app.scan.phase = "inspect".into();
    let account = app.accounts["2"].clone();
    app.pending = Some(Work {
        command_id: "inspection".into(),
        owner_id: "1".into(),
        command: Command::InspectAccount {
            target_id: "2".into(),
            policy: app.policy.clone(),
        },
    });
    app.key(key('c')).await.unwrap();
    app.bridge_event(BridgeEvent::Message(ClientMessage::Result {
        session_id: "s".into(),
        command_id: "inspection".into(),
        result: WorkResult::Account { account },
    }))
    .await
    .unwrap();
    assert!(app.auto_policy.is_none());
    assert!(app.batch.is_none());
    assert!(app.paused);
    app.store
        .run(|s| {
            assert!(s.get::<Option<Policy>>("auto:1")?.flatten().is_none());
            assert!(s.get::<Option<Batch>>("batch:1")?.flatten().is_none());
            Ok(())
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn final_collection_receipt_cannot_undo_a_full_auto_pause() {
    use forgive_me::protocol::{ClientMessage, WorkResult};
    let mut app = fixture();
    app.auto_policy = Some(app.policy.clone());
    app.scan.phase = "followers".into();
    app.paused = false;
    app.pending = Some(Work {
        command_id: "last-page".into(),
        owner_id: "1".into(),
        command: Command::ScanPage {
            list: "followers".into(),
            cursor: None,
        },
    });
    app.key(key('p')).await.unwrap();
    app.bridge_event(BridgeEvent::Message(ClientMessage::Result {
        session_id: "s".into(),
        command_id: "last-page".into(),
        result: WorkResult::Page {
            list: "followers".into(),
            accounts: vec![],
            next_cursor: None,
            complete: true,
        },
    }))
    .await
    .unwrap();
    assert!(app.paused);
    assert!(app.auto_policy.is_some());
    assert_eq!(app.scan.phase, "inspect");
}
