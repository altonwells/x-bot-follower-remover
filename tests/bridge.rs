use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::{
    net::TcpListener,
    sync::mpsc,
    time::{Duration, timeout},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use x_bot_follower_remover::{
    bridge::{self, BridgeEvent},
    config::Config,
};
#[tokio::test]
async fn pairs_pins_origin_and_roundtrips_commands() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::default();
    let token = config.token.clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, mut rx) = mpsc::channel(8);
    let server = tokio::spawn(bridge::serve(listener, config, dir.path().to_owned(), tx));
    let mut request = format!("ws://{addr}/bridge").into_client_request().unwrap();
    request.headers_mut().insert(
        "Origin",
        "chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .parse()
            .unwrap(),
    );
    let (mut ws, _) = connect_async(request).await.unwrap();
    ws.send(Message::Text(json!({"type":"hello","v":1,"token":token,"extension_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}).to_string().into())).await.unwrap();
    let welcome: Value =
        serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(welcome["type"], "welcome");
    let event = timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    let BridgeEvent::Connected { sender, session_id } = event else {
        panic!("not connected")
    };
    sender
        .send(json!({"type":"pause","session_id":session_id}))
        .await
        .unwrap();
    loop {
        let msg: Value =
            serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        if msg["type"] == "pause" {
            break;
        }
    }
    assert_eq!(
        x_bot_follower_remover::config::load(dir.path())
            .unwrap()
            .extension_id
            .as_deref(),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );
    ws.send(Message::Text(
        json!({"type":"x_page_ready","session_id":session_id})
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    assert!(matches!(
        timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap(),
        BridgeEvent::Message(x_bot_follower_remover::protocol::ClientMessage::XPageReady { .. })
    ));
    ws.send(Message::Text(
        json!({"type":"heartbeat","session_id":"stale"})
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    assert!(matches!(
        timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap(),
        BridgeEvent::Disconnected(_)
    ));
    server.abort();
}
#[tokio::test]
async fn rejects_untrusted_web_origin() {
    let dir = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, _rx) = mpsc::channel(8);
    let server = tokio::spawn(bridge::serve(
        listener,
        Config::default(),
        dir.path().to_owned(),
        tx,
    ));
    let mut request = format!("ws://{addr}/bridge").into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Origin", "https://evil.example".parse().unwrap());
    assert!(connect_async(request).await.is_err());
    server.abort();
}
