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
    pub scroll: u16,
}
impl Setup {
    pub fn new(config: &Config) -> Self {
        let bundled = std::env::current_exe()
            .ok()
            .and_then(|p| p.canonicalize().ok())
            .and_then(|p| p.parent().map(|p| p.join("forgive-me-extension")))
            .filter(|p| p.join("manifest.json").is_file());
        Self {
            step: Step::Install,
            port: config.port,
            token: config.token.clone(),
            extension_dir: bundled.unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("extension/dist")
            }),
            revealed: false,
            scroll: 0,
        }
    }
    pub fn go(&mut self, step: Step) {
        self.step = step;
        self.revealed = false;
        self.scroll = 0;
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
            " 1 / Load the Chrome extension ",
            format!(
                "Open Chrome using the profile where you use X.\n\n1. Go to chrome://extensions\n2. Turn on Developer mode (top right).\n3. Click Load unpacked and choose this folder:\n\n{}\n\nPress y to copy the folder path. In the macOS folder chooser,\npress Cmd+Shift+G, paste the path, then select the folder.\n\nAlready loaded? Press Enter to continue.",
                setup.extension_dir.display()
            ),
            " Enter next   y copy folder   q quit",
        ),
        Step::Pair => (
            " 2 / Pair with this terminal ",
            format!(
                "1. Click Chrome's puzzle icon, then forgive-me.\n   This opens the extension's connection settings.\n2. Enter the local port and paste the pairing secret below.\n3. Click Save & connect. Keep this terminal running.\n\nLocal port: {}\n\nPairing secret: {}\n\nPress y to copy the secret without showing it.\nPress v to show/hide it. Keep the secret private.\n\nChrome connection: {}",
                setup.port,
                if setup.revealed {
                    &setup.token
                } else {
                    "[hidden · y copies · v reveals]"
                },
                if app.sender.is_some() {
                    "connected"
                } else {
                    "waiting for Save & connect"
                }
            ),
            " Enter next   y copy secret   v show/hide   ← back   q quit",
        ),
        Step::Connect => (
            " 3 / Connect your X account ",
            format!(
                "1. Keep an x.com tab open and sign in to your account.\n2. In the extension settings, click Save & connect.\n3. Wait here while forgive-me checks the signed-in account.\n\nChrome: {}\nX account: {}\n\nIf the account check fails, refresh your X tab, choose\nRefresh X discovery in the extension, then press r here.\n\nStill disconnected? Press ← to check the port and secret.\nUse the same Chrome profile for the extension and X.",
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
            " r retry account check   ← pairing details   q quit",
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
