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
    assert_ne!(screen(&mut app, 140, 42, 0), screen(&mut app, 140, 42, 8));
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
            Mode::Help,
            Mode::Confirm,
        ] {
            app.mode = mode;
            screen(&mut app, w, h, 0);
        }
    }
}
