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
    Start,
    Pause,
    Resume,
    Cancel,
    Stop,
    Settings { policy: crate::model::Policy },
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
    #[serde(default)]
    pub collected: usize,
    #[serde(default)]
    pub checked: usize,
    #[serde(default)]
    pub kept: usize,
    #[serde(default)]
    pub retry_later: usize,
    #[serde(default)]
    pub policy: crate::model::Policy,
    #[serde(default)]
    pub rows: Vec<(String, String, String)>,
    #[serde(default)]
    pub estimate_seconds: Option<i64>,
}
impl Status {
    fn from_app(app: &App) -> Self {
        let checked = app
            .accounts
            .values()
            .filter(|a| a.checked_at_ms.is_some_and(|t| t >= app.scan.started_at))
            .count();
        let unchecked = app
            .accounts
            .values()
            .filter(|a| {
                a.basic_candidate(&app.policy)
                    && a.checked_at_ms.is_none_or(|t| t < app.scan.started_at)
            })
            .count();
        Self {
            estimate_seconds: if checked >= 20 && app.scan.started_at > 0 && unchecked > 0 {
                Some(
                    ((now_ms() - app.scan.started_at).max(0) / 1000) * unchecked as i64
                        / checked as i64,
                )
            } else {
                None
            },
            handle: app.handle.clone(),
            state: if app.sender.is_none() {
                "Waiting for Chrome"
            } else if app.handle.is_empty() {
                "Checking account"
            } else if app.uncertain > 0 && !app.simple_running() {
                "Needs reconciliation"
            } else if app.batch.is_none() && app.auto_policy.is_none() {
                "No queued work"
            } else if app.paused {
                "Paused"
            } else if app.pacing.until_ms > now_ms() {
                "Cooling down"
            } else if app.auto_policy.is_some() {
                "Full Auto"
            } else {
                "Running"
            }
            .into(),
            remaining: app.batch.as_ref().map_or(0, |b| b.ids.len()),
            removed: app.removed,
            uncertain: app.uncertain,
            wait_seconds: app.pacing.remaining_seconds(),
            message: app.notice.clone(),
            collected: app
                .accounts
                .values()
                .filter(|a| a.follows_me == Some(true))
                .count()
                .max(app.scan.collected_total),
            checked,
            kept: app
                .accounts
                .values()
                .filter(|a| {
                    a.follows_me == Some(true)
                        && (a.verified == Some(true)
                            || a.i_follow == Some(true)
                            || a.kept
                            || a.last_activity_ms
                                .is_some_and(|t| t > now_ms() - 30 * 86_400_000))
                })
                .count(),
            retry_later: app.retries.len(),
            policy: app.policy.clone(),
            rows: app
                .ordered_followers()
                .into_iter()
                .skip(app.focus.saturating_sub(2))
                .take(12)
                .map(|a| {
                    let date = a
                        .last_activity_ms
                        .map(|t| format!("{}d ago", (now_ms() - t).max(0) / 86_400_000))
                        .unwrap_or_else(|| {
                            if a.posts == Some(0) {
                                "No posts".into()
                            } else {
                                "Not checked".into()
                            }
                        });
                    let state = if app.pending.as_ref().is_some_and(|w| match &w.command {
                        crate::protocol::Command::InspectAccount { target_id, .. }
                        | crate::protocol::Command::RemoveFollower { target_id, .. }
                        | crate::protocol::Command::Reconcile { target_id, .. } => {
                            target_id == &a.id
                        }
                        _ => false,
                    }) {
                        "Working".into()
                    } else if app.retries.contains_key(&a.id) {
                        "Retry later".into()
                    } else if a.approved_reason(&app.policy, now_ms()).is_ok() {
                        "Queued".into()
                    } else if a.verified == Some(true) {
                        "Keep: verified".into()
                    } else if a.i_follow == Some(true) {
                        "Keep: you follow".into()
                    } else if a.checked_at_ms.is_some_and(|t| t >= app.scan.started_at) {
                        "Keep / unavailable".into()
                    } else {
                        "Waiting".into()
                    };
                    (a.handle.chars().take(20).collect(), date, state)
                })
                .collect(),
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
            .take(65_536)
            .read_line(&mut line)
            .await?;
        anyhow::ensure!(line.len() < 65_536, "Invalid background response");
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
    anyhow::ensure!(
        app.batch.is_some() || app.auto_policy.is_some(),
        "No approved queue or Full Auto run"
    );
    let expected_owner = app.owner.clone();
    let mut automatic = !app.simple_running() || app.managed_run;
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
                        if bytes.len() > 2048 {
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
    let mut awake: Option<std::process::Child> = None;
    let mut tick = tokio::time::interval(Duration::from_millis(250));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let result = tokio::select! {
            event = events.recv() => match event {
                Some(event) => {
                    if matches!(&event, BridgeEvent::Connected { .. } | BridgeEvent::Disconnected(_)) { identity_retry = false; }
                    if matches!(&event, BridgeEvent::Message(crate::protocol::ClientMessage::XPageReady { .. })) && app.sender.is_some() && app.handle.is_empty() {
                        identity_retry = true;
                    }
                    if let BridgeEvent::Message(crate::protocol::ClientMessage::Result { result, .. }) = &event
                        && app.pending.as_ref().is_some_and(|w| matches!(w.command, crate::protocol::Command::GetSession)) {
                        identity_retry = matches!(result, crate::protocol::WorkResult::Error { code, .. }
                            if matches!(code.as_str(), "rate_limited" | "network_unavailable")
                                || (identity_retry && !matches!(code.as_str(), "access_denied" | "account_changed")));
                    }
                    let identified = matches!(&event, BridgeEvent::Message(crate::protocol::ClientMessage::Result { result: crate::protocol::WorkResult::Session { .. }, .. }));
                    let result = app.bridge_event(event).await;
                    // A reconnect may resume only the owner and queue already approved by the user.
                    if !app.handle.is_empty() && app.owner != expected_owner {
                        automatic = false; app.pause().await?; app.log("X account changed. Stop the worker and confirm the account in the TUI.");
                    } else if identified && result.is_ok() && automatic && !app.handle.is_empty() && app.paused && (app.batch.is_some() || app.auto_policy.is_some()) && (app.uncertain == 0 || app.simple_running()) {
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
                    Control::Start => {
                        let result = app.begin_cleanup().await;
                        if result.is_ok() { automatic = true; app.detach_requested = false; }
                        result
                    },
                    Control::Pause => { automatic = false; app.save_managed(false).await?; app.pause().await },
                    Control::Resume => {
                        if app.owner != expected_owner || app.handle.is_empty() || (app.uncertain > 0 && !app.simple_running()) || !app.capabilities.iter().any(|c| c == "durable_queue:1") { app.log("Reconnect the approved account and resolve uncertain actions first."); Ok(()) }
                        else { automatic = true; app.save_managed(true).await?; if app.paused { app.key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE)).await } else { Ok(()) } }
                    },
                    Control::Cancel => { automatic = false; app.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)).await },
                    Control::Stop => { automatic = false; app.save_managed(false).await?; app.quit = true; app.pause().await },
                    Control::Settings { policy } => {
                        if !(5..=300).contains(&policy.delay_seconds) || !(1..=100).contains(&policy.rest_every) || policy.rest_seconds > 3600 || !(1..=500).contains(&policy.batch_limit) { app.log("Settings out of range"); Ok(()) }
                        else { app.policy.copy_pacing(&policy); app.save_processing().await }
                    },
                };
                let _ = reply.send(Status::from_app(&app));
                result
            },
            _ = tick.tick() => {
                if identity_retry && app.sender.is_some() && app.handle.is_empty() && app.pending.is_none() && app.pacing.until_ms <= now_ms() {
                    identity_retry = false; app.check_session().await
                } else { app.tick().await }
            },
            _ = tokio::signal::ctrl_c() => { app.quit = true; app.pause().await },
        };
        if let Err(error) = result {
            automatic = false;
            app.save_managed(false).await?;
            app.pause().await?;
            app.log(format!("{error:#}"));
        }
        // Fatal results pause within App without returning an error. Do not auto-resume those.
        if app.paused && !app.handle.is_empty() && app.pending.is_none() {
            automatic = false;
            app.save_managed(false).await?;
        }
        let should_stay_awake = app.policy.keep_awake
            && automatic
            && (app.auto_policy.is_some() || app.batch.is_some());
        if should_stay_awake && awake.is_none() && cfg!(target_os = "macos") {
            awake = std::process::Command::new("/usr/bin/caffeinate")
                .args(["-i", "-w", &std::process::id().to_string()])
                .spawn()
                .ok();
        } else if !should_stay_awake && let Some(mut child) = awake.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if app.quit {
            break;
        }
    }
    if let Some(mut child) = awake {
        let _ = child.kill();
        let _ = child.wait();
    }
    accept.abort();
    let _ = std::fs::remove_file(socket);
    Ok(())
}
