use crate::{
    app::App,
    config::Config,
    theme::{self, *},
};
use anyhow::Result;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Install,
    Pair,
    Connect,
    Ready,
}

pub struct Setup {
    pub step: Step,
    pub port: u16,
    pub token: String,
    pub extension_dir: PathBuf,
    pub revealed: bool,
    pub manual: bool,
    pub scroll: u16,
}
impl Setup {
    pub fn new(config: &Config) -> Self {
        Self {
            step: Step::Install,
            port: config.port,
            token: config.token.clone(),
            extension_dir: extension_dir(),
            revealed: false,
            manual: false,
            scroll: 0,
        }
    }
    pub fn go(&mut self, step: Step) {
        self.step = step;
        self.revealed = false;
        self.manual = false;
        self.scroll = 0;
    }
}

pub fn extension_dir() -> PathBuf {
    let bundled = std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .and_then(|p| p.parent().map(|p| p.join("forgive-me-extension")))
        .filter(|p| p.join("manifest.json").is_file());
    bundled.unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("extension/dist"))
}

pub async fn open_chrome(url: String) -> Result<()> {
    #[cfg(target_os = "macos")]
    return tokio::task::spawn_blocking(move || {
        let status = std::process::Command::new("/usr/bin/open")
            .args(["-a", "Google Chrome", &url])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        anyhow::ensure!(status.success(), "Open Chrome manually to continue setup");
        Ok(())
    })
    .await?;
    #[cfg(not(target_os = "macos"))]
    {
        let _ = url;
        anyhow::bail!("Open Chrome manually to continue setup");
    }
}

pub async fn copy(value: String) -> Result<()> {
    #[cfg(target_os = "macos")]
    return tokio::task::spawn_blocking(move || {
        use std::{
            io::Write,
            process::{Command, Stdio},
        };
        let mut child = Command::new("/usr/bin/pbcopy")
            .stdin(Stdio::piped())
            .spawn()?;
        let written = child.stdin.take().unwrap().write_all(value.as_bytes());
        let status = child.wait()?;
        written?;
        anyhow::ensure!(
            status.success(),
            "Clipboard unavailable; press v to reveal the secret."
        );
        Ok(())
    })
    .await?;
    #[cfg(not(target_os = "macos"))]
    {
        let _ = value;
        anyhow::bail!(
            "Clipboard shortcut is available on macOS. Select the text to copy; v reveals the secret."
        );
    }
}

pub fn render(frame: &mut Frame, app: &App) {
    let Some(setup) = &app.setup else {
        return;
    };
    let area = frame.area();
    let area = centered(area, area.width.saturating_sub(4).min(96), area.height);
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(if area.height >= 24 { 6 } else { 4 }),
        Constraint::Min(2),
        Constraint::Length(4),
    ])
    .areas(area);
    let accent = bold(MINT);
    let muted = fg(MUTED);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(" ◈ forgive-me  /  FIRST CONNECTION", accent)),
            Line::styled(" Your terminal controls it. Chrome does the X work.", muted),
            Line::from(Span::styled(
                match setup.step {
                    Step::Install => " [1 Install]  →  2 Pair  →  3 Connect",
                    Step::Pair => " 1 Install  →  [2 Pair]  →  3 Connect",
                    Step::Connect => " 1 Install  →  2 Pair  →  [3 Connect]",
                    Step::Ready => " ✓ Chrome paired  ·  ✓ X account identified",
                },
                accent,
            )),
        ]),
        header,
    );
    let (title, text, keys) = match setup.step {
        Step::Install => (
            " 1 / Set up Chrome ",
            format!(
                "Press b to start setup.

The wizard opens Chrome and copies the extension folder path.
Chrome requires you to confirm installation once.
The extension then pairs with this terminal automatically.

Extension folder:
{}

Already installed? Press Enter, then b to open its setup page.",
                setup.extension_dir.display()
            ),
            " b start setup   Enter already installed   q quit",
        ),
        Step::Pair if setup.manual => (
            " Manual pairing ",
            format!(
                "Open Manual connection and repair in the extension.
Enter the port and pairing secret. Select Save & connect.

Local port: {}

Pairing secret: {}

Press y to copy the secret. Press v to show or hide it.
Keep the secret private.

Press m to return to automatic pairing.",
                setup.port,
                if setup.revealed {
                    &setup.token
                } else {
                    "[hidden · y copies · v reveals]"
                }
            ),
            " y copy secret   v show/hide   m automatic   ← back   q quit",
        ),
        Step::Pair => (
            " 2 / Install and pair ",
            format!(
                "In Chrome's extension page:

1. Enable Developer mode.
2. Select Load unpacked.
3. Press Cmd+Shift+G in the folder selector.
4. Paste the copied folder path.
5. Select the folder.

{}

Pairing starts automatically when the extension loads.
If already installed, press b to open its setup page.
No port or secret entry is needed.",
                setup.extension_dir.display()
            ),
            " b open extension   y copy folder   m manual   Enter next   q quit",
        ),
        Step::Connect => (
            " 3 / Connect your X account ",
            format!(
                "Press b to open X in Chrome.
Sign in to the account you want to use.
The extension rechecks the account after the page loads.

Chrome: {}
X account: {}

Use the same Chrome profile for the extension and X.
If identification fails, refresh X and press r.
If Chrome is disconnected, press ←, then b to retry pairing.",
                if app.sender.is_some() {
                    "paired and connected"
                } else {
                    "waiting for the extension"
                },
                if app.pending.is_some() {
                    "checking…"
                } else {
                    "not identified yet"
                }
            ),
            " b open X   r retry account check   ← pairing   m manual   q quit",
        ),
        Step::Ready => (
            " You're connected ",
            format!(
                "Connected as @{}\n\nCheck that this is the account you want to clean.\nIf it is wrong, switch accounts on X and press r to recheck.\n\nNext, press Enter to open your follower list.\nPress s there to scan your following and followers.\nReview the results before choosing anyone to remove.\n\nKeep Chrome and this terminal open during cleanup.\nYou can reopen this guide with Shift+P.",
                app.handle
            ),
            " Enter open followers   r recheck account   q quit",
        ),
    };
    frame.render_widget(
        Paragraph::new(text)
            .block(theme::panel(title).border_style(fg(ICE)))
            .wrap(Wrap { trim: false })
            .scroll((setup.scroll, 0)),
        body,
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(app.notice.clone(), fg(AMBER))),
            Line::styled(keys, fg(ICE)),
            Line::from(Span::styled(" ↑ ↓ scroll instructions", muted)),
        ])
        .wrap(Wrap { trim: false }),
        footer,
    );
}
