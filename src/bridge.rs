use crate::{
    config::{self, Config},
    model::new_id,
    protocol::{ClientMessage, VERSION},
};
use anyhow::{Context, Result, bail};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};
use subtle::ConstantTimeEq;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
    time::{Instant, timeout},
};
use tokio_tungstenite::{
    accept_hdr_async_with_config,
    tungstenite::{
        Message,
        handshake::server::{Request, Response},
        protocol::WebSocketConfig,
    },
};

// Bounded, low-volume channel; keeping messages inline avoids an allocation per event.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum BridgeEvent {
    Connected {
        session_id: String,
        sender: mpsc::Sender<Value>,
    },
    ExtensionVersion(String),
    Disconnected(String),
    Message(ClientMessage),
}
pub fn valid_origin(origin: &str) -> Option<&str> {
    let id = origin.strip_prefix("chrome-extension://")?;
    (id.len() == 32 && id.bytes().all(|b| (b'a'..=b'p').contains(&b))).then_some(id)
}

pub async fn serve(
    listener: TcpListener,
    mut config: Config,
    dir: PathBuf,
    events: mpsc::Sender<BridgeEvent>,
) -> Result<()> {
    loop {
        if events.is_closed() {
            return Ok(());
        }
        let (socket, _) = listener.accept().await?;
        if let Err(error) = connection(socket, &mut config, &dir, &events).await
            && events
                .send(BridgeEvent::Disconnected(format!("{error:#}")))
                .await
                .is_err()
        {
            return Ok(());
        }
    }
}
async fn connection(
    socket: TcpStream,
    config: &mut Config,
    dir: &std::path::Path,
    events: &mpsc::Sender<BridgeEvent>,
) -> Result<()> {
    let mut origin = String::new();
    // Tungstenite requires this exact HTTP error response type.
    #[allow(clippy::result_large_err)]
    let callback = |req: &Request, response: Response| {
        origin = req
            .headers()
            .get("origin")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("")
            .to_string();
        if valid_origin(&origin).is_none() || req.uri().path() != "/bridge" {
            return Err(tokio_tungstenite::tungstenite::http::Response::builder()
                .status(403)
                .body(Some("Forbidden".into()))
                .unwrap());
        }
        Ok(response)
    };
    let limits = WebSocketConfig::default()
        .max_message_size(Some(1_048_576))
        .max_frame_size(Some(1_048_576));
    let mut ws = timeout(
        Duration::from_secs(5),
        accept_hdr_async_with_config(socket, callback, Some(limits)),
    )
    .await??;
    let first = timeout(Duration::from_secs(5), ws.next())
        .await?
        .context("Disconnected before pairing")??;
    let hello: ClientMessage = serde_json::from_str(first.to_text()?)?;
    let ClientMessage::Hello {
        v,
        token,
        extension_id,
        extension_version,
    } = hello
    else {
        bail!("Pairing required");
    };
    if v != VERSION
        || !bool::from(token.as_bytes().ct_eq(config.token.as_bytes()))
        || valid_origin(&origin) != Some(extension_id.as_str())
    {
        bail!("Pairing rejected: check token and protocol version");
    }
    if config
        .extension_id
        .as_ref()
        .is_some_and(|id| id != &extension_id)
    {
        bail!("Extension ID differs from the paired extension; run remover pair --reset");
    }
    if config.extension_id.is_none() {
        config.extension_id = Some(extension_id);
        config::save(dir, config)?;
    }
    let session_id = new_id();
    ws.send(Message::Text(
        json!({"v":VERSION,"type":"welcome","manager_version":1,"session_id":session_id})
            .to_string()
            .into(),
    ))
    .await?;
    let (sender, mut outgoing) = mpsc::channel::<Value>(8);
    events
        .send(BridgeEvent::Connected {
            session_id: session_id.clone(),
            sender,
        })
        .await?;
    if let Some(version) = extension_version {
        events
            .send(BridgeEvent::ExtensionVersion(
                crate::model::clean(&version).chars().take(24).collect(),
            ))
            .await?;
    }
    let mut tick = tokio::time::interval(Duration::from_secs(10));
    let mut alive = Instant::now();
    loop {
        tokio::select! {
            _=tick.tick()=>{
                if alive.elapsed()>Duration::from_secs(35) { bail!("Extension heartbeat lost; work paused"); }
                ws.send(Message::Text(json!({"type":"heartbeat","session_id":session_id}).to_string().into())).await?;
            }
            item=outgoing.recv()=>match item {
                Some(v)=>ws.send(Message::Text(v.to_string().into())).await?,
                None=>{let _=ws.close(None).await;return Ok(())}
            },
            item=ws.next()=>{
                match item.context("Chrome disconnected; work paused")?? {
                    Message::Text(text)=>{
                        let msg:ClientMessage=serde_json::from_str(&text)?;
                        let received_session=match &msg {
                            ClientMessage::Manager{session_id,..}|ClientMessage::Heartbeat{session_id}|ClientMessage::XPageReady{session_id}|ClientMessage::Result{session_id,..}|ClientMessage::Recovery{session_id,..}=>session_id,
                            ClientMessage::Hello{..}=>bail!("Unexpected pairing message"),
                        };
                        if received_session!=&session_id { bail!("Stale session rejected"); }
                        alive=Instant::now();
                        if !matches!(msg,ClientMessage::Heartbeat{..}) { events.send(BridgeEvent::Message(msg)).await?; }
                    }
                    Message::Close(_)=>bail!("Chrome disconnected; work paused"),
                    Message::Ping(data)=>ws.send(Message::Pong(data)).await?,
                    _=>{}
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_web_origins_and_lookalikes() {
        assert!(valid_origin("https://x.com").is_none());
        assert!(valid_origin("chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.evil").is_none());
        assert_eq!(
            valid_origin("chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
    }
}
