use forgive_me::{
    app::{App, Batch},
    background::{self, Control},
    bridge::BridgeEvent,
    protocol::{ClientMessage, WorkResult},
    store::Store,
};
use std::{collections::VecDeque, path::Path, time::Duration};
use tokio::sync::mpsc;

#[tokio::test]
async fn worker_survives_monitor_disconnect_and_preserves_manual_pause_on_reconnect() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(Path::new(":memory:")).unwrap();
    store.set("last_owner", &"1").unwrap();
    let batch = Batch {
        id: "approved".into(),
        ids: VecDeque::from(["2".into()]),
        policy: Default::default(),
    };
    store.set("batch:1", &Some(batch)).unwrap();
    let pace = forgive_me::pacing::Pacing {
        until_ms: forgive_me::model::now_ms() + 3_600_000,
        ..Default::default()
    };
    store.set("pacing:1", &pace).unwrap();
    let app = App::new(store, false).unwrap();
    let (events, rx) = mpsc::channel(16);
    let path = dir.path().to_path_buf();
    let worker = tokio::spawn(async move { background::run(app, rx, &path).await });
    for _ in 0..100 {
        if dir.path().join("worker.sock").exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    async fn identify(
        events: &mpsc::Sender<BridgeEvent>,
        owner: &str,
    ) -> mpsc::Receiver<serde_json::Value> {
        let (tx, mut rx) = mpsc::channel(8);
        events
            .send(BridgeEvent::Connected {
                session_id: "test".into(),
                sender: tx,
            })
            .await
            .unwrap();
        let command = rx.recv().await.unwrap();
        events
            .send(BridgeEvent::Message(ClientMessage::Result {
                session_id: "test".into(),
                command_id: command["work"]["command_id"].as_str().unwrap().into(),
                result: WorkResult::Session {
                    owner_id: owner.into(),
                    handle: "example".into(),
                    capabilities: vec!["adapter:2".into(), "durable_queue:1".into()],
                },
            }))
            .await
            .unwrap();
        // Keep receiver alive until session handling's resume control is drained (when sent).
        let _ = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await;
        rx
    }
    let _browser = identify(&events, "1").await;
    let status = background::request(dir.path(), Control::Status)
        .await
        .unwrap();
    assert_eq!(status.state, "Cooling down");
    assert_eq!(status.remaining, 1);
    assert_eq!(
        background::request(dir.path(), Control::Pause)
            .await
            .unwrap()
            .state,
        "Paused"
    );
    events
        .send(BridgeEvent::Disconnected("test disconnect".into()))
        .await
        .unwrap();
    let _browser = identify(&events, "1").await;
    assert_eq!(
        background::request(dir.path(), Control::Status)
            .await
            .unwrap()
            .state,
        "Paused"
    );
    background::request(dir.path(), Control::Stop)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), worker)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(!dir.path().join("worker.sock").exists());
}

#[tokio::test]
async fn page_ready_does_not_erase_a_scheduled_identity_retry() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(Path::new(":memory:")).unwrap();
    store.set("last_owner", &"1").unwrap();
    store
        .set(
            "batch:1",
            &Some(Batch {
                id: "approved".into(),
                ids: VecDeque::from(["2".into()]),
                policy: Default::default(),
            }),
        )
        .unwrap();
    let app = App::new(store, false).unwrap();
    let (events, rx) = mpsc::channel(16);
    let path = dir.path().to_path_buf();
    let worker = tokio::spawn(async move { background::run(app, rx, &path).await });
    for _ in 0..100 {
        if dir.path().join("worker.sock").exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let (browser, mut commands) = mpsc::channel(8);
    events
        .send(BridgeEvent::Connected {
            session_id: "test".into(),
            sender: browser,
        })
        .await
        .unwrap();
    let first = commands.recv().await.unwrap();
    events
        .send(BridgeEvent::Message(ClientMessage::Result {
            session_id: "test".into(),
            command_id: first["work"]["command_id"].as_str().unwrap().into(),
            result: WorkResult::Error {
                code: "network_unavailable".into(),
                message: "temporary read failure".into(),
                retry_at_ms: None,
            },
        }))
        .await
        .unwrap();
    events
        .send(BridgeEvent::Message(ClientMessage::XPageReady {
            session_id: "test".into(),
        }))
        .await
        .unwrap();
    let retry = tokio::time::timeout(Duration::from_secs(33), async {
        loop {
            let command = commands.recv().await.unwrap();
            if command["type"] == "command" {
                break command;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(retry["work"]["command"]["kind"], "get_session");
    assert_ne!(retry["work"]["command_id"], first["work"]["command_id"]);
    background::request(dir.path(), Control::Stop)
        .await
        .unwrap();
    worker.await.unwrap().unwrap();
}

#[tokio::test]
async fn full_auto_worker_resumes_collection_without_an_existing_removal_batch() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(Path::new(":memory:")).unwrap();
    store.set("last_owner", &"1").unwrap();
    store
        .set("auto:1", &Some(forgive_me::model::Policy::default()))
        .unwrap();
    store
        .set(
            "scan:1",
            &forgive_me::app::Scan {
                adapter_revision: 2,
                phase: "following".into(),
                started_at: forgive_me::model::now_ms(),
                ..Default::default()
            },
        )
        .unwrap();
    let app = App::new(store, false).unwrap();
    assert!(app.auto_policy.is_some() && app.paused);
    let (events, rx) = mpsc::channel(16);
    let path = dir.path().to_path_buf();
    let worker = tokio::spawn(async move { background::run(app, rx, &path).await });
    for _ in 0..100 {
        if dir.path().join("worker.sock").exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let (tx, mut rx) = mpsc::channel(8);
    events
        .send(BridgeEvent::Connected {
            session_id: "s".into(),
            sender: tx,
        })
        .await
        .unwrap();
    let work = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    events
        .send(BridgeEvent::Message(ClientMessage::Result {
            session_id: "s".into(),
            command_id: work["work"]["command_id"].as_str().unwrap().into(),
            result: WorkResult::Session {
                owner_id: "1".into(),
                handle: "demo".into(),
                capabilities: vec![
                    "adapter:2".into(),
                    "durable_queue:1".into(),
                    "saved_activity:1".into(),
                    "remove_follower".into(),
                ],
            },
        }))
        .await
        .unwrap();
    let page = tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            let message = rx.recv().await.unwrap();
            if message["type"] == "command" {
                break message;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(page["work"]["command"]["kind"], "scan_page");
    assert_eq!(page["work"]["command"]["list"], "following");
    background::request(dir.path(), Control::Cancel)
        .await
        .unwrap();
    background::request(dir.path(), Control::Stop)
        .await
        .unwrap();
    worker.await.unwrap().unwrap();
}

#[tokio::test]
async fn simple_worker_preserves_pause_after_process_restart_and_accepts_only_pacing_changes() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("cleanup.sqlite");
    let store = Store::open(&db).unwrap();
    let policy = forgive_me::model::Policy::cleanup();
    store.set("last_owner", &"1").unwrap();
    store.set("auto:1", &Some(policy.clone())).unwrap();
    store.set("managed:1", &true).unwrap();
    store
        .set(
            "scan:1",
            &forgive_me::app::Scan {
                phase: "following".into(),
                adapter_revision: 2,
                ..Default::default()
            },
        )
        .unwrap();
    let pace = forgive_me::pacing::Pacing {
        until_ms: forgive_me::model::now_ms() + 3600000,
        ..Default::default()
    };
    store.set("pacing:1", &pace).unwrap();
    drop(store);
    for restart in [false, true] {
        let app = App::new(Store::open(&db).unwrap(), false).unwrap();
        let (events, rx) = mpsc::channel(16);
        let path = dir.path().to_path_buf();
        let worker = tokio::spawn(async move { background::run(app, rx, &path).await });
        for _ in 0..100 {
            if dir.path().join("worker.sock").exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let (tx, mut browser) = mpsc::channel(16);
        events
            .send(BridgeEvent::Connected {
                session_id: "s".into(),
                sender: tx,
            })
            .await
            .unwrap();
        let request = browser.recv().await.unwrap();
        events
            .send(BridgeEvent::Message(ClientMessage::Result {
                session_id: "s".into(),
                command_id: request["work"]["command_id"].as_str().unwrap().into(),
                result: WorkResult::Session {
                    owner_id: "1".into(),
                    handle: "example".into(),
                    capabilities: vec![
                        "adapter:2".into(),
                        "durable_queue:1".into(),
                        "simple_cleanup:1".into(),
                    ],
                },
            }))
            .await
            .unwrap();
        // Ack proves the Session has been applied before reading status.
        browser.recv().await.unwrap();
        let status = background::request(dir.path(), Control::Status)
            .await
            .unwrap();
        assert_eq!(
            status.state,
            if restart { "Paused" } else { "Cooling down" }
        );
        let mut changed = policy.clone();
        changed.delay_seconds = 10;
        changed.skip_verified = false;
        let status = background::request(dir.path(), Control::Settings { policy: changed })
            .await
            .unwrap();
        assert_eq!(status.policy.delay_seconds, 10);
        assert!(status.policy.skip_verified);
        assert!(status.wait_seconds > 3500);
        background::request(dir.path(), Control::Pause)
            .await
            .unwrap();
        background::request(dir.path(), Control::Stop)
            .await
            .unwrap();
        worker.await.unwrap().unwrap();
        assert_eq!(
            Store::open(&db).unwrap().get::<bool>("managed:1").unwrap(),
            Some(false)
        );
    }
}
