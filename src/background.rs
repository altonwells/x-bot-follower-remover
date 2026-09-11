//! Detached controller and private local controls. Chrome still owns X credentials.
use crate::{app::App, bridge::BridgeEvent, model::now_ms};
use anyhow::{Context, Result, bail};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::{mpsc, oneshot},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    Status,
    Pause,
    Resume,
    Cancel,
    Stop,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Status {
    pub handle: String,
    pub state: String,
    pub remaining: usize,
    pub removed: usize,
    pub uncertain: usize,
    pub wait_seconds: i64,
    pub message: String,
}
impl Status {
    fn from_app(app: &App) -> Self {
        Self {
            handle: app.handle.clone(),
            state: if app.sender.is_none() {
                "Waiting for Chrome"
            } else if app.handle.is_empty() {
                "Checking account"
            } else if app.uncertain > 0 {
                "Needs reconciliation"
            } else if app.batch.is_none() {
                "No queued work"
            } else if app.paused {
                "Paused"
            } else if app.pacing.until_ms > now_ms() {
                "Cooling down"
            } else if app.batch.is_some() {
                "Running"
            } else {
                "Complete"
            }
            .into(),
            remaining: app.batch.as_ref().map_or(0, |b| b.ids.len()),
            removed: app.removed,
            uncertain: app.uncertain,
            wait_seconds: app.pacing.remaining_seconds(),
            message: app.notice.clone(),
        }
    }
}
pub async fn request(dir: &Path, command: Control) -> Result<Status> {
    tokio::time::timeout(Duration::from_secs(3), async {
        let mut stream = UnixStream::connect(dir.join("worker.sock"))
            .await
            .context("No background worker. Start a removal queue, then press b in the TUI")?;
        stream
            .write_all(format!("{}\n", serde_json::to_string(&command)?).as_bytes())
            .await?;
        let mut line = String::new();
        BufReader::new(stream)
            .take(16_384)
            .read_line(&mut line)
            .await?;
        anyhow::ensure!(line.len() < 16_384, "Invalid background response");
        Ok(serde_json::from_str(&line)?)
    })
    .await
    .context("Background worker did not respond")?
}
pub fn spawn(dir: &Path) -> Result<()> {
    use std::os::unix::process::CommandExt;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("worker.log"))?;
    std::process::Command::new(std::env::current_exe()?.canonicalize()?)
        .arg("--data-dir")
        .arg(dir)
        .arg("worker")
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .process_group(0)
        .spawn()?;
    Ok(())
}
pub async fn run(mut app: App, mut events: mpsc::Receiver<BridgeEvent>, dir: &Path) -> Result<()> {
    anyhow::ensure!(app.batch.is_some(), "No approved removal queue to run");
    let expected_owner = app.owner.clone();
    let mut automatic = true;
    let mut identity_retry = false;
    let socket = dir.join("worker.sock");
    if socket.exists() {
        std::fs::remove_file(&socket)?;
    }
    let listener = UnixListener::bind(&socket)?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
    let (tx, mut rx) = mpsc::channel::<(Control, oneshot::Sender<Status>)>(8);
    let accept = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let tx = tx.clone();
            tokio::spawn(async move {
                let _ = tokio::time::timeout(Duration::from_secs(3), async {
                    let mut reader = BufReader::new(stream);
                    // Commands are tiny and fixed. Bound the read before allocating.
                    let mut bytes = Vec::new();
                    loop {
                        let n = tokio::io::AsyncReadExt::read_u8(&mut reader).await?;
                        if n == b'\n' {
                            break;
                        }
                        bytes.push(n);
                        if bytes.len() > 64 {
                            bail!("Invalid worker command");
                        }
                    }
                    let command = serde_json::from_slice(&bytes)?;
                    let (reply, response) = oneshot::channel();
                    tx.send((command, reply)).await?;
                    let state = response.await?;
                    reader
                        .get_mut()
                        .write_all(format!("{}\n", serde_json::to_string(&state)?).as_bytes())
                        .await?;
                    Ok::<_, anyhow::Error>(())
                })
                .await;
            });
        }
    });
    let mut tick = tokio::time::interval(Duration::from_millis(250));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let result = tokio::select! {
            event = events.recv() => match event {
                Some(event) => {
                    if matches!(&event, BridgeEvent::Connected { .. } | BridgeEvent::Disconnected(_)) { identity_retry = false; }
                    if let BridgeEvent::Message(crate::protocol::ClientMessage::Result { result, .. }) = &event
                        && app.pending.as_ref().is_some_and(|w| matches!(w.command, crate::protocol::Command::GetSession)) {
                        identity_retry = matches!(result, crate::protocol::WorkResult::Error { code, .. } if matches!(code.as_str(), "rate_limited" | "network_unavailable"));
                    }
                    let identified = matches!(&event, BridgeEvent::Message(crate::protocol::ClientMessage::Result { result: crate::protocol::WorkResult::Session { .. }, .. }));
                    let result = app.bridge_event(event).await;
                    // A reconnect may resume only the owner and queue already approved by the user.
                    if !app.handle.is_empty() && app.owner != expected_owner {
                        automatic = false; app.pause().await?; app.log("X account changed. Stop the worker and confirm the account in the TUI.");
                    } else if identified && result.is_ok() && automatic && !app.handle.is_empty() && app.paused && app.batch.is_some() && app.uncertain == 0 {
                        if app.capabilities.iter().any(|c| c == "durable_queue:1") {
                            app.key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE)).await?;
                        } else { automatic = false; app.log("Reload the Chrome extension. Durable queue support is missing."); }
                    }
                    result
                },
                None => break,
            },
            Some((command, reply)) = rx.recv() => {
                let result = match command {
                    Control::Status => Ok(()),
                    Control::Pause => { automatic = false; app.pause().await },
                    Control::Resume => {
                        if app.owner != expected_owner || app.handle.is_empty() || app.uncertain > 0 || !app.capabilities.iter().any(|c| c == "durable_queue:1") { app.log("Reconnect the approved account and resolve uncertain actions first."); Ok(()) }
                        else { automatic = true; if app.paused { app.key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE)).await } else { Ok(()) } }
                    },
                    Control::Cancel => { automatic = false; app.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)).await },
                    Control::Stop => { automatic = false; app.quit = true; app.pause().await },
                };
                let _ = reply.send(Status::from_app(&app));
                result
            },
            _ = tick.tick() => {
                if automatic && identity_retry && app.sender.is_some() && app.pending.is_none() && app.pacing.until_ms <= now_ms() {
                    identity_retry = false; app.check_session().await
                } else { app.tick().await }
            },
            _ = tokio::signal::ctrl_c() => { app.quit = true; app.pause().await },
        };
        if let Err(error) = result {
            automatic = false;
            app.pause().await?;
            app.log(format!("{error:#}"));
        }
        // Fatal results pause within App without returning an error. Do not auto-resume those.
        if app.paused && !app.handle.is_empty() && app.pending.is_none() {
            automatic = false;
        }
        if app.quit {
            break;
        }
    }
    accept.abort();
    let _ = std::fs::remove_file(socket);
    Ok(())
}
