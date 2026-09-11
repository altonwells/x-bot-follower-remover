use std::{collections::VecDeque, path::Path, time::Instant};
use tokio::sync::mpsc;
use x_bot_follower_remover::{
    app::{App, Batch},
    bridge::BridgeEvent,
    model::{Account, Policy, now_ms},
    protocol::{ClientMessage, Command, WorkResult},
    store::Store,
};

fn fixture() -> (App, mpsc::Receiver<serde_json::Value>) {
    let mut app = App::new(Store::open(Path::new(":memory:")).unwrap(), true).unwrap();
    app.owner = "1".into();
    app.handle = "owner".into();
    app.policy = Policy::cleanup();
    let (tx, rx) = mpsc::channel(64);
    app.sender = Some(tx);
    app.paused = false;
    app.scan.phase = "inspect".into();
    app.scan.adapter_revision = 2;
    app.scan.started_at = now_ms() - 1000;
    app.auto_policy = Some(app.policy.clone());
    app.managed_run = true;
    for (id, name) in [("2", "alpha"), ("3", "beta")] {
        app.accounts.insert(
            id.into(),
            Account {
                id: id.into(),
                handle: name.into(),
                follows_me: Some(true),
                i_follow: Some(false),
                verified: Some(false),
                protected: Some(false),
                posts: Some(20),
                ..Default::default()
            },
        );
    }
    (app, rx)
}
fn ready(app: &mut App) {
    app.next_at = Instant::now();
    app.pacing.until_ms = 0;
}
async fn reply(app: &mut App, result: WorkResult) {
    let id = app.pending.as_ref().unwrap().command_id.clone();
    app.bridge_event(BridgeEvent::Message(ClientMessage::Result {
        session_id: app.session.clone(),
        command_id: id,
        result,
    }))
    .await
    .unwrap();
    ready(app);
}
fn old(mut a: Account) -> Account {
    a.checked_at_ms = Some(now_ms());
    a.last_activity_ms = Some(now_ms() - 60 * 86_400_000);
    a.coverage_since_ms = Some(now_ms() - 31 * 86_400_000);
    a
}

#[tokio::test]
async fn unreadable_account_is_retried_without_blocking_next_follower() {
    let (mut app, mut rx) = fixture();
    app.tick().await.unwrap();
    assert_eq!(
        rx.recv().await.unwrap()["work"]["command"]["target_id"],
        "2"
    );
    let mut a = app.accounts["2"].clone();
    a.checked_at_ms = Some(now_ms());
    reply(&mut app, WorkResult::Account { account: a }).await;
    assert!(app.batch.is_none());
    assert_eq!(app.retries["2"].attempts, 1);
    assert!(!app.paused);
    app.tick().await.unwrap();
    assert!(
        matches!(&app.pending.as_ref().unwrap().command,Command::InspectAccount{target_id,..} if target_id=="3")
    );
    app.store
        .run(|s| {
            assert!(
                s.get::<serde_json::Value>("retries:1")?.unwrap()["2"]["due_ms"]
                    .as_i64()
                    .unwrap()
                    > now_ms()
            );
            Ok(())
        })
        .await
        .unwrap();
}
#[tokio::test]
async fn uncertain_removal_is_durable_and_other_accounts_continue_before_recovery() {
    let (mut app, _rx) = fixture();
    let a = old(app.accounts["2"].clone());
    app.accounts.insert("2".into(), a);
    app.batch = Some(Batch {
        id: "batch".into(),
        ids: VecDeque::from(["2".into()]),
        policy: app.policy.clone(),
    });
    app.tick().await.unwrap();
    reply(
        &mut app,
        WorkResult::Action {
            target_id: "2".into(),
            status: "uncertain".into(),
            message: "Relationship still present".into(),
        },
    )
    .await;
    assert!(!app.paused);
    assert_eq!(app.uncertain, 1);
    app.tick().await.unwrap(); // end singleton batch
    app.tick().await.unwrap();
    assert!(
        matches!(&app.pending.as_ref().unwrap().command,Command::InspectAccount{target_id,..} if target_id=="3")
    );
    let b = old(app.accounts["3"].clone());
    reply(&mut app, WorkResult::Account { account: b }).await;
    app.retries.get_mut("2").unwrap().due_ms = 0;
    app.tick().await.unwrap();
    assert!(
        matches!(&app.pending.as_ref().unwrap().command,Command::Reconcile{target_id,..} if target_id=="2")
    );
    reply(
        &mut app,
        WorkResult::Action {
            target_id: "2".into(),
            status: "already_absent".into(),
            message: "Absent after recovery".into(),
        },
    )
    .await;
    assert_eq!(app.uncertain, 0);
    assert_eq!(app.accounts["2"].follows_me, Some(false));
    app.tick().await.unwrap();
    assert!(
        matches!(&app.pending.as_ref().unwrap().command,Command::RemoveFollower{target_id,..} if target_id=="3")
    );
}
#[test]
fn approved_simple_snapshot_survives_days_but_recent_or_unknown_never_qualifies() {
    let p = Policy::cleanup();
    let now = now_ms();
    let a = old(Account {
        follows_me: Some(true),
        i_follow: Some(false),
        verified: Some(false),
        protected: Some(false),
        posts: Some(20),
        ..Default::default()
    });
    assert!(a.reason(&p, now + 3 * 86_400_000).is_err());
    assert!(a.approved_reason(&p, now + 3 * 86_400_000).is_ok());
    let mut recent = a.clone();
    recent.last_activity_ms = Some(now);
    assert!(recent.approved_reason(&p, now + 3 * 86_400_000).is_err());
    let mut unknown = a.clone();
    unknown.coverage_since_ms = None;
    assert!(unknown.approved_reason(&p, now + 3 * 86_400_000).is_err());
    unknown.posts = Some(1);
    assert!(unknown.approved_reason(&p, now).is_err());
}
#[tokio::test]
async fn begin_cleanup_uses_exact_rule_and_requests_worker_handoff() {
    let (mut app, _rx) = fixture();
    app.demo = false;
    app.auto_policy = None;
    app.paused = true;
    app.capabilities = vec!["adapter:2".into(), "simple_cleanup:1".into()];
    app.policy.inactive_days = 60;
    app.policy.skip_following = false;
    app.policy.skip_verified = false;
    app.uncertain = 1; // old unresolved actions may be recovered by the approved worker
    app.begin_cleanup().await.unwrap();
    assert_eq!(app.policy.inactive_days, 30);
    assert!(app.policy.skip_verified && app.policy.skip_following);
    assert_eq!(app.policy.sparse_old_max_posts, 0);
    assert!(app.detach_requested);
    assert!(app.managed_run);
    app.store
        .run(|s| {
            assert_eq!(s.get::<bool>("managed:1")?, Some(true));
            Ok(())
        })
        .await
        .unwrap();
}
#[tokio::test]
async fn transient_reconciliation_error_waits_and_authentication_error_pauses() {
    let (mut app, _rx) = fixture();
    let a = old(app.accounts["2"].clone());
    app.accounts.insert("2".into(), a);
    app.batch = Some(Batch {
        id: "b".into(),
        ids: VecDeque::from(["2".into()]),
        policy: app.policy.clone(),
    });
    app.tick().await.unwrap();
    reply(
        &mut app,
        WorkResult::Action {
            target_id: "2".into(),
            status: "uncertain".into(),
            message: "unknown".into(),
        },
    )
    .await;
    app.retries.get_mut("2").unwrap().due_ms = 0;
    app.tick().await.unwrap();
    reply(
        &mut app,
        WorkResult::Error {
            code: "rate_limited".into(),
            message: "wait".into(),
            retry_at_ms: Some(now_ms() + 60000),
        },
    )
    .await;
    assert!(!app.paused);
    assert_eq!(app.uncertain, 1);
    app.tick().await.unwrap();
    reply(
        &mut app,
        WorkResult::Error {
            code: "login_required".into(),
            message: "Sign in".into(),
            retry_at_ms: None,
        },
    )
    .await;
    assert!(app.paused);
}

#[tokio::test]
async fn successful_reinspection_does_not_reset_failed_removal_budget() {
    let (mut app, _rx) = fixture();
    app.retries.insert(
        "2".into(),
        x_bot_follower_remover::app::Retry {
            attempts: 3,
            due_ms: 0,
        },
    );
    app.tick().await.unwrap();
    let a = old(app.accounts["2"].clone());
    reply(&mut app, WorkResult::Account { account: a }).await;
    assert_eq!(app.retries["2"].attempts, 3);
    app.tick().await.unwrap();
    reply(
        &mut app,
        WorkResult::Action {
            target_id: "2".into(),
            status: "failed".into(),
            message: "Still present".into(),
        },
    )
    .await;
    assert_eq!(app.retries["2"].attempts, 4);
    app.tick().await.unwrap();
    app.tick().await.unwrap();
    assert!(
        matches!(&app.pending.as_ref().unwrap().command,Command::InspectAccount{target_id,..} if target_id=="3")
    );
}

#[tokio::test]
async fn login_failure_keeps_target_for_resume_without_another_activity_check() {
    let (mut app, _rx) = fixture();
    let a = old(app.accounts["2"].clone());
    app.accounts.insert("2".into(), a);
    app.batch = Some(Batch {
        id: "auth".into(),
        ids: VecDeque::from(["2".into()]),
        policy: app.policy.clone(),
    });
    app.tick().await.unwrap();
    reply(
        &mut app,
        WorkResult::Action {
            target_id: "2".into(),
            status: "failed".into(),
            message: "login_required: Sign in".into(),
        },
    )
    .await;
    assert!(app.paused);
    assert!(!app.managed_run);
    assert_eq!(app.retries["2"].attempts, 1);
    app.paused = false;
    app.retries.get_mut("2").unwrap().due_ms = 0;
    app.tick().await.unwrap(); // close the finished singleton batch
    app.tick().await.unwrap(); // reuse approved evidence for the retry
    app.tick().await.unwrap();
    assert!(
        matches!(&app.pending.as_ref().unwrap().command,Command::RemoveFollower{target_id,..} if target_id=="2")
    );
}
