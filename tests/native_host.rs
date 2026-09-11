//! Real native-message child process -> authenticated local WebSocket. No Chrome or X.
use fs2::FileExt;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};
use tokio::{net::TcpListener, sync::mpsc};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use x_bot_follower_remover::{bridge, config, native};

#[tokio::test]
async fn native_pairing_connects_to_the_real_bridge_without_manual_secrets() {
    let data = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let settings = config::Config {
        port,
        ..Default::default()
    };
    config::save(data.path(), &settings).unwrap();
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(data.path().join("app.lock"))
        .unwrap();
    lock.lock_exclusive().unwrap();
    // Match an installed bundle without depending on an existing npm build.
    let extension = data.path().join("remover-extension");
    fs::create_dir(&extension).unwrap();
    fs::write(extension.join("manifest.json"), "{}").unwrap();
    let binary = data.path().join("remover");
    fs::copy(env!("CARGO_BIN_EXE_remover"), &binary).unwrap();
    let id = native::extension_id(&extension).unwrap();
    let mut host = Command::new(binary)
        .arg("--data-dir")
        .arg(data.path())
        .arg("native-host")
        .arg(format!("chrome-extension://{id}/"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let request = serde_json::to_vec(&json!({"type":"pair", "v":1})).unwrap();
    let mut input = host.stdin.take().unwrap();
    input
        .write_all(&(request.len() as u32).to_ne_bytes())
        .unwrap();
    input.write_all(&request).unwrap();
    drop(input);
    let output = host.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let len = u32::from_ne_bytes(output.stdout[..4].try_into().unwrap()) as usize;
    assert_eq!(output.stdout.len(), len + 4);
    let pair: Value = serde_json::from_slice(&output.stdout[4..]).unwrap();
    assert_eq!(pair["ok"], true);
    assert_eq!(pair["port"], port);
    let (tx, mut rx) = mpsc::channel(8);
    let server = tokio::spawn(bridge::serve(listener, settings, data.path().into(), tx));
    let mut request = format!("ws://127.0.0.1:{port}/bridge")
        .into_client_request()
        .unwrap();
    request.headers_mut().insert(
        "Origin",
        format!("chrome-extension://{id}").parse().unwrap(),
    );
    let (mut socket, _) = connect_async(request).await.unwrap();
    socket
        .send(Message::Text(
            json!({"v":1,"type":"hello","token":pair["token"],"extension_id":id})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let reply = socket.next().await.unwrap().unwrap();
    let value: Value = serde_json::from_str(reply.to_text().unwrap()).unwrap();
    assert_eq!(value["type"], "welcome");
    assert!(matches!(
        rx.recv().await.unwrap(),
        bridge::BridgeEvent::Connected { .. }
    ));
    assert_eq!(
        config::load(data.path()).unwrap().extension_id.as_deref(),
        Some(id.as_str())
    );
    server.abort();
}
