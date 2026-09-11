//! Regressions for visible safety controls and bounded terminal navigation.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use forgive_me::{
    app::{App, Batch, Mode},
    model::Account,
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend};
use std::{collections::VecDeque, path::Path};

fn app() -> App {
    let mut app = App::new(Store::open(Path::new(":memory:")).unwrap(), true).unwrap();
    app.handle = "demo_account".into();
    app
}
fn screen(app: &mut App, w: u16, h: u16, tick: u64) -> String {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| ui::render(f, app, tick)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect()
}
#[test]
fn confirmation_warning_and_cancel_stay_visible_at_supported_sizes() {
    let mut app = app();
    app.mode = Mode::Confirm;
    for (w, h) in [(52, 12), (80, 24), (140, 42)] {
        let screen = screen(&mut app, w, h, 0);
        assert!(screen.contains("There is no restore-followers action."));
        assert!(screen.contains("Enter / n / Esc cancels"));
        assert!(screen.contains("y confirms"));
    }
}
#[tokio::test]
async fn help_scroll_clamps_to_content_and_can_immediately_move_back() {
    let mut app = app();
    app.mode = Mode::Help;
    app.modal_scroll = u16::MAX;
    let bottom = screen(&mut app, 52, 12, 0);
    assert!(bottom.contains("Pause, save and quit"));
    assert!(app.modal_scroll > 0 && app.modal_scroll < 100);
    let previous = app.modal_scroll;
    app.key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))
        .await
        .unwrap();
    assert_eq!(app.modal_scroll, previous - 1);
    assert_ne!(bottom, screen(&mut app, 52, 12, 0));
    screen(&mut app, 140, 42, 0);
    assert_eq!(app.modal_scroll, 0);
}
#[test]
fn unknown_evidence_is_explicit_and_search_empty_is_distinct() {
    let mut app = app();
    app.accounts.insert(
        "1".into(),
        Account {
            id: "1".into(),
            handle: "unknown".into(),
            follows_me: Some(true),
            ..Default::default()
        },
    );
    app.mode = Mode::Details;
    let text = screen(&mut app, 120, 34, 0);
    assert!(text.contains("Unknown"));
    assert!(!text.contains("Some(") && !text.contains("None"));
    app.mode = Mode::Browse;
    app.query = "no match".into();
    assert!(screen(&mut app, 120, 34, 0).contains("No accounts in this view"));
}
#[test]
fn ritual_is_still_when_paused_and_reports_real_count_even_when_small() {
    let mut app = app();
    app.show_queue_list = false;
    app.batch = Some(Batch {
        id: "demo".into(),
        ids: VecDeque::new(),
        policy: app.policy.clone(),
    });
    app.removed = 7;
    let paused = screen(&mut app, 140, 42, 0);
    assert_eq!(paused, screen(&mut app, 140, 42, 9));
    assert!(paused.contains("Reseting followers: 7"));
    assert!(screen(&mut app, 52, 12, 0).contains("Reseting followers: 7"));
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    app.sender = Some(tx);
    app.paused = false;
    assert_ne!(screen(&mut app, 140, 42, 0), screen(&mut app, 140, 42, 3));
}
#[test]
fn all_modes_render_across_resize_boundaries() {
    let mut app = app();
    for (w, h) in [
        (30, 8),
        (52, 12),
        (80, 23),
        (80, 24),
        (110, 26),
        (112, 26),
        (140, 42),
    ] {
        for mode in [
            Mode::Browse,
            Mode::Search,
            Mode::Filters,
            Mode::Settings,
            Mode::AutoConfirm,
            Mode::Help,
            Mode::Confirm,
        ] {
            app.mode = mode;
            screen(&mut app, w, h, 0);
        }
    }
}

#[test]
fn removal_rules_and_workflow_name_direction_and_next_action() {
    let mut app = app();
    app.scan.phase = "review".into();
    let text = screen(&mut app, 140, 42, 0);
    assert!(text.contains("READY TO CHECK ACTIVITY: press i"));
    assert!(text.contains("unverified only"));
    app.mode = Mode::Filters;
    let text = screen(&mut app, 140, 42, 0);
    assert!(text.contains("PROTECT: verified accounts"));
    assert!(text.contains("REMOVE: zero posts too"));
    assert!(text.contains("Hourly attempt budget"));
}

#[tokio::test]
async fn selected_unchecked_accounts_and_active_work_are_visible_in_the_queue_list() {
    use forgive_me::protocol::{Command, Work};
    let mut app = app();
    app.accounts.insert(
        "1".into(),
        Account {
            id: "1".into(),
            handle: "checking_me".into(),
            follows_me: Some(true),
            i_follow: Some(false),
            verified: Some(false),
            protected: Some(false),
            ..Default::default()
        },
    );
    app.selected.insert("1".into());
    let text = screen(&mut app, 140, 42, 0);
    assert!(text.contains("CHECK FIRST"));
    assert!(!text.contains("PROTECTED FROM REMOVAL"));
    app.batch = Some(Batch {
        id: "b".into(),
        ids: VecDeque::from(["1".into()]),
        policy: app.policy.clone(),
    });
    app.pending = Some(Work {
        command_id: "w".into(),
        owner_id: app.owner.clone(),
        command: Command::InspectAccount {
            target_id: "1".into(),
            policy: app.policy.clone(),
        },
    });
    let text = screen(&mut app, 140, 42, 0);
    assert!(text.contains("▶"));
    assert!(text.contains("Checking activity"));
    assert!(text.contains("Working…"));
    assert!(text.contains("list / animation"));
    app.key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE))
        .await
        .unwrap();
    assert!(screen(&mut app, 140, 42, 0).contains("Reseting followers"));
}

#[test]
fn sparse_old_is_a_candidate_and_missing_coverage_is_review_not_keep() {
    let mut app = app();
    app.accounts.insert(
        "1".into(),
        Account {
            id: "1".into(),
            handle: "example12345".into(),
            follows_me: Some(true),
            i_follow: Some(false),
            verified: Some(false),
            protected: Some(false),
            posts: Some(2),
            checked_at_ms: Some(forgive_me::model::now_ms()),
            last_activity_ms: Some(forgive_me::model::now_ms() - 657 * 86_400_000),
            activity_note: "Replies coverage incomplete".into(),
            ..Default::default()
        },
    );
    let text = screen(&mut app, 140, 36, 0);
    assert!(text.contains("Sparse + old observed activity"));
    assert!(text.contains(env!("CARGO_PKG_VERSION")));
    assert!(text.contains("Replies coverage incomplete"));
    app.policy.sparse_old_max_posts = 0;
    let text = screen(&mut app, 140, 42, 0);
    assert!(text.contains("REVIEW: Incomplete activity coverage"));
    assert!(!text.contains("PROTECTED FROM REMOVAL"));
}

#[test]
fn stream_continues_during_cooldown_and_confirmed_tags_move_downward() {
    let mut app = app();
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    app.sender = Some(tx);
    app.paused = false;
    app.show_queue_list = false;
    app.batch = Some(Batch {
        id: "b".into(),
        ids: VecDeque::new(),
        policy: app.policy.clone(),
    });
    app.pacing.until_ms = forgive_me::model::now_ms() + 60_000;
    app.animation.removed("departed_user".into());
    assert!(forgive_me::ritual::animating(&app));
    let top = screen(&mut app, 140, 42, 0);
    let down = screen(&mut app, 140, 42, 80);
    assert!(top.contains("✓ REMOVED"));
    assert!(down.find("@departed_user").unwrap() > top.find("@departed_user").unwrap());
    assert_eq!(app.removed, 0); // Animation cannot invent confirmed counts.
    app.animation.advance(std::time::Duration::from_secs(1));
    for _ in 0..9 {
        app.animation.advance(std::time::Duration::from_secs(1));
    }
    assert!(app.animation.departures.is_empty());
}

#[test]
fn auto_consent_and_system_settings_fit_small_terminals() {
    let mut app = app();
    for (w, h) in [(52, 12), (80, 24), (140, 42)] {
        app.mode = Mode::AutoConfirm;
        let text = screen(&mut app, w, h, 0);
        assert!(text.contains("There is no restore-followers action."));
        assert!(text.contains("y starts Full Auto"));
        app.mode = Mode::Settings;
        let text = screen(&mut app, w, h, 0);
        assert!(text.contains("Removal interval"));
        assert!(text.contains("Enter / Esc saves"));
    }
}
