use crate::{
    app::App,
    config::Config,
    theme::{self, *},
};
use anyhow::Result;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout},
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
            step: if config.extension_id.is_some() {
                Step::Pair
            } else {
                Step::Install
            },
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

pub fn render(frame: &mut Frame, app: &mut App) {
    let Some(setup) = &mut app.setup else { return };
    let connected = app.sender.is_some();
    let ready = connected && !app.handle.is_empty();
    let compact = frame.area().height < 22;
    let area = centered(
        frame.area(),
        frame.area().width.saturating_sub(4).min(82),
        23,
    );
    let [header, status, body, action, keys, notice] = Layout::vertical([
        Constraint::Length(if compact { 1 } else { 3 }),
        Constraint::Length(if compact { 1 } else { 4 }),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Length(if compact { 2 } else { 3 }),
    ])
    .areas(area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("◈ forgive-me", bold(MINT)),
            Span::styled(
                format!("   SETUP  /  v{}", env!("CARGO_PKG_VERSION")),
                fg(MUTED),
            ),
        ])),
        header,
    );
    let chrome = if connected {
        "✓ Chrome connected"
    } else {
        "○ Chrome not connected"
    };
    let account = if ready {
        format!("✓ @{}", app.handle)
    } else if connected {
        "○ X account not identified".into()
    } else {
        "· X account waits for Chrome".into()
    };
    let states = if compact {
        vec![Line::from(vec![
            Span::styled("✓ Terminal   ", fg(MINT)),
            Span::styled(
                if connected {
                    "✓ Chrome   "
                } else {
                    "○ Chrome   "
                },
                fg(if connected { MINT } else { AMBER }),
            ),
            Span::styled(
                if ready {
                    "✓ X account"
                } else {
                    "○ X account"
                },
                fg(MUTED),
            ),
        ])]
    } else {
        vec![
            Line::styled("✓ Terminal is ready", fg(MINT)),
            Line::styled(chrome, fg(if connected { MINT } else { AMBER })),
            Line::styled(account, fg(if ready { MINT } else { MUTED })),
        ]
    };
    frame.render_widget(Paragraph::new(states), status);
    let (title, text, primary, secondary) = if setup.manual {
        (
            " Manual connection ",
            format!(
                "Open Manual connection and repair in the extension.\nPort: {}\nSecret: {}\n\nPaste the secret there, then Save & connect.",
                setup.port,
                if setup.revealed {
                    &setup.token
                } else {
                    "[hidden]"
                }
            ),
            "Enter  Copy secret",
            "o open extension  v show/hide  Esc back  q quit",
        )
    } else if ready {
        (
            " Confirm your account ",
            format!(
                "Connected as @{}\n\nIs this the account you want to clean?\nContinue to review your followers. Nothing runs yet.",
                app.handle
            ),
            "Enter  Use this account",
            "r recheck account  q quit",
        )
    } else if connected {
        if app.pending.is_some() {
            (" Identifying your account ", "Chrome is paired. Checking the signed-in X account.\n\nThis screen advances when the account is identified.".into(),
            "Checking X…", "b open X  q quit")
        } else {
            (" Sign in to X ", "Open X in this Chrome profile and sign in.\n\nThe extension checks again when the page loads.\nAlready signed in? Press r to check again.".into(),
            "Enter  Open X", "r recheck account  m repair  q quit")
        }
    } else if setup.step == Step::Install {
        (" Add the Chrome extension ", "Press Enter to open Chrome and copy the folder path.\n\n1. Turn on Developer mode. Select Load unpacked.\n2. Press Cmd+Shift+G, paste, and select the folder.\n\nPairing starts when the extension loads.".into(),
        "Enter  Set up Chrome", "o already installed  m manual  q quit")
    } else {
        (" Connect your extension ", "Open the extension and select Connect automatically.\nKeep this terminal running.\n\nNeed to install or update it? Press i to open Chrome's\nextension page, then Load unpacked or Reload.\n\nThis screen advances when Chrome connects.".into(),
        "Enter  Open extension", "i install / reload  m manual  q quit")
    };
    let block = theme::panel(title).border_style(fg(ICE));
    let inner = block.inner(body);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
    let max_scroll = paragraph
        .line_count(inner.width)
        .saturating_sub(usize::from(inner.height))
        .min(u16::MAX as usize) as u16;
    setup.scroll = setup.scroll.min(max_scroll);
    frame.render_widget(paragraph.block(block).scroll((setup.scroll, 0)), body);
    frame.render_widget(
        Paragraph::new(primary)
            .alignment(Alignment::Center)
            .style(bold(BG).bg(if connected && app.pending.is_some() {
                ICE
            } else {
                MINT
            })),
        action,
    );
    frame.render_widget(
        Paragraph::new(if max_scroll > 0 {
            format!("{secondary}   ↑↓ scroll")
        } else {
            secondary.into()
        })
        .wrap(Wrap { trim: false })
        .style(fg(MUTED)),
        keys,
    );
    frame.render_widget(
        Paragraph::new(app.notice.as_str())
            .wrap(Wrap { trim: false })
            .style(fg(AMBER)),
        notice,
    );
}
