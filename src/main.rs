use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEvent,
        KeyEventKind, KeyModifiers, MouseEventKind,
    },
    execute,
};
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
    io::{IsTerminal, stdout},
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
    #[arg(
        long,
        help = "Show a still cleanup illustration instead of moving water"
    )]
    no_animation: bool,
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
    /// Show or control the detached removal queue.
    Status,
    Pause,
    Resume,
    Stop,
    #[command(hide = true)]
    Worker,
    #[command(hide = true)]
    NativeHost {
        origin: String,
    },
    #[command(hide = true)]
    UnregisterHost,
}
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if let Some(CliCommand::NativeHost { origin }) = &args.command {
        let dir = config::data_dir(args.data_dir.clone())?;
        let extension = forgive_me::setup::extension_dir();
        return forgive_me::native::serve(
            &dir,
            &extension,
            origin,
            std::io::stdin().lock(),
            std::io::stdout().lock(),
        );
    }
    if matches!(args.command, Some(CliCommand::UnregisterHost)) {
        return forgive_me::native::unregister();
    }
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
        let result = run(app, events_rx, args.no_animation).await;
        worker.abort();
        return result.map(|_| ());
    }
    let dir = config::data_dir(args.data_dir)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join("app.lock"))?;
    let control = match args.command {
        Some(CliCommand::Status) => Some(forgive_me::background::Control::Status),
        Some(CliCommand::Pause) => Some(forgive_me::background::Control::Pause),
        Some(CliCommand::Resume) => Some(forgive_me::background::Control::Resume),
        Some(CliCommand::Stop) => Some(forgive_me::background::Control::Stop),
        _ => None,
    };
    if let Some(control) = control {
        let status = forgive_me::background::request(&dir, control).await?;
        println!(
            "@{} · {} · {} queued · {} removed · {} uncertain · next in {}s\n{}",
            status.handle,
            status.state,
            status.remaining,
            status.removed,
            status.uncertain,
            status.wait_seconds,
            status.message
        );
        return Ok(());
    }
    if lock.try_lock_exclusive().is_err() {
        if args.command.is_none() && dir.join("worker.sock").exists() {
            return monitor(&dir).await;
        }
        anyhow::bail!("forgive-me is already running for this data directory");
    }
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
        None | Some(CliCommand::Worker) => {}
        Some(
            CliCommand::NativeHost { .. }
            | CliCommand::UnregisterHost
            | CliCommand::Status
            | CliCommand::Pause
            | CliCommand::Resume
            | CliCommand::Stop,
        ) => unreachable!(),
    }
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, config.port))
        .await
        .context("Local bridge port unavailable; use --port to choose another")?;
    let mut app = App::new(Store::open(&dir.join("cleanup.sqlite"))?, false)?;
    let background = matches!(args.command, Some(CliCommand::Worker));
    if !background {
        app.configure_setup(&config);
    }
    if !background
        && let Err(error) = forgive_me::native::register(&dir, &forgive_me::setup::extension_dir())
    {
        app.log(format!(
            "Automatic pairing unavailable: {error}. Manual pairing remains available."
        ));
    }
    let bridge = tokio::spawn(bridge::serve(listener, config, dir.clone(), events_tx));
    let result = if background {
        forgive_me::background::run(app, events_rx, &dir)
            .await
            .map(|_| false)
    } else {
        run(app, events_rx, args.no_animation).await
    };
    bridge.abort();
    let _ = bridge.await;
    drop(lock);
    if result? {
        forgive_me::background::spawn(&dir)?;
        println!(
            "Approved queue moved to background. Keep Chrome open and your Mac awake.\nRun forgive-me to monitor; forgive-me pause or forgive-me stop to stop work."
        );
    }
    Ok(())
}
async fn run(
    mut app: App,
    mut bridge: mpsc::Receiver<BridgeEvent>,
    no_animation: bool,
) -> Result<bool> {
    anyhow::ensure!(
        std::io::stdin().is_terminal() && stdout().is_terminal(),
        "Open forgive-me in an interactive terminal (no pipes or output redirection)."
    );
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = execute!(stdout(), DisableMouseCapture);
            ratatui::restore();
        }
    }
    let _restore = Restore;
    // Ratatui owns the full alternate screen and raw mode. Capture wheel events
    // as navigation so terminal emulators do not scroll the shell behind it.
    let mut terminal = ratatui::try_init()?;
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(stdout(), DisableMouseCapture);
        previous_hook(info);
    }));
    execute!(stdout(), EnableMouseCapture)?;
    terminal.clear()?;
    let mut keys = EventStream::new();
    let mut clock = tokio::time::interval(Duration::from_millis(50));
    clock.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut animation_clock = std::time::Instant::now();
    terminal.draw(|f| ui::render(f, &mut app, 0))?;
    let mut refresh = tokio::time::Instant::now();
    while !app.quit {
        if app.detach_requested && app.pending.is_none() {
            return Ok(true);
        }
        let mut redraw = true;
        let result = tokio::select! {
         event = keys.next() => match event {
            Some(Ok(Event::Resize(_, _))) => Ok(()),
            Some(Ok(event)) => match navigation_key(event) {
                Some(key) => app.key(key).await,
                None => { redraw = false; Ok(()) },
            },
            Some(Err(e)) => Err(e.into()),
            None => { app.quit = true; Ok(()) },
         },
         event=bridge.recv()=>match event{Some(event)=>app.bridge_event(event).await,None=>{app.quit=true;Ok(())}},
         _=clock.tick()=>{
            let elapsed = animation_clock.elapsed();
            animation_clock = std::time::Instant::now();
            if !no_animation && (app.batch.is_some() || app.auto_policy.is_some()) && !app.paused && app.sender.is_some() {
                app.animation.advance(elapsed);
            }
            if (no_animation || !forgive_me::ritual::animating(&app)) && (app.paused || app.pending.is_some() || app.sender.is_none() || std::time::Instant::now() < app.next_at || app.pacing.until_ms > now_ms()) { redraw = false; }
            app.tick().await
         },
        };
        if let Err(e) = result {
            redraw = true;
            app.paused = true;
            app.log(format!("{e:#}"));
        }
        if redraw || refresh.elapsed() >= Duration::from_secs(1) {
            let animation_tick = app.animation.clock_ms / 50;
            terminal
                .draw(|f| ui::render(f, &mut app, if no_animation { 0 } else { animation_tick }))?;
            refresh = tokio::time::Instant::now();
        }
    }
    Ok(false)
}
// Mouse actions only navigate. They can never select or approve a removal.
fn navigation_key(event: Event) -> Option<KeyEvent> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => Some(key),
        Event::Mouse(mouse) => match mouse.kind {
            MouseEventKind::ScrollDown => Some(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            MouseEventKind::ScrollUp => Some(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            _ => None,
        },
        _ => None,
    }
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

async fn monitor(dir: &Path) -> Result<()> {
    use forgive_me::background::{Control, request};
    use forgive_me::theme::*;
    use ratatui::{
        text::{Line, Span},
        widgets::{Block, Borders, Paragraph, Wrap},
    };
    anyhow::ensure!(
        std::io::stdin().is_terminal() && stdout().is_terminal(),
        "Use forgive-me status for noninteractive output"
    );
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            ratatui::restore();
        }
    }
    let _restore = Restore;
    let mut terminal = ratatui::try_init()?;
    let mut keys = EventStream::new();
    let mut timer = tokio::time::interval(Duration::from_secs(1));
    loop {
        let state = request(dir, Control::Status).await?;
        terminal.draw(|frame| {
            let area = frame.area();
            let lines = vec![
                Line::styled(" ◈ forgive-me / BACKGROUND QUEUE", bold(MINT)),
                Line::from(format!(" @{}  ·  {}", state.handle, state.state)),
                Line::from(""),
                Line::from(format!(
                    " {} remaining  ·  {} verified removals  ·  {} uncertain",
                    state.remaining, state.removed, state.uncertain
                )),
                Line::from(format!(" Next task in {} seconds", state.wait_seconds)),
                Line::from(""),
                Line::from(state.message.clone()),
                Line::from(""),
                Line::styled(
                    " Chrome must stay open. Your Mac must stay awake.",
                    fg(MUTED),
                ),
                Line::from(" Closing this view leaves the approved queue running."),
                Line::from(""),
                Line::from(vec![
                    Span::styled(" p ", bold(ICE)),
                    Span::raw("pause   "),
                    Span::styled(" r ", bold(ICE)),
                    Span::raw("resume   "),
                    Span::styled(" c ", bold(ICE)),
                    Span::raw("cancel queue"),
                ]),
                Line::from(" x stop worker and save queue    q close monitor"),
            ];
            frame.render_widget(
                Paragraph::new(lines)
                    .block(Block::default().borders(Borders::ALL))
                    .style(fg(TEXT).bg(BG))
                    .wrap(Wrap { trim: false }),
                area,
            );
        })?;
        tokio::select! {
            _ = timer.tick() => {},
            event = keys.next() => if let Some(Ok(Event::Key(key))) = event {
                if key.kind != KeyEventKind::Press { continue; }
                let command = match key.code { KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('p') => Some(Control::Pause), KeyCode::Char('r') => Some(Control::Resume),
                    KeyCode::Char('c') => Some(Control::Cancel), KeyCode::Char('x') => Some(Control::Stop), _ => None };
                if let Some(command) = command { let stop = matches!(command, Control::Stop); request(dir, command).await?; if stop { break; } }
            } else if event.is_none() { break; },
        }
    }
    Ok(())
}
