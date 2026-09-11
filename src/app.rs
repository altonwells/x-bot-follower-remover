use crate::{
    bridge::BridgeEvent,
    model::{Account, Policy, clean, new_id, now_ms},
    protocol::{ClientMessage, Command, VERSION, Work, WorkResult},
    setup::{Setup, Step},
    store::{Storage, Store},
};
use anyhow::{Result, bail};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    time::{Duration, Instant},
};
use tokio::sync::mpsc;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Scan {
    #[serde(default)]
    pub adapter_revision: u8,
    pub phase: String,
    pub cursor: Option<String>,
    pub cursors: BTreeSet<String>,
    pub started_at: i64,
    pub following_complete: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Batch {
    pub id: String,
    pub ids: VecDeque<String>,
    pub policy: Policy,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    Setup,
    Browse,
    Search,
    Filters,
    Details,
    Help,
    Confirm,
}
pub struct App {
    pub setup: Option<Setup>,
    pub store: Storage,
    pub owner: String,
    pub handle: String,
    pub browser_version: String,
    pub accounts: BTreeMap<String, Account>,
    pub policy: Policy,
    pub selected: BTreeSet<String>,
    pub focus: usize,
    pub query: String,
    pub mode: Mode,
    pub filter_row: usize,
    pub modal_scroll: u16,
    pub paused: bool,
    pub scan: Scan,
    pub batch: Option<Batch>,
    pub confirmation: Vec<String>,
    pub quit: bool,
    pub sender: Option<mpsc::Sender<Value>>,
    pub session: String,
    pub pending: Option<Work>,
    pub next_at: Instant,
    pub notice: String,
    pub capabilities: Vec<String>,
    pub removed: usize,
    pub uncertain: usize,
    pub demo: bool,
    pub only_matching: bool,
    pub pacing: crate::pacing::Pacing,
    pub detach_requested: bool,
}
impl App {
    pub fn new(store: Store, demo: bool) -> Result<Self> {
        let owner: String = store.get("last_owner")?.unwrap_or_default();
        let accounts = store
            .accounts(&owner)?
            .into_iter()
            .map(|a| (a.id.clone(), a))
            .collect();
        let policy = store.get("policy")?.unwrap_or_default();
        let scan = store.get(&format!("scan:{owner}"))?.unwrap_or_default();
        let batch = store.get(&format!("batch:{owner}"))?.flatten();
        let pacing = store.get(&format!("pacing:{owner}"))?.unwrap_or_default();
        let (removed, uncertain) = store.action_counts(&owner)?;
        Ok(Self {
            setup: None,
            store: Storage::new(store),
            owner,
            handle: String::new(),
            browser_version: String::new(),
            accounts,
            policy,
            selected: BTreeSet::new(),
            focus: 0,
            query: String::new(),
            mode: Mode::Browse,
            filter_row: 0,
            modal_scroll: 0,
            paused: true,
            scan,
            batch,
            confirmation: vec![],
            quit: false,
            sender: None,
            session: String::new(),
            pending: None,
            next_at: Instant::now(),
            notice: "Open Chrome, pair the extension, then press s to scan. ? shows help.".into(),
            capabilities: vec![],
            removed,
            uncertain,
            demo,
            only_matching: false,
            pacing,
            detach_requested: false,
        })
    }
    pub fn configure_setup(&mut self, config: &crate::config::Config) {
        self.setup = Some(Setup::new(config));
        if config.extension_id.is_none() || self.owner.is_empty() {
            self.mode = Mode::Setup;
            self.log("Welcome. Let's connect your Chrome extension.");
        }
    }
    pub fn visible(&self) -> Vec<&Account> {
        let q = self.query.to_lowercase();
        let now = now_ms();
        let mut accounts: Vec<_> = self
            .accounts
            .values()
            .filter(|a| a.follows_me == Some(true))
            .filter(|a| {
                q.is_empty()
                    || a.handle.to_lowercase().contains(&q)
                    || a.name.to_lowercase().contains(&q)
            })
            .filter(|a| !self.only_matching || a.reason(&self.policy, now).is_ok())
            .collect();
        accounts.sort_by_cached_key(|a| (a.handle.to_lowercase(), &a.id));
        accounts
    }
    pub fn focused(&self) -> Option<String> {
        let rows = self.visible();
        rows.get(self.focus.min(rows.len().saturating_sub(1)))
            .map(|a| a.id.clone())
    }
    pub fn matches(&self) -> usize {
        self.accounts
            .values()
            .filter(|a| a.reason(&self.policy, now_ms()).is_ok())
            .count()
    }
    pub fn log(&mut self, text: impl Into<String>) {
        self.notice = clean(&text.into());
    }
    async fn save_batch(&self) -> Result<()> {
        let key = format!("batch:{}", self.owner);
        let b = self.batch.clone();
        self.store.run(move |s| s.set(&key, &b)).await
    }
    async fn save_scan(&self) -> Result<()> {
        let key = format!("scan:{}", self.owner);
        let scan = self.scan.clone();
        self.store.run(move |s| s.set(&key, &scan)).await
    }
    async fn send_control(&self, kind: &str) -> Result<()> {
        if let Some(tx) = &self.sender {
            tx.send(json!({"type":kind,"session_id":self.session}))
                .await?;
        }
        Ok(())
    }
    async fn submit(&mut self, command: Command) -> Result<()> {
        if self.pending.is_some() {
            bail!("Wait for the current task");
        }
        let sender = self
            .sender
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Chrome is not connected"))?;
        let work = Work {
            command_id: new_id(),
            owner_id: self.owner.clone(),
            command,
        };
        if let Command::RemoveFollower { policy, .. } = &work.command {
            let allowed = self.pacing.reserve(policy.batch_limit);
            let pacing = self.pacing.clone();
            let key = format!("pacing:{}", self.owner);
            self.store.run(move |s| s.set(&key, &pacing)).await?;
            if !allowed {
                return Ok(());
            }
            let w = work.clone();
            self.store.run(move |s| s.start_action(&w)).await?;
        }
        self.pending = Some(work.clone());
        sender
            .send(json!({"v":VERSION,"type":"command","session_id":self.session,"work":work}))
            .await?;
        Ok(())
    }
    pub async fn pause(&mut self) -> Result<()> {
        self.detach_requested = false;
        self.paused = true;
        self.send_control("pause").await?;
        self.save_batch().await?;
        self.save_scan().await
    }
    async fn start_scan(&mut self) -> Result<()> {
        if !self.demo && !self.capabilities.iter().any(|c| c == "adapter:2") {
            bail!("Reload forgive-me in chrome://extensions, then Save & connect before scanning");
        }
        if self.owner.is_empty() || self.sender.is_none() {
            bail!("Connect Chrome and identify the signed-in account first");
        }
        if self.batch.is_some() || self.uncertain > 0 {
            bail!("Finish/cancel the removal queue and reconcile uncertain actions first (r)");
        }
        if self.pending.is_some() {
            bail!("Wait for the current browser task before starting a scan");
        }
        if self.scan.phase.is_empty() || self.scan.phase == "done" || self.scan.adapter_revision < 2
        {
            for a in self.accounts.values_mut() {
                a.follows_me = None;
                a.i_follow = None;
            }
            self.scan = Scan {
                adapter_revision: 2,
                phase: "following".into(),
                started_at: now_ms(),
                ..Default::default()
            };
            let (owner, items, scan) = (
                self.owner.clone(),
                self.accounts.values().cloned().collect::<Vec<_>>(),
                self.scan.clone(),
            );
            self.store
                .run(move |s| s.save_page(&owner, &items, &format!("scan:{owner}"), &scan))
                .await?;
        }
        self.send_control("resume").await?;
        self.paused = false;
        self.log("Collecting following, then followers. Activity is a separate step (i). Progress is saved after each page.");
        Ok(())
    }
    pub async fn key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.pause().await?;
            self.quit = true;
            return Ok(());
        }
        if self.mode == Mode::Setup {
            return self.setup_key(key).await;
        }
        // Stop controls remain available in every modal, including search.
        match key.code {
            KeyCode::Char('p') => {
                if self.paused {
                    if self.uncertain > 0 {
                        bail!("Press r to reconcile uncertain actions first");
                    }
                    self.send_control("resume").await?;
                    self.paused = false;
                } else {
                    self.pause().await?;
                }
            }
            KeyCode::Char('c') => {
                self.pause().await?;
                self.batch = None;
                self.save_batch().await?;
                self.log("Pending removals cancelled; an already dispatched action may finish.");
            }
            _ => {}
        }
        if matches!(key.code, KeyCode::Char('p' | 'c')) {
            return Ok(());
        }
        match self.mode {
            Mode::Search => {
                match key.code {
                    KeyCode::Esc | KeyCode::Enter => self.mode = Mode::Browse,
                    KeyCode::Backspace => {
                        self.query.pop();
                    }
                    KeyCode::Char(c) => self.query.push(c),
                    _ => {}
                }
                self.focus = 0;
                return Ok(());
            }
            Mode::Confirm => {
                match key.code {
                    KeyCode::Char('y') => {
                        if self.pending.is_some() || self.uncertain > 0 {
                            bail!("Wait for in-flight work; reconcile uncertain actions first");
                        }
                        let ids: VecDeque<_> = self
                            .confirmation
                            .iter()
                            .filter(|id| {
                                self.accounts
                                    .get(*id)
                                    .is_some_and(|a| a.reason(&self.policy, now_ms()).is_ok())
                            })
                            .cloned()
                            .collect();
                        if !ids.is_empty() {
                            self.batch = Some(Batch {
                                id: new_id(),
                                ids,
                                policy: self.policy.clone(),
                            });
                            self.save_batch().await?;
                            self.send_control("resume").await?;
                            self.paused = false;
                            self.log("Removal queue approved. p pauses; b runs in background; c cancels remaining work.");
                        }
                        self.mode = Mode::Browse;
                    }
                    KeyCode::Esc | KeyCode::Char('n') | KeyCode::Enter => self.mode = Mode::Browse,
                    _ => {}
                }
                return Ok(());
            }
            Mode::Filters => {
                match key.code {
                    KeyCode::Esc | KeyCode::Enter => {
                        self.mode = Mode::Browse;
                        let p = self.policy.clone();
                        self.store.run(move |s| s.set("policy", &p)).await?;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.filter_row = (self.filter_row + 1) % 6
                    }
                    KeyCode::Up | KeyCode::Char('k') => self.filter_row = (self.filter_row + 5) % 6,
                    KeyCode::Left | KeyCode::Char('-') => self.adjust(-1),
                    KeyCode::Right | KeyCode::Char('+') | KeyCode::Char(' ') => self.adjust(1),
                    _ => {}
                }
                return Ok(());
            }
            Mode::Details | Mode::Help => {
                match key.code {
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q' | '?') => {
                        self.mode = Mode::Browse
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.modal_scroll = self.modal_scroll.saturating_add(1)
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.modal_scroll = self.modal_scroll.saturating_sub(1)
                    }
                    KeyCode::PageDown => self.modal_scroll = self.modal_scroll.saturating_add(10),
                    KeyCode::PageUp => self.modal_scroll = self.modal_scroll.saturating_sub(10),
                    KeyCode::Home => self.modal_scroll = 0,
                    _ => {}
                }
                return Ok(());
            }
            Mode::Browse | Mode::Setup => {}
        }
        match key.code {
            KeyCode::Char('P') if self.setup.is_some() => {
                self.pause().await?;
                self.confirmation.clear();
                let setup = self.setup.as_mut().unwrap();
                setup.go(if self.sender.is_none() {
                    Step::Pair
                } else if self.handle.is_empty() {
                    Step::Connect
                } else {
                    Step::Ready
                });
                self.mode = Mode::Setup;
                self.log("Pairing guide. Work is paused.");
            }
            KeyCode::Char('q') => {
                self.pause().await?;
                self.quit = true;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.focus = (self.focus + 1).min(self.visible().len().saturating_sub(1))
            }
            KeyCode::Up | KeyCode::Char('k') => self.focus = self.focus.saturating_sub(1),
            KeyCode::PageDown => {
                self.focus = (self.focus + 15).min(self.visible().len().saturating_sub(1))
            }
            KeyCode::PageUp => self.focus = self.focus.saturating_sub(15),
            KeyCode::Char('/') => self.mode = Mode::Search,
            KeyCode::Char('?') => {
                self.modal_scroll = 0;
                self.mode = Mode::Help;
            }
            KeyCode::Enter => {
                self.modal_scroll = 0;
                self.mode = Mode::Details;
            }
            KeyCode::Char('f') => {
                if self.batch.is_some() {
                    bail!("Cancel the current batch before changing filters");
                }
                self.mode = Mode::Filters;
            }
            KeyCode::Char('m') => {
                self.only_matching = !self.only_matching;
                self.focus = 0;
            }
            KeyCode::Char('s') => self.start_scan().await?,
            KeyCode::Char('i') => {
                if self.batch.is_some()
                    || self.pending.is_some()
                    || !matches!(self.scan.phase.as_str(), "review" | "inspect" | "done")
                {
                    bail!("Finish collecting followers first (s), then press i to check activity");
                }
                self.scan.phase = "inspect".into();
                self.scan.started_at = now_ms();
                self.save_scan().await?;
                self.send_control("resume").await?;
                self.paused = false;
                self.log("Checking activity. Protected accounts are skipped; unknown evidence never qualifies for removal.");
            }
            KeyCode::Char('b') => {
                if self.demo {
                    bail!("Background work is unavailable in demo mode");
                }
                if self.batch.is_none() || self.paused {
                    bail!("Approve and start a removal queue first (d then y)");
                }
                if !self.capabilities.iter().any(|c| c == "durable_queue:1") {
                    bail!("Reload the updated Chrome extension before running in the background");
                }
                self.detach_requested = true;
                self.log(
                    "Moving approved queue to the background after the current task finishes…",
                );
            }
            KeyCode::Char(' ') => {
                if let Some(id) = self.focused() {
                    if self.accounts[&id].reason(&self.policy, now_ms()).is_ok() {
                        if !self.selected.remove(&id) {
                            self.selected.insert(id);
                        }
                    } else {
                        self.log("Protected from removal: inspect the decision column or press Enter for the reason.");
                    }
                }
            }
            KeyCode::Char('a') => {
                self.selected = self
                    .visible()
                    .into_iter()
                    .filter(|a| a.reason(&self.policy, now_ms()).is_ok())
                    .map(|a| a.id.clone())
                    .collect();
            }
            KeyCode::Char('K') => {
                if let Some(id) = self.focused() {
                    if self.pending.as_ref().is_some_and(|w| matches!(&w.command, Command::RemoveFollower { target_id, .. } if target_id == &id)) { self.pause().await?; }
                    let kept = !self.accounts[&id].kept;
                    self.accounts.get_mut(&id).unwrap().kept = kept;
                    self.selected.remove(&id);
                    let owner = self.owner.clone();
                    self.store.run(move |s| s.keep(&owner, &id, kept)).await?;
                }
            }
            KeyCode::Char('d') => {
                if self.batch.is_some() || self.pending.is_some() {
                    bail!(
                        "Pause and wait for the current task, or cancel the existing removal queue"
                    );
                }
                if !self.capabilities.iter().any(|c| c == "remove_follower") {
                    bail!(
                        "Removal adapter unavailable. Open X in Chrome and refresh discovery in the extension."
                    );
                }
                self.confirmation = self
                    .selected
                    .iter()
                    .filter(|id| {
                        self.accounts
                            .get(*id)
                            .is_some_and(|a| a.reason(&self.policy, now_ms()).is_ok())
                    })
                    .cloned()
                    .collect();
                if self.confirmation.is_empty() {
                    bail!("Select eligible accounts first");
                }
                self.paused = true;
                self.mode = Mode::Confirm;
            }
            KeyCode::Char('o') => {
                if let Some(target_id) = self.focused() {
                    self.submit(Command::OpenProfile { target_id }).await?;
                }
            }
            KeyCode::Char('r') => {
                let owner = self.owner.clone();
                let unresolved = self.store.run(move |s| s.unresolved(&owner)).await?;
                if let Some(w) = unresolved.first() {
                    if let Command::RemoveFollower { target_id, .. } = &w.command {
                        self.submit(Command::Reconcile {
                            target_id: target_id.clone(),
                            original_command_id: w.command_id.clone(),
                        })
                        .await?;
                    }
                } else {
                    self.log("No uncertain actions to reconcile.");
                }
            }
            _ => {}
        }
        Ok(())
    }
    async fn setup_key(&mut self, key: KeyEvent) -> Result<()> {
        let Some(setup) = self.setup.as_mut() else {
            return Ok(());
        };
        match key.code {
            KeyCode::Char('q') => {
                self.pause().await?;
                self.quit = true;
            }
            KeyCode::Char('v') if setup.step == Step::Pair => {
                setup.manual = true;
                setup.revealed = !setup.revealed;
            }
            KeyCode::Char('m') => {
                let manual = !setup.manual;
                setup.go(Step::Pair);
                setup.manual = manual;
            }
            KeyCode::Char('i') if self.sender.is_none() => {
                setup.go(Step::Install);
                crate::setup::copy(setup.extension_dir.display().to_string()).await?;
                crate::setup::open_chrome("chrome://extensions".into()).await?;
                self.log("Folder copied. In Chrome, Load unpacked or Reload forgive-me. Waiting for connection.");
            }
            KeyCode::Char('o') => {
                let id = crate::native::extension_id(&setup.extension_dir)?;
                crate::setup::open_chrome(format!("chrome-extension://{id}/options.html")).await?;
                if !setup.manual && self.sender.is_none() {
                    setup.go(Step::Pair);
                }
                self.log("Extension opened. Follow the instructions above.");
            }
            KeyCode::Char('y') if self.sender.is_none() || setup.manual => {
                let value = if setup.manual {
                    setup.token.clone()
                } else {
                    setup.extension_dir.display().to_string()
                };
                crate::setup::copy(value).await?;
                self.log("Copied. Paste in Chrome.");
            }
            KeyCode::Enter | KeyCode::Char('b') => {
                if setup.manual {
                    crate::setup::copy(setup.token.clone()).await?;
                    self.log("Secret copied. Paste in Manual connection and repair.");
                } else if self.sender.is_some()
                    && !self.handle.is_empty()
                    && key.code == KeyCode::Enter
                {
                    self.mode = Mode::Browse;
                    self.log("Account confirmed. Press s to scan your followers.");
                } else if self.sender.is_some()
                    && setup.account_error.is_some()
                    && key.code == KeyCode::Enter
                {
                    self.check_session().await?;
                } else if self.sender.is_some() {
                    if self.pending.is_none() || key.code == KeyCode::Char('b') {
                        crate::setup::open_chrome("https://x.com/".into()).await?;
                        self.log("Sign in to X. The extension checks again when the page loads.");
                    }
                } else if setup.step == Step::Install {
                    crate::setup::copy(setup.extension_dir.display().to_string()).await?;
                    crate::setup::open_chrome("chrome://extensions".into()).await?;
                    self.log("Folder copied. Load unpacked in Chrome. Waiting for the extension to connect.");
                } else {
                    let id = crate::native::extension_id(&setup.extension_dir)?;
                    crate::setup::open_chrome(format!("chrome-extension://{id}/options.html"))
                        .await?;
                    self.log(
                        "Select Connect automatically in the extension. Waiting for connection.",
                    );
                }
            }
            KeyCode::Esc | KeyCode::Left => {
                setup.go(if self.sender.is_none() {
                    Step::Pair
                } else if self.handle.is_empty() {
                    Step::Connect
                } else {
                    Step::Ready
                });
            }
            KeyCode::Down | KeyCode::PageDown => {
                setup.scroll = setup.scroll.saturating_add(1).min(40)
            }
            KeyCode::Up | KeyCode::PageUp => setup.scroll = setup.scroll.saturating_sub(1),
            KeyCode::Char('r') if self.sender.is_some() && self.pending.is_none() => {
                self.check_session().await?;
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) async fn check_session(&mut self) -> Result<()> {
        if self
            .setup
            .as_ref()
            .and_then(|s| s.retry_at_ms)
            .is_some_and(|t| t > now_ms())
        {
            bail!("X asked us to wait. Retry after the current cooldown.");
        }
        if let Some(setup) = self.setup.as_mut() {
            setup.go(Step::Connect);
            setup.account_error = None;
        }
        self.handle.clear();
        self.submit(Command::GetSession).await?;
        self.log("Checking the signed-in X account…");
        Ok(())
    }
    fn adjust(&mut self, d: i32) {
        match self.filter_row {
            0 => {
                self.policy.inactive_days =
                    (self.policy.inactive_days as i32 + d * 10).clamp(1, 3650) as u32
            }
            1 => self.policy.skip_verified = !self.policy.skip_verified,
            2 => self.policy.skip_following = !self.policy.skip_following,
            3 => self.policy.include_zero_posts = !self.policy.include_zero_posts,
            4 => {
                self.policy.delay_seconds =
                    (self.policy.delay_seconds as i32 + d).clamp(5, 300) as u32
            }
            5 => {
                self.policy.batch_limit =
                    (self.policy.batch_limit as i32 + d * 10).clamp(1, 500) as usize
            }
            _ => {}
        }
    }
    pub async fn tick(&mut self) -> Result<()> {
        if self.mode == Mode::Setup
            || self.paused
            || self.pending.is_some()
            || self.sender.is_none()
            || Instant::now() < self.next_at
            || now_ms() < self.pacing.until_ms
            || self.uncertain > 0
        {
            return Ok(());
        }
        if !self.demo && !self.capabilities.iter().any(|c| c == "adapter:2") {
            self.paused = true;
            self.log("Chrome adapter needs an update. Reload the extension, then Save & connect.");
            return Ok(());
        }
        if !self.demo && self.scan.adapter_revision < 2 && !self.scan.phase.is_empty() {
            self.paused = true;
            self.log("Saved scan uses the old X adapter. Press s to refresh account evidence; keep choices are preserved.");
            return Ok(());
        }
        if let Some(batch) = &mut self.batch {
            let batch_id = batch.id.clone();
            let finished = self
                .store
                .run(move |s| s.finished_targets(&batch_id))
                .await?;
            batch.ids.retain(|id| !finished.contains(id));
            while let Some(id) = batch.ids.front() {
                if self
                    .accounts
                    .get(id)
                    .is_some_and(|a| !a.kept && a.follows_me != Some(false))
                {
                    break;
                }
                batch.ids.pop_front();
            }
            if let Some(target_id) = batch.ids.front().cloned() {
                let command = Command::RemoveFollower {
                    target_id,
                    batch_id: batch.id.clone(),
                    policy: batch.policy.clone(),
                    deadline_ms: now_ms() + 120_000,
                };
                self.submit(command).await?;
            } else {
                self.batch = None;
                self.paused = true;
                self.save_batch().await?;
                self.log("Removal batch finished.");
            }
            return Ok(());
        }
        match self.scan.phase.as_str() {
            "following" | "followers" => {
                self.submit(Command::ScanPage {
                    list: self.scan.phase.clone(),
                    cursor: self.scan.cursor.clone(),
                })
                .await?
            }
            "inspect" => {
                let id = self
                    .accounts
                    .values()
                    .find(|a| {
                        a.basic_candidate(&self.policy)
                            && a.checked_at_ms.unwrap_or(0) < self.scan.started_at
                    })
                    .map(|a| a.id.clone());
                if let Some(target_id) = id {
                    self.submit(Command::InspectAccount {
                        target_id,
                        policy: self.policy.clone(),
                    })
                    .await?;
                } else {
                    self.scan.phase = "done".into();
                    self.paused = true;
                    self.save_scan().await?;
                    self.log("Activity checked. f sets removal rules; m shows candidates; a selects candidates; d reviews the removal queue.");
                }
            }
            _ => self.paused = true,
        }
        Ok(())
    }
    pub async fn bridge_event(&mut self, event: BridgeEvent) -> Result<()> {
        match event {
            BridgeEvent::ExtensionVersion(version) => {
                self.browser_version = version;
            }
            BridgeEvent::Connected { session_id, sender } => {
                if self.mode == Mode::Setup {
                    self.setup.as_mut().unwrap().go(Step::Connect);
                } else {
                    self.mode = Mode::Browse;
                }
                self.handle.clear();
                self.capabilities.clear();
                self.confirmation.clear();
                self.selected.clear();
                self.sender = Some(sender);
                self.session = session_id;
                self.pending = None;
                self.paused = true;
                self.check_session().await?;
            }
            BridgeEvent::Disconnected(message) => {
                if self.mode == Mode::Setup {
                    self.setup.as_mut().unwrap().go(Step::Pair);
                } else {
                    self.mode = Mode::Browse;
                }
                self.handle.clear();
                self.confirmation.clear();
                self.sender = None;
                self.session.clear();
                self.paused = true;
                self.pending = None;
                self.refresh_counts().await?;
                self.log(message);
            }
            BridgeEvent::Message(ClientMessage::XPageReady { .. }) => {
                if self.mode == Mode::Setup
                    && self.sender.is_some()
                    && self.handle.is_empty()
                    && self
                        .setup
                        .as_ref()
                        .and_then(|s| s.retry_at_ms)
                        .is_none_or(|t| t <= now_ms())
                {
                    if self
                        .pending
                        .as_ref()
                        .is_some_and(|w| matches!(w.command, Command::GetSession))
                    {
                        if let Some(setup) = self.setup.as_mut() {
                            setup.recheck_requested = true;
                        }
                    } else if self.pending.is_none() {
                        self.check_session().await?;
                    }
                }
            }
            BridgeEvent::Message(ClientMessage::Result {
                command_id, result, ..
            }) => {
                if self
                    .pending
                    .as_ref()
                    .is_some_and(|w| w.command_id == command_id)
                {
                    let w = self.pending.take().unwrap();
                    let identity = matches!(w.command, Command::GetSession);
                    let can_retry = matches!(&result, WorkResult::Error { code, retry_at_ms: None, .. }
                        if !matches!(code.as_str(), "rate_limited" | "access_denied" | "account_changed"));
                    let queued = self.setup.as_ref().is_some_and(|s| s.recheck_requested);
                    if identity && let Some(setup) = self.setup.as_mut() {
                        setup.recheck_requested = false;
                    }
                    self.apply_result(&w, result).await?;
                    self.ack(&command_id).await?;
                    if identity && queued && can_retry && self.mode == Mode::Setup {
                        self.check_session().await?;
                    }
                }
            }
            BridgeEvent::Message(ClientMessage::Recovery { work, result, .. }) => {
                let id = work.command_id.clone();
                let stored = self.store.run(move |s| s.action(&id)).await?;
                if let Some(stored) = stored {
                    if stored.owner_id != work.owner_id {
                        bail!("Recovery owner mismatch");
                    }
                    self.apply_result(&stored, result).await?;
                    self.ack(&work.command_id).await?;
                    self.log("Recovered browser receipt. Press r for any uncertain action; p resumes a saved queue.");
                } else {
                    self.paused = true;
                    self.log("Browser has an unknown outstanding removal. Use the original data directory to recover it.");
                }
            }
            _ => {}
        }
        Ok(())
    }
    async fn ack(&self, id: &str) -> Result<()> {
        if let Some(tx) = &self.sender {
            tx.send(json!({"type":"ack","session_id":self.session,"command_id":id}))
                .await?;
        }
        Ok(())
    }
    async fn refresh_counts(&mut self) -> Result<()> {
        let owner = self.owner.clone();
        let (r, u) = self.store.run(move |s| s.action_counts(&owner)).await?;
        self.removed = r;
        self.uncertain = u;
        Ok(())
    }
    async fn apply_action(&mut self, w: &Work, result: &WorkResult) -> Result<()> {
        let (target, status, message) = match result {
            WorkResult::Action {
                target_id,
                status,
                message,
            } => (target_id.clone(), status.clone(), message.clone()),
            _ => bail!("Invalid action receipt"),
        };
        let (id, expected) = match &w.command {
            Command::Reconcile {
                original_command_id,
                target_id,
            } => (original_command_id.clone(), target_id),
            Command::RemoveFollower { target_id, .. } => (w.command_id.clone(), target_id),
            _ => bail!("Action response for non-action"),
        };
        if expected != &target {
            bail!("Action target mismatch");
        }
        if ![
            "verified_removed",
            "already_absent",
            "skipped",
            "failed",
            "uncertain",
        ]
        .contains(&status.as_str())
        {
            bail!("Unknown action status");
        }
        let (s, m) = (status.clone(), message.clone());
        self.store
            .run(move |db| db.finish_action(&id, &s, &m))
            .await?;
        if w.owner_id == self.owner {
            if status == "verified_removed" || status == "already_absent" {
                self.pacing.failures = 0;
                if let Some(a) = self.accounts.get_mut(&target) {
                    a.follows_me = Some(false);
                    let (owner, record) = (self.owner.clone(), a.clone());
                    self.store
                        .run(move |s| s.save_page(&owner, &[record], "last_result", &now_ms()))
                        .await?;
                }
                self.selected.remove(&target);
            }
            if let Some(b) = &mut self.batch {
                b.ids.retain(|id| id != &target);
            }
            self.save_batch().await?;
            if status == "uncertain" || status == "failed" {
                self.detach_requested = false;
                self.paused = true;
            }
            self.refresh_counts().await?;
            self.log(format!("{status}: {message}"));
        }
        Ok(())
    }
    async fn apply_result(&mut self, w: &Work, result: WorkResult) -> Result<()> {
        match result {
            WorkResult::Session {
                owner_id,
                handle,
                capabilities,
            } => {
                if !matches!(w.command, Command::GetSession) {
                    bail!("Unexpected session response");
                }
                if !self.demo && !capabilities.iter().any(|c| c == "adapter:2") {
                    bail!(
                        "Chrome extension is out of date. Reload forgive-me in chrome://extensions, then Save & connect."
                    );
                }
                if owner_id.is_empty() || handle.is_empty() {
                    bail!("X did not identify a signed-in account. Sign in, then retry (r).");
                }
                if self.mode != Mode::Setup {
                    self.mode = Mode::Browse;
                }
                self.confirmation.clear();
                self.paused = true;
                self.owner = owner_id;
                self.handle = clean(&handle);
                self.capabilities = capabilities;
                self.selected.clear();
                self.focus = 0;
                let owner = self.owner.clone();
                let (records, scan, batch, pacing) = self
                    .store
                    .run(move |s| {
                        s.set("last_owner", &owner)?;
                        Ok((
                            s.accounts(&owner)?,
                            s.get(&format!("scan:{owner}"))?.unwrap_or_default(),
                            s.get(&format!("batch:{owner}"))?.flatten(),
                            s.get(&format!("pacing:{owner}"))?.unwrap_or_default(),
                        ))
                    })
                    .await?;
                self.accounts = records.into_iter().map(|a| (a.id.clone(), a)).collect();
                self.scan = scan;
                self.batch = batch;
                self.pacing = pacing;
                self.refresh_counts().await?;
                if self.mode == Mode::Setup {
                    self.setup.as_mut().unwrap().go(Step::Ready);
                }
                self.log("Ready. s scans; p resumes saved work; r reconciles uncertain actions.");
            }
            WorkResult::Page {
                list,
                accounts,
                next_cursor,
                complete,
            } => {
                let Command::ScanPage {
                    list: expected,
                    cursor,
                } = &w.command
                else {
                    bail!("Unexpected page response");
                };
                if &list != expected || w.owner_id != self.owner {
                    bail!("Page owner or kind mismatch");
                }
                if next_cursor
                    .as_ref()
                    .is_some_and(|c| Some(c) == cursor.as_ref() || self.scan.cursors.contains(c))
                {
                    self.paused = true;
                    bail!("Repeated cursor; scan stopped without claiming completeness");
                }
                let mut records = vec![];
                for mut a in accounts {
                    if a.id.is_empty() {
                        bail!("Missing account identity");
                    }
                    if let Some(old) = self.accounts.get(&a.id) {
                        a.kept = old.kept;
                        if list == "followers" && self.scan.following_complete {
                            a.i_follow = old.i_follow.or(Some(false));
                        }
                    } else if list == "followers" && self.scan.following_complete {
                        a.i_follow = Some(false);
                    }
                    if list == "following" {
                        a.i_follow = Some(true);
                    } else {
                        a.follows_me = Some(true);
                    }
                    self.accounts.insert(a.id.clone(), a.clone());
                    records.push(a);
                }
                self.scan.cursor = next_cursor;
                if let Some(c) = &self.scan.cursor {
                    self.scan.cursors.insert(c.clone());
                }
                if complete {
                    self.scan.cursor = None;
                    self.scan.cursors.clear();
                    if list == "following" {
                        self.scan.following_complete = true;
                        self.scan.phase = "followers".into();
                    } else {
                        self.scan.phase = "review".into();
                        self.paused = true;
                        self.log("Followers collected. f sets removal rules; i checks activity. Nothing is selected or removed yet.");
                    }
                } else if self.scan.cursor.is_none() {
                    self.paused = true;
                    bail!("Page did not establish completion or a next cursor");
                }
                let (owner, scan) = (self.owner.clone(), self.scan.clone());
                self.store
                    .run(move |s| s.save_page(&owner, &records, &format!("scan:{owner}"), &scan))
                    .await?;
            }
            WorkResult::Account { mut account } => {
                let Command::InspectAccount { target_id, .. } = &w.command else {
                    bail!("Unexpected account response");
                };
                if target_id != &account.id || w.owner_id != self.owner {
                    bail!("Account response identity mismatch");
                }
                account.kept = self.accounts.get(&account.id).is_some_and(|a| a.kept);
                let (owner, copy) = (self.owner.clone(), account.clone());
                self.store
                    .run(move |s| s.save_page(&owner, &[copy], "last_inspection", &now_ms()))
                    .await?;
                self.accounts.insert(account.id.clone(), account);
            }
            WorkResult::Deferred {
                target_id,
                code,
                message,
                retry_at_ms,
            } => {
                let Command::RemoveFollower {
                    target_id: expected,
                    ..
                } = &w.command
                else {
                    bail!("Deferred response for non-removal");
                };
                if expected != &target_id || w.owner_id != self.owner {
                    bail!("Deferred target mismatch");
                }
                let id = w.command_id.clone();
                self.store
                    .run(move |s| {
                        s.finish_action(&id, "deferred", "No write sent; retry after cooldown")
                    })
                    .await?;
                self.pacing.retry(Some(retry_at_ms), &code);
                self.refresh_counts().await?;
                self.log(format!(
                    "Waiting {}s: {message}. No removal was sent; this target stays queued.",
                    self.pacing.remaining_seconds()
                ));
            }
            action @ WorkResult::Action { .. } => self.apply_action(w, &action).await?,
            WorkResult::Opened => {}
            WorkResult::Error {
                code,
                message,
                retry_at_ms,
            } => {
                if let Command::RemoveFollower { target_id, .. } = &w.command {
                    self.apply_action(
                        w,
                        &WorkResult::Action {
                            target_id: target_id.clone(),
                            status: "uncertain".into(),
                            message: message.clone(),
                        },
                    )
                    .await?;
                }
                let retry_read =
                    !matches!(
                        w.command,
                        Command::RemoveFollower { .. }
                            | Command::GetSession
                            | Command::Reconcile { .. }
                    ) && matches!(code.as_str(), "rate_limited" | "network_unavailable");
                if matches!(code.as_str(), "rate_limited" | "network_unavailable") {
                    self.pacing.retry(retry_at_ms, &code);
                }
                if !retry_read {
                    self.paused = true;
                    self.detach_requested = false;
                }
                if matches!(w.command, Command::GetSession)
                    && let Some(setup) = self.setup.as_mut()
                {
                    setup.account_error = Some(format!("{}: {}", clean(&code), clean(&message)));
                    setup.retry_at_ms = retry_at_ms;
                }
                let task = match &w.command {
                    Command::ScanPage { list, .. } => format!("{list} collection"),
                    Command::InspectAccount { .. } => "activity check".into(),
                    _ => "browser task".into(),
                };
                self.log(format!(
                    "{} during {task}: {code}: {}",
                    if retry_read {
                        "Waiting; retries automatically"
                    } else {
                        "Paused"
                    },
                    message.trim_end_matches('.')
                ));
                if let Some(retry) = retry_at_ms {
                    self.next_at =
                        Instant::now() + Duration::from_millis((retry - now_ms()).max(0) as u64);
                }
            }
        }
        let seconds = if matches!(w.command, Command::RemoveFollower { .. }) {
            self.batch
                .as_ref()
                .map_or(self.policy.delay_seconds, |b| b.policy.delay_seconds)
        } else {
            2
        };
        self.pacing.until_ms = self
            .pacing
            .until_ms
            .max(now_ms() + i64::from(seconds) * 1000);
        let pacing = self.pacing.clone();
        let key = format!("pacing:{}", w.owner_id);
        self.store.run(move |s| s.set(&key, &pacing)).await?;
        self.next_at = self
            .next_at
            .max(Instant::now() + Duration::from_secs(seconds.into()));
        Ok(())
    }
}
