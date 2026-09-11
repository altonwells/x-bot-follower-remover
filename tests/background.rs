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
