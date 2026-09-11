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
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Install,
    Pair,
    Connect,
    Ready,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Installation {
    NotFound,
    Found(String),
    LegacyOnly,
    Unknown,
}

/// Chrome's profile files are a best-effort installation hint, never proof of connection.
/// Only extension registrations are inspected; credentials are neither retained nor logged.
pub fn detect_installation(root: &Path, extension: &Path) -> Installation {
    let Ok(profiles) = fs::read_dir(root) else {
        return Installation::Unknown;
    };
    let expected = crate::native::extension_id(extension).ok();
    let mut readable = false;
    let mut uncertain = false;
    let mut legacy = false;
    for profile in profiles.flatten() {
        let name = profile.file_name().to_string_lossy().into_owned();
        if name != "Default" && !name.starts_with("Profile ") {
            continue;
        }
        for file in ["Secure Preferences", "Preferences"] {
            let bytes = match fs::read(profile.path().join(file)) {
                Ok(bytes) => bytes,
                Err(error) => {
                    uncertain |= error.kind() != std::io::ErrorKind::NotFound;
                    continue;
                }
            };
            let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
                uncertain = true;
                continue;
            };
            readable = true;
            let Some(settings) = value
                .pointer("/extensions/settings")
                .and_then(Value::as_object)
            else {
                continue;
            };
            for (id, entry) in settings {
                let path = entry
                    .get("path")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if expected.as_ref() == Some(id) || Path::new(path) == extension {
                    return Installation::Found(name);
                }
                legacy |= path.ends_with("/forgive-me-extension");
            }
        }
    }
    if uncertain {
        Installation::Unknown
    } else if legacy {
        Installation::LegacyOnly
    } else if readable {
        Installation::NotFound
    } else {
        Installation::Unknown
    }
}

pub fn installed_extension(extension: &Path) -> Installation {
    let Some(home) = dirs::home_dir() else {
        return Installation::Unknown;
    };
    if !cfg!(target_os = "macos") {
        return Installation::Unknown;
    }
    detect_installation(
        &home.join("Library/Application Support/Google/Chrome"),
        extension,
    )
}

pub async fn open_install(extension: &Path) -> Result<()> {
    copy(extension.display().to_string()).await?;
    open_chrome("chrome://extensions".into()).await
}

pub async fn open_options(extension: &Path) -> Result<()> {
    let id = crate::native::extension_id(extension)?;
    open_chrome(format!("chrome-extension://{id}/options.html")).await
}

pub struct Setup {
    pub step: Step,
    pub installation: Installation,
    pub port: u16,
    pub token: String,
    pub extension_dir: PathBuf,
    pub revealed: bool,
    pub manual: bool,
    pub scroll: u16,
    pub recheck_requested: bool,
    pub account_error: Option<String>,
    pub retry_at_ms: Option<i64>,
}
impl Setup {
    pub fn new(config: &Config) -> Self {
        Self {
            installation: Installation::Unknown,
            step: Step::Install,
            port: config.port,
            token: config.token.clone(),
            extension_dir: extension_dir(),
            revealed: false,
            manual: false,
            scroll: 0,
            recheck_requested: false,
            account_error: None,
            retry_at_ms: None,
        }
    }
    pub fn refresh_installation(&mut self) {
        self.installation = installed_extension(&self.extension_dir);
        if matches!(
            self.installation,
            Installation::NotFound | Installation::LegacyOnly
        ) {
            self.go(Step::Install);
        } else if matches!(self.installation, Installation::Found(_)) {
            self.go(Step::Pair);
        }
    }
    pub fn go(&mut self, step: Step) {
        self.step = step;
        self.revealed = false;
        self.manual = false;
        self.scroll = 0;
        self.recheck_requested = false;
        if step == Step::Ready {
            self.account_error = None;
        }
    }
}

pub fn extension_dir() -> PathBuf {
    let bundled = std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .and_then(|p| p.parent().map(|p| p.join("remover-extension")))
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
        27,
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
            Span::styled("◈ remover", bold(MINT)),
            Span::styled(
                format!("   SETUP  /  v{}", env!("CARGO_PKG_VERSION")),
                fg(MUTED),
            ),
        ])),
        header,
    );
    let chrome = if connected {
        format!(
            "✓ Chrome connected / extension {}",
            if app.browser_version.is_empty() {
                "version unknown"
            } else {
                &app.browser_version
            }
        )
    } else {
        match &setup.installation {
            Installation::Found(profile) => {
                format!("○ Extension listed in {profile}; not connected")
            }
            Installation::NotFound => "○ Chrome extension not installed".into(),
            Installation::LegacyOnly => "○ Only the old extension was found".into(),
            Installation::Unknown => "○ Chrome not connected; installation not confirmed".into(),
        }
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
                "Open Connection help in the extension.\nPort: {}\nSecret: {}\n\nPaste the secret there, then Save & connect.",
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
                "Connected as @{}\n\nIs this the account you want to clean?\nContinue to review the cleanup rule. Nothing runs yet.",
                app.handle
            ),
            "Enter  Use this account",
            "r recheck account  q quit",
        )
    } else if connected {
        if app.pending.is_some() {
            (" Identifying your account ", "Chrome is paired. Checking the signed-in X account.\n\nThis screen advances when the account is identified.".into(),
            "Checking X…", "b open X  q quit")
        } else if let Some(error) = &setup.account_error {
            (
                " X account check failed ",
                format!(
                    "{error}\n\nChrome is still paired. Press b to open X, or o to open\nthe extension's repair settings."
                ),
                "Enter  Retry account check",
                "b open X  o extension  r retry  q quit",
            )
        } else {
            (" Sign in to X ", "Open X in this Chrome profile and sign in.\n\nThe extension checks again when the page loads.\nAlready signed in? Press r to check again.".into(),
            "Enter  Open X", "r recheck account  m repair  q quit")
        }
    } else if setup.step == Step::Install {
        (
            " Add the Chrome extension ",
            format!(
                "{}Press Enter to open Chrome and copy the folder path.\n\n1. Turn on Developer mode. Select Load unpacked.\n2. Press Cmd+Shift+G, paste, and select the folder.\n3. Open Remover, then Open X in the same profile.\n\nPairing starts when the extension loads.",
                if setup.installation == Installation::LegacyOnly {
                    "The old extension cannot connect. Install the new R icon.\n\n"
                } else {
                    ""
                }
            ),
            "Enter  Set up Chrome",
            "r check again  o installed  m manual  q quit",
        )
    } else {
        (" Connect your extension ", "Open the extension and select Connect terminal.\nKeep this terminal running.\n\nIf disabled, enable Remover on chrome://extensions.\nPress i to install or reload it. Use the same Chrome\nprofile for the extension and X.\n\nThis screen advances when Chrome connects.".into(),
        "Enter  Open extension", "i install / reload  r check again  m manual  q quit")
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
