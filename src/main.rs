use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use crossterm::event::{Event, EventStream, KeyEventKind};
use forgive_me::{
    app::App,
    bridge::{self, BridgeEvent},
    config,
    model::{Account, now_ms},
    protocol::{ClientMessage, Command, Work, WorkResult},
    store::Store,
    ui,
};
use fs2::FileExt;
use futures_util::StreamExt;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{net::TcpListener, sync::mpsc};

#[derive(Parser)]
#[command(
    version,
    about = "A terminal-controlled X follower cleaner. X work stays in Chrome."
)]
struct Args {
    #[arg(long)]
    data_dir: Option<PathBuf>,
    #[arg(long)]
    port: Option<u16>,
    #[arg(
        long,
        help = "Explore the complete interface with fake accounts; no browser or X access"
    )]
    demo: bool,
    #[command(subcommand)]
    command: Option<CliCommand>,
}
#[derive(Subcommand)]
enum CliCommand {
    Pair {
        #[arg(long)]
        reset: bool,
    },
    Doctor,
}
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let (events_tx, events_rx) = mpsc::channel(64);
    if args.demo {
        let app = App::new(Store::open(Path::new(":memory:"))?, true)?;
        let records = demo_accounts();
        let copy = records.clone();
        app.store
            .run(move |s| s.save_page("demo", &copy, "demo", &true))
            .await?;
        let (tx, rx) = mpsc::channel(8);
        events_tx
            .send(BridgeEvent::Connected {
                session_id: "demo".into(),
                sender: tx,
            })
            .await?;
        let worker = tokio::spawn(demo_worker(rx, events_tx, records));
        let result = run(app, events_rx).await;
        worker.abort();
        return result;
    }
    let dir = config::data_dir(args.data_dir)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join("app.lock"))?;
    lock.try_lock_exclusive()
        .context("forgive-me is already running for this data directory")?;
    let mut config = config::load(&dir)?;
    if let Some(port) = args.port {
        config.port = port;
        config::save(&dir, &config)?;
    }
    match args.command {
        Some(CliCommand::Pair { reset }) => {
            if reset {
                config.token = forgive_me::model::new_id();
                config.extension_id = None;
                config::save(&dir, &config)?;
            }
            println!(
                "forgive-me pairing\n\nPort: {}\nPairing secret: {}\n\nPaste these into the extension options, then run forgive-me.\nKeep the secret private.\nData: {}",
                config.port,
                config.token,
                dir.display()
            );
            return Ok(());
        }
        Some(CliCommand::Doctor) => {
            let s = Store::open(&dir.join("cleanup.sqlite"))?;
            let owner: String = s.get("last_owner")?.unwrap_or_default();
            println!(
                "forgive-me {}\nData: {}\nBridge: ws://127.0.0.1:{}/bridge\nPaired extension: {}\nLast owner: {}\nUnresolved actions: {}\n\nRun the TUI and connect Chrome to check live adapter readiness.",
                env!("CARGO_PKG_VERSION"),
                dir.display(),
                config.port,
                config.extension_id.as_deref().unwrap_or("not paired"),
                if owner.is_empty() { "none" } else { &owner },
                s.unresolved(&owner)?.len()
            );
            return Ok(());
        }
        None => {}
    }
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, config.port))
        .await
        .context("Local bridge port unavailable; use --port to choose another")?;
    let app = App::new(Store::open(&dir.join("cleanup.sqlite"))?, false)?;
    let bridge = tokio::spawn(bridge::serve(listener, config, dir, events_tx));
    let result = run(app, events_rx).await;
    bridge.abort();
    result
}
async fn run(mut app: App, mut bridge: mpsc::Receiver<BridgeEvent>) -> Result<()> {
    let mut terminal = ratatui::init();
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            ratatui::restore();
        }
    }
    let _restore = Restore;
    let mut keys = EventStream::new();
    let mut clock = tokio::time::interval(Duration::from_millis(100));
    terminal.draw(|f| ui::render(f, &app))?;
    let mut refresh = tokio::time::Instant::now();
    while !app.quit {
        let mut redraw = true;
        let result = tokio::select! {
         event=keys.next()=>match event{Some(Ok(Event::Key(key)))if key.kind==KeyEventKind::Press=>app.key(key).await,Some(Ok(_))=>Ok(()),Some(Err(e))=>Err(e.into()),None=>{app.quit=true;Ok(())}},
         event=bridge.recv()=>match event{Some(event)=>app.bridge_event(event).await,None=>{app.quit=true;Ok(())}},
         _=clock.tick()=>{
            if app.paused || app.pending.is_some() || app.sender.is_none() || std::time::Instant::now() < app.next_at { redraw = false; }
            app.tick().await
         },
        };
        if let Err(e) = result {
            redraw = true;
            app.paused = true;
            app.log(format!("{e:#}"));
        }
        if redraw || refresh.elapsed() >= Duration::from_secs(30) {
            terminal.draw(|f| ui::render(f, &app))?;
            refresh = tokio::time::Instant::now();
        }
    }
    Ok(())
}
fn demo_accounts() -> Vec<Account> {
    let now = now_ms();
    let specs = [
        ("quiet_orbit", Some(false), Some(false), Some(8), Some(240)),
        ("empty_echo", Some(false), Some(false), Some(0), None),
        (
            "actual_friend",
            Some(false),
            Some(true),
            Some(42),
            Some(300),
        ),
        ("blue_example", Some(true), Some(false), Some(22), Some(500)),
        ("active_reader", Some(false), Some(false), Some(20), Some(2)),
        ("unknown_activity", Some(false), Some(false), Some(3), None),
    ];
    specs
        .into_iter()
        .enumerate()
        .map(|(i, (name, verified, i_follow, posts, days))| Account {
            id: format!("{}", 1000 + i),
            handle: name.into(),
            name: name.replace('_', " "),
            bio: "Demonstration account. No real X account is represented.".into(),
            followers: Some(12 + i as u64),
            following_count: Some(1400),
            posts,
            verified,
            protected: Some(false),
            follows_me: Some(true),
            i_follow,
            last_activity_ms: days.map(|d| now - d * 86_400_000),
            coverage_since_ms: days.map(|_| now - 365 * 86_400_000),
            checked_at_ms: Some(now),
            observed_at_ms: now,
            activity_note: if days.is_none() && posts != Some(0) {
                "Lookup failed; kept unknown.".into()
            } else {
                "Simulated posting evidence".into()
            },
            kept: false,
        })
        .collect()
}
async fn demo_worker(
    mut rx: mpsc::Receiver<Value>,
    tx: mpsc::Sender<BridgeEvent>,
    mut accounts: Vec<Account>,
) {
    while let Some(v) = rx.recv().await {
        if v["type"] != "command" {
            continue;
        }
        let Ok(w) = serde_json::from_value::<Work>(v["work"].clone()) else {
            continue;
        };
        let result = match &w.command {
            Command::GetSession => WorkResult::Session {
                owner_id: "demo".into(),
                handle: "demo_account".into(),
                capabilities: vec!["remove_follower".into()],
            },
            Command::ScanPage { list, .. } => WorkResult::Page {
                list: list.clone(),
                accounts: accounts
                    .iter()
                    .filter(|a| {
                        if list == "following" {
                            a.i_follow == Some(true)
                        } else {
                            a.follows_me == Some(true)
                        }
                    })
                    .cloned()
                    .collect(),
                next_cursor: None,
                complete: true,
            },
            Command::InspectAccount { target_id, .. } => {
                let mut a = accounts
                    .iter()
                    .find(|a| &a.id == target_id)
                    .unwrap()
                    .clone();
                a.checked_at_ms = Some(now_ms());
                WorkResult::Account { account: a }
            }
            Command::RemoveFollower { target_id, .. } => {
                accounts
                    .iter_mut()
                    .find(|a| &a.id == target_id)
                    .unwrap()
                    .follows_me = Some(false);
                WorkResult::Action {
                    target_id: target_id.clone(),
                    status: "verified_removed".into(),
                    message: "Simulated removal — no X request was sent".into(),
                }
            }
            Command::Reconcile { target_id, .. } => WorkResult::Action {
                target_id: target_id.clone(),
                status: "already_absent".into(),
                message: "Simulated reconciliation".into(),
            },
            Command::OpenProfile { .. } => WorkResult::Opened,
        };
        tokio::time::sleep(Duration::from_millis(120)).await;
        if tx
            .send(BridgeEvent::Message(ClientMessage::Result {
                session_id: "demo".into(),
                command_id: w.command_id,
                result,
            }))
            .await
            .is_err()
        {
            break;
        }
    }
}
