use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use forgive_me::{
    app::{App, Mode},
    bridge::BridgeEvent,
    config::Config,
    protocol::{ClientMessage, WorkResult},
    setup::Step,
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend};
use std::path::Path;
use tokio::sync::mpsc;

fn app() -> App {
    let mut app = App::new(Store::open(Path::new(":memory:")).unwrap(), false).unwrap();
    app.configure_setup(&Config::default());
    app
}
async fn press(app: &mut App, code: KeyCode) {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
        .await
        .unwrap();
}
async fn result(app: &mut App, result: WorkResult) {
    let command_id = app.pending.as_ref().unwrap().command_id.clone();
    app.bridge_event(BridgeEvent::Message(ClientMessage::Result {
        session_id: "browser".into(),
        command_id,
        result,
    }))
    .await
    .unwrap();
}
fn screen(app: &App, width: u16, height: u16) -> String {
    let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
    term.draw(|f| ui::render(f, app, 0)).unwrap();
    term.backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect()
}

#[tokio::test]
async fn setup_waits_for_identity_and_never_resumes_work() {
    let mut app = app();
    assert_eq!(app.mode, Mode::Setup);
    let (tx, mut rx) = mpsc::channel(16);
    app.bridge_event(BridgeEvent::Connected {
        session_id: "browser".into(),
        sender: tx,
    })
    .await
    .unwrap();
    assert!(app.setup.as_ref().unwrap().step == Step::Connect);
    assert_eq!(
        rx.recv().await.unwrap()["work"]["command"]["kind"],
        "get_session"
    );
    result(
        &mut app,
        WorkResult::Error {
            code: "auth".into(),
            message: "Sign in to X".into(),
            retry_at_ms: None,
        },
    )
    .await;
    assert!(app.setup.as_ref().unwrap().step == Step::Connect);
    assert_eq!(rx.recv().await.unwrap()["type"], "ack");
    for c in ['p', 's', 'a', 'd', 'y'] {
        press(&mut app, KeyCode::Char(c)).await;
    }
    press(&mut app, KeyCode::Enter).await;
    app.tick().await.unwrap();
    assert!(app.paused && app.batch.is_none() && app.pending.is_none());
    assert!(rx.try_recv().is_err());
    press(&mut app, KeyCode::Char('r')).await;
    assert_eq!(
        rx.recv().await.unwrap()["work"]["command"]["kind"],
        "get_session"
    );
    result(
        &mut app,
        WorkResult::Session {
            owner_id: "1".into(),
            handle: "example".into(),
            capabilities: vec![],
        },
    )
    .await;
    assert!(app.setup.as_ref().unwrap().step == Step::Ready);
    assert!(screen(&app, 94, 26).contains("Connected as @example"));
    press(&mut app, KeyCode::Enter).await;
    assert_eq!(app.mode, Mode::Browse);
    assert!(app.paused);
    assert_eq!(
        app.store
            .run(|s| s.get::<String>("last_owner"))
            .await
            .unwrap()
            .as_deref(),
        Some("1")
    );
}

#[tokio::test]
async fn reconnect_hides_secret_and_disconnect_cannot_leave_ready() {
    let mut app = app();
    press(&mut app, KeyCode::Enter).await;
    press(&mut app, KeyCode::Char('v')).await;
    let token = app.setup.as_ref().unwrap().token.clone();
    assert!(screen(&app, 94, 26).contains(&token));
    let (tx, _rx) = mpsc::channel(16);
    app.bridge_event(BridgeEvent::Connected {
        session_id: "browser".into(),
        sender: tx,
    })
    .await
    .unwrap();
    assert!(!app.setup.as_ref().unwrap().revealed);
    result(
        &mut app,
        WorkResult::Session {
            owner_id: "1".into(),
            handle: "example".into(),
            capabilities: vec![],
        },
    )
    .await;
    app.bridge_event(BridgeEvent::Disconnected("Chrome closed".into()))
        .await
        .unwrap();
    press(&mut app, KeyCode::Enter).await;
    assert_eq!(app.mode, Mode::Setup);
    assert!(app.setup.as_ref().unwrap().step == Step::Connect);
    assert!(!screen(&app, 94, 26).contains("Connected as @example"));
}

#[test]
fn returning_users_skip_setup_but_reset_or_incomplete_pairing_does_not() {
    let store = Store::open(Path::new(":memory:")).unwrap();
    store.set("last_owner", &"1").unwrap();
    let mut app = App::new(store, false).unwrap();
    let mut config = Config {
        extension_id: Some("extension".into()),
        ..Config::default()
    };
    app.configure_setup(&config);
    assert_eq!(app.mode, Mode::Browse);
    config.extension_id = None;
    app.configure_setup(&config);
    assert_eq!(app.mode, Mode::Setup);
    let mut fresh = App::new(Store::open(Path::new(":memory:")).unwrap(), false).unwrap();
    config.extension_id = Some("extension".into());
    fresh.configure_setup(&config);
    assert_eq!(fresh.mode, Mode::Setup);
}

#[tokio::test]
async fn instructions_render_at_supported_sizes_and_secret_is_opt_in() {
    let mut app = app();
    let token = app.setup.as_ref().unwrap().token.clone();
    for step in [Step::Install, Step::Pair, Step::Connect, Step::Ready] {
        app.setup.as_mut().unwrap().go(step);
        for (w, h) in [(94, 26), (52, 12), (30, 8)] {
            assert!(!screen(&app, w, h).contains(&token));
        }
    }
    app.mode = Mode::Browse;
    app.paused = false;
    press(&mut app, KeyCode::Char('P')).await;
    assert!(app.paused);
    assert_eq!(app.mode, Mode::Setup);
}
