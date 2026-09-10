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
    app.capabilities = vec!["remove_follower".into()];
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
