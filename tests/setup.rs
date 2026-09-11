use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use std::path::Path;
use tokio::sync::mpsc;
use x_bot_follower_remover::{
    app::{App, Mode},
    bridge::BridgeEvent,
    config::Config,
    protocol::{ClientMessage, WorkResult},
    setup::Step,
    store::Store,
    ui,
};

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
fn screen(app: &mut App, width: u16, height: u16) -> String {
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
    press(&mut app, KeyCode::Right).await;
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
            capabilities: vec!["adapter:2".into()],
        },
    )
    .await;
    assert!(app.setup.as_ref().unwrap().step == Step::Ready);
    assert!(screen(&mut app, 94, 26).contains("Connected as @example"));
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
    press(&mut app, KeyCode::Char('m')).await;
    press(&mut app, KeyCode::Char('v')).await;
    let token = app.setup.as_ref().unwrap().token.clone();
    assert!(screen(&mut app, 94, 26).contains(&token));
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
            capabilities: vec!["adapter:2".into()],
        },
    )
    .await;
    app.bridge_event(BridgeEvent::Disconnected("Chrome closed".into()))
        .await
        .unwrap();
    press(&mut app, KeyCode::Right).await;
    assert_eq!(app.mode, Mode::Setup);
    assert!(app.setup.as_ref().unwrap().step == Step::Pair);
    assert!(!screen(&mut app, 94, 26).contains("Connected as @example"));
}

#[test]
fn saved_pairing_never_skips_live_connection_onboarding() {
    let store = Store::open(Path::new(":memory:")).unwrap();
    store.set("last_owner", &"1").unwrap();
    let mut app = App::new(store, false).unwrap();
    let mut config = Config {
        extension_id: Some("extension".into()),
        ..Config::default()
    };
    app.configure_setup(&config);
    assert_eq!(app.mode, Mode::Setup);
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
            assert!(!screen(&mut app, w, h).contains(&token));
        }
    }
    app.mode = Mode::Browse;
    app.paused = false;
    press(&mut app, KeyCode::Char('P')).await;
    assert!(app.paused);
    assert_eq!(app.mode, Mode::Setup);
}

#[tokio::test]
async fn navigation_cannot_skip_pairing_and_display_uses_live_connection() {
    let mut app = app();
    for _ in 0..4 {
        press(&mut app, KeyCode::Right).await;
    }
    assert!(app.setup.as_ref().unwrap().step == Step::Install);
    // Even stale saved UI state cannot claim a disconnected account is ready.
    app.setup.as_mut().unwrap().go(Step::Ready);
    app.handle = "stale_account".into();
    let text = screen(&mut app, 94, 26);
    assert!(text.contains("Chrome not connected"));
    assert!(text.contains("Enter  Open extension"));
    assert!(!text.contains("Connected as @stale_account"));
    for (w, h) in [(52, 12), (94, 26), (140, 42)] {
        let text = screen(&mut app, w, h);
        assert!(text.contains("Enter  Open extension"));
        assert!(text.contains("i install / reload"));
    }
}

#[tokio::test]
async fn enter_waits_during_identity_check_and_setup_reopens_at_live_state() {
    let mut app = app();
    let (tx, mut rx) = mpsc::channel(16);
    app.bridge_event(BridgeEvent::Connected {
        session_id: "browser".into(),
        sender: tx,
    })
    .await
    .unwrap();
    let first = rx.recv().await.unwrap();
    assert_eq!(first["work"]["command"]["kind"], "get_session");
    press(&mut app, KeyCode::Enter).await;
    assert_eq!(app.mode, Mode::Setup);
    assert!(rx.try_recv().is_err());
    assert!(screen(&mut app, 94, 26).contains("Checking X"));
    result(
        &mut app,
        WorkResult::Session {
            owner_id: "1".into(),
            handle: "example".into(),
            capabilities: vec!["adapter:2".into()],
        },
    )
    .await;
    press(&mut app, KeyCode::Enter).await;
    press(&mut app, KeyCode::Char('P')).await;
    assert!(app.setup.as_ref().unwrap().step == Step::Ready);
    assert!(screen(&mut app, 94, 26).contains("Enter  Use this account"));
    assert!(app.paused && app.batch.is_none());
}

#[tokio::test]
async fn wizard_scroll_clamps_and_recovers_after_resize() {
    let mut app = app();
    app.setup.as_mut().unwrap().go(Step::Pair);
    app.setup.as_mut().unwrap().scroll = 40;
    let text = screen(&mut app, 52, 12);
    let end = app.setup.as_ref().unwrap().scroll;
    assert!(end > 0 && end < 40);
    assert!(text.contains("scroll"));
    press(&mut app, KeyCode::Up).await;
    screen(&mut app, 52, 12);
    assert_eq!(app.setup.as_ref().unwrap().scroll, end - 1);
    screen(&mut app, 140, 42);
    assert_eq!(app.setup.as_ref().unwrap().scroll, 0);
}

async fn page_ready(app: &mut App) {
    app.bridge_event(BridgeEvent::Message(ClientMessage::XPageReady {
        session_id: "browser".into(),
    }))
    .await
    .unwrap();
}

#[tokio::test]
async fn page_load_during_identity_check_does_not_cancel_and_retries_only_once() {
    let mut app = app();
    let (tx, mut rx) = mpsc::channel(16);
    app.bridge_event(BridgeEvent::Connected {
        session_id: "browser".into(),
        sender: tx,
    })
    .await
    .unwrap();
    let initial = app.pending.as_ref().unwrap().command_id.clone();
    rx.recv().await.unwrap();
    page_ready(&mut app).await;
    page_ready(&mut app).await;
    assert_eq!(app.pending.as_ref().unwrap().command_id, initial);
    assert!(rx.try_recv().is_err());
    result(
        &mut app,
        WorkResult::Error {
            code: "login_required".into(),
            message: "Sign in".into(),
            retry_at_ms: None,
        },
    )
    .await;
    assert_eq!(rx.recv().await.unwrap()["type"], "ack");
    assert_eq!(
        rx.recv().await.unwrap()["work"]["command"]["kind"],
        "get_session"
    );
    assert_ne!(app.pending.as_ref().unwrap().command_id, initial);
    result(
        &mut app,
        WorkResult::Error {
            code: "signing_unavailable".into(),
            message: "Signing failed".into(),
            retry_at_ms: None,
        },
    )
    .await;
    assert_eq!(rx.recv().await.unwrap()["type"], "ack");
    assert!(rx.try_recv().is_err());
    assert!(app.sender.is_some() && app.pending.is_none() && app.paused);
    assert!(screen(&mut app, 94, 26).contains("signing_unavailable: Signing failed"));
    press(&mut app, KeyCode::Enter).await;
    assert_eq!(
        rx.recv().await.unwrap()["work"]["command"]["kind"],
        "get_session"
    );
}

#[tokio::test]
async fn page_readiness_never_retries_success_or_rate_limits_or_starts_cleanup() {
    for limited in [false, true] {
        let mut app = app();
        let (tx, mut rx) = mpsc::channel(16);
        app.bridge_event(BridgeEvent::Connected {
            session_id: "browser".into(),
            sender: tx,
        })
        .await
        .unwrap();
        rx.recv().await.unwrap();
        page_ready(&mut app).await;
        result(
            &mut app,
            if limited {
                WorkResult::Error {
                    code: "rate_limited".into(),
                    message: "Wait".into(),
                    retry_at_ms: Some(x_bot_follower_remover::model::now_ms() + 60_000),
                }
            } else {
                WorkResult::Session {
                    owner_id: "1".into(),
                    handle: "example".into(),
                    capabilities: vec!["adapter:2".into()],
                }
            },
        )
        .await;
        rx.recv().await.unwrap();
        page_ready(&mut app).await;
        assert!(app.pending.is_none() && app.paused && rx.try_recv().is_err());
        app.mode = Mode::Browse;
        app.handle.clear();
        page_ready(&mut app).await;
        assert!(app.pending.is_none() && rx.try_recv().is_err());
    }
}

#[tokio::test]
async fn reconnect_preserves_identity_rate_limit_without_another_request() {
    let mut app = app();
    let (tx, mut rx) = mpsc::channel(16);
    app.bridge_event(BridgeEvent::Connected {
        session_id: "browser".into(),
        sender: tx,
    })
    .await
    .unwrap();
    rx.recv().await.unwrap();
    result(
        &mut app,
        WorkResult::Error {
            code: "rate_limited".into(),
            message: "Wait".into(),
            retry_at_ms: Some(x_bot_follower_remover::model::now_ms() + 60_000),
        },
    )
    .await;
    rx.recv().await.unwrap();
    app.bridge_event(BridgeEvent::Disconnected("test reconnect".into()))
        .await
        .unwrap();
    let (tx, mut rx) = mpsc::channel(16);
    let error = app
        .bridge_event(BridgeEvent::Connected {
            session_id: "new-browser".into(),
            sender: tx,
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("cooldown"));
    assert!(screen(&mut app, 94, 26).contains("rate_limited: Wait"));
    page_ready(&mut app).await;
    assert!(app.sender.is_some() && app.pending.is_none());
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn simple_start_stays_in_onboarding_until_live_identity_is_confirmed() {
    let mut app = app();
    app.policy = x_bot_follower_remover::model::Policy::cleanup();
    app.owner = "saved-owner".into();
    app.setup.as_mut().unwrap().installation =
        x_bot_follower_remover::setup::Installation::LegacyOnly;
    let text = screen(&mut app, 94, 30);
    assert!(text.contains("Only the old extension"));
    assert!(text.contains("Set up Chrome"));
    assert!(!text.contains("Start cleanup"));
    assert!(!text.contains("press s to scan"));
    let (tx, mut rx) = mpsc::channel(16);
    app.bridge_event(BridgeEvent::Connected {
        session_id: "browser".into(),
        sender: tx,
    })
    .await
    .unwrap();
    rx.recv().await.unwrap();
    result(
        &mut app,
        WorkResult::Session {
            owner_id: "1".into(),
            handle: "example".into(),
            capabilities: vec!["adapter:2".into(), "simple_cleanup:1".into()],
        },
    )
    .await;
    press(&mut app, KeyCode::Enter).await;
    assert_eq!(app.mode, Mode::Browse);
    assert!(app.paused && app.auto_policy.is_none() && app.pending.is_none());
    assert!(screen(&mut app, 94, 30).contains("Enter Start cleanup"));
    app.bridge_event(BridgeEvent::Disconnected("Chrome closed".into()))
        .await
        .unwrap();
    assert_eq!(app.mode, Mode::Setup);
    assert!(!screen(&mut app, 94, 30).contains("Start cleanup"));
}

#[test]
fn chrome_registration_detection_distinguishes_missing_legacy_installed_and_unreadable() {
    use x_bot_follower_remover::setup::{Installation, detect_installation};
    let tmp = tempfile::tempdir().unwrap();
    let extension = tmp.path().join("extension");
    std::fs::create_dir(&extension).unwrap();
    let root = tmp.path().join("chrome");
    assert_eq!(
        detect_installation(&root, &extension),
        Installation::Unknown
    );
    let profile = root.join("Profile 2");
    std::fs::create_dir_all(&profile).unwrap();
    let file = profile.join("Secure Preferences");
    std::fs::write(&file, "{}").unwrap();
    assert_eq!(
        detect_installation(&root, &extension),
        Installation::NotFound
    );
    std::fs::write(
        &file,
        r#"{"extensions":{"settings":{"old":{"path":"/old/forgive-me-extension"}}}}"#,
    )
    .unwrap();
    assert_eq!(
        detect_installation(&root, &extension),
        Installation::LegacyOnly
    );
    let id = x_bot_follower_remover::native::extension_id(&extension).unwrap();
    std::fs::write(&file, serde_json::to_vec(&serde_json::json!({"extensions":{"settings":{id:{"path":extension,"disable_reasons":[1]}}}})).unwrap()).unwrap();
    assert_eq!(
        detect_installation(&root, &extension),
        Installation::Found("Profile 2".into())
    );
    std::fs::write(profile.join("Preferences"), "{}").unwrap();
    std::fs::write(&file, "incomplete write").unwrap();
    assert_eq!(
        detect_installation(&root, &extension),
        Installation::Unknown
    );
}

#[test]
fn renamed_bundle_repairs_only_the_known_old_pairing_once() {
    use x_bot_follower_remover::{config, native};
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("old");
    let new = tmp.path().join("new");
    std::fs::create_dir(&old).unwrap();
    std::fs::create_dir(&new).unwrap();
    let mut config = Config {
        extension_id: Some(native::extension_id(&old).unwrap()),
        ..Default::default()
    };
    let token = config.token.clone();
    let old = old.canonicalize().unwrap();
    std::fs::remove_dir(&old).unwrap();
    assert!(native::migrate_legacy_pairing(tmp.path(), &mut config, &old, &new).unwrap());
    assert_eq!(
        config.extension_id,
        Some(native::extension_id(&new).unwrap())
    );
    assert_ne!(config.token, token);
    assert_eq!(config::load(tmp.path()).unwrap().token, config.token);
    assert!(!native::migrate_legacy_pairing(tmp.path(), &mut config, &old, &new).unwrap());
    config.extension_id = Some("manual-extension".into());
    let manual = config.token.clone();
    assert!(!native::migrate_legacy_pairing(tmp.path(), &mut config, &old, &new).unwrap());
    assert_eq!(config.token, manual);
}
