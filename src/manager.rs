//! Browser manager requests share the authenticated bridge and the App controller.
use crate::{
    app::{App, Mode},
    model::{Account, Policy, now_ms},
    protocol::Command,
};
use anyhow::{Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    #[serde(default)]
    pub search: String,
    #[serde(default)]
    pub filter: String,
    #[serde(default)]
    pub page: usize,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Snapshot {
        #[serde(default)]
        query: Query,
    },
    Collect,
    Inspect,
    Start {
        confirmed: bool,
    },
    Pause,
    Resume,
    Cancel,
    Keep {
        target_id: String,
        kept: bool,
    },
    Settings {
        policy: Policy,
    },
}
#[derive(Serialize)]
pub struct Row<'a> {
    pub account: &'a Account,
    pub decision: String,
    pub reason: String,
    pub queued: bool,
    pub working: bool,
    pub retry_at_ms: Option<i64>,
}
impl App {
    pub async fn manager_request(
        &mut self,
        request_id: String,
        owner_id: String,
        action: Action,
        controls_allowed: bool,
    ) -> Result<()> {
        let result = async {
            ensure!(request_id.len() <= 80, "Invalid request ID");
            ensure!(self.sender.is_some() && !self.handle.is_empty(), "Connect X before opening your follower list");
            ensure!(owner_id.is_empty() && matches!(action, Action::Snapshot {..}) || owner_id == self.owner, "Account changed. Refresh before continuing.");
            if let Action::Snapshot { query } = action { return self.manager_snapshot(query).await; }
            ensure!(controls_allowed, "Account changed. Reconnect the account approved for this worker.");
            match action {
                Action::Collect => {
                    ensure!(self.auto_policy.is_none(), "Cancel the current cleanup before collecting again");
                    ensure!(self.pending.is_none(), "Wait for the current task before collecting again");
                    if !matches!(self.scan.phase.as_str(), "following"|"followers") { self.scan.phase.clear(); }
                    self.mode = Mode::Browse;
                    self.start_scan().await?;
                }
                Action::Inspect => { self.start_activity().await?; self.mode = Mode::Browse; }
                Action::Start { confirmed } => {
                    ensure!(confirmed, "Confirm the cleanup rule first");
                    self.begin_cleanup().await?;
                }
                Action::Pause => { self.save_managed(false).await?; self.pause().await?; }
                Action::Resume => {
                    ensure!(self.auto_policy.is_some() || self.batch.is_some() || matches!(self.scan.phase.as_str(), "following"|"followers"|"inspect"), "No work to resume");
                    ensure!(self.uncertain == 0 || self.simple_running(), "Resolve uncertain removals in the terminal first");
                    self.save_managed(true).await?;
                    self.send_control("resume").await?;
                    self.paused = false;
                    self.mode = Mode::Browse;
                }
                Action::Cancel => {
                    // This goes through the same cancellation path as the terminal.
                    self.mode = Mode::Browse;
                    self.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)).await?;
                    self.scan.phase = "done".into();
                    self.save_scan().await?;
                }
                Action::Keep { target_id, kept } => {
                    ensure!(self.accounts.contains_key(&target_id), "Account no longer in this inventory");
                    ensure!(!self.pending.as_ref().is_some_and(|w| matches!(&w.command, Command::RemoveFollower {target_id: id,..} if id == &target_id)), "Removal already in flight. Its result must arrive before this account can be changed.");
                    let owner = self.owner.clone(); let id = target_id.clone();
                    self.store.run(move |s| s.keep(&owner, &id, kept)).await?;
                    self.accounts.get_mut(&target_id).unwrap().kept = kept;
                    self.selected.remove(&target_id);
                    if kept {
                        if let Some(batch) = &mut self.batch { batch.ids.retain(|id| id != &target_id); }
                        self.save_batch().await?;
                    }
                    self.log(if kept { "Account marked Keep. It is excluded from pending removals." } else { "Keep exception removed. The standard rules still apply." });
                }
                Action::Settings { policy } => {
                    ensure!((5..=300).contains(&policy.delay_seconds) && (1..=100).contains(&policy.rest_every) && policy.rest_seconds <= 3600 && (1..=500).contains(&policy.batch_limit), "Settings out of range");
                    self.policy.copy_pacing(&policy);
                    self.save_processing().await?;
                }
                Action::Snapshot {..} => unreachable!(),
            }
            Ok(json!({"accepted":true}))
        }.await;
        let reply = match result {
            Ok(data) => {
                json!({"type":"manager_result","session_id":self.session,"request_id":request_id,"ok":true,"data":data})
            }
            Err(error) => {
                json!({"type":"manager_result","session_id":self.session,"request_id":request_id,"ok":false,"error":format!("{error:#}")})
            }
        };
        if let Some(sender) = &self.sender {
            sender.send(reply).await?;
        }
        Ok(())
    }
    pub async fn manager_snapshot(&self, query: Query) -> Result<Value> {
        ensure!(
            query.search.len() <= 200 && query.page <= 1_000_000,
            "Search too large"
        );
        ensure!(
            ["", "all", "keep", "remove", "review", "removed", "queue"]
                .contains(&query.filter.as_str()),
            "Unknown view"
        );
        let owner = self.owner.clone();
        let receipts: BTreeMap<String, String> = self
            .store
            .run(move |s| {
                let mut q = s
                    .conn
                    .prepare("SELECT target,state FROM actions WHERE owner=? ORDER BY rowid")?;
                let rows = q.query_map([owner], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })?;
                Ok(rows.collect::<rusqlite::Result<BTreeMap<_, _>>>()?)
            })
            .await?;
        let queued: BTreeSet<&str> = self
            .batch
            .as_ref()
            .map(|b| b.ids.iter().map(String::as_str).collect())
            .unwrap_or_default();
        let active_id = self.pending.as_ref().and_then(|w| match &w.command {
            Command::InspectAccount { target_id, .. }
            | Command::RemoveFollower { target_id, .. }
            | Command::Reconcile { target_id, .. } => Some(target_id.as_str()),
            _ => None,
        });
        let active_kind = self.pending.as_ref().map(|w| match w.command {
            Command::GetSession => "Connecting X",
            Command::ScanPage { .. } => "Loading followers",
            Command::InspectAccount { .. } => "Checking activity",
            Command::RemoveFollower { .. } => "Removing follower",
            Command::Reconcile { .. } => "Checking removal result",
            Command::OpenProfile { .. } => "Opening profile",
        });
        let now = now_ms();
        let mut rows = Vec::new();
        let mut counts = BTreeMap::<String, usize>::from_iter(
            ["all", "keep", "remove", "review", "removed", "queue"].map(|k| (k.into(), 0)),
        );
        for account in self.accounts.values() {
            let receipt = receipts.get(&account.id).map(String::as_str);
            if account.follows_me != Some(true) && receipt.is_none() {
                continue;
            }
            let (decision, reason) = if matches!(receipt, Some("dispatched" | "uncertain")) {
                ("review", "Removal outcome needs reconciliation".into())
            } else if account.follows_me != Some(true) && receipt == Some("verified_removed") {
                ("removed", "Removal confirmed by X".into())
            } else if account.follows_me == Some(false) {
                ("keep", "Already absent from your follower list".into())
            } else if let Ok(reason) = account.approved_reason(&self.policy, now) {
                ("remove", reason.into())
            } else if account.kept
                || account.verified == Some(true)
                || account.i_follow == Some(true)
                || account.protected == Some(true)
                || account
                    .last_activity_ms
                    .is_some_and(|t| t > now - i64::from(self.policy.inactive_days) * 86_400_000)
            {
                (
                    "keep",
                    account
                        .approved_reason(&self.policy, now)
                        .unwrap_err()
                        .into(),
                )
            } else {
                (
                    "review",
                    account
                        .approved_reason(&self.policy, now)
                        .unwrap_err()
                        .into(),
                )
            };
            let in_queue = queued.contains(account.id.as_str());
            *counts.get_mut("all").unwrap() += 1;
            *counts.get_mut(decision).unwrap() += 1;
            if in_queue {
                *counts.get_mut("queue").unwrap() += 1;
            }
            rows.push(Row {
                account,
                decision: decision.into(),
                reason,
                queued: in_queue,
                working: active_id == Some(account.id.as_str()),
                retry_at_ms: self.retries.get(&account.id).map(|r| r.due_ms),
            });
        }
        let search = query.search.to_lowercase();
        rows.retain(|r| {
            (query.filter.is_empty()
                || query.filter == "all"
                || query.filter == r.decision
                || query.filter == "queue" && r.queued)
                && (search.is_empty()
                    || r.account.handle.to_lowercase().contains(&search)
                    || r.account.name.to_lowercase().contains(&search))
        });
        rows.sort_by_cached_key(|r| (r.account.handle.to_lowercase(), r.account.id.clone()));
        let total = rows.len();
        let page = query.page.min(total.saturating_sub(1) / 50);
        let rows: Vec<_> = rows.into_iter().skip(page * 50).take(50).collect();
        let active = active_id
            .and_then(|id| self.accounts.get(id))
            .map(|a| json!({"id":a.id,"handle":a.handle,"name":a.name}));
        Ok(
            json!({"owner_id":self.owner,"handle":self.handle,"generated_at_ms":now,"rows":rows,"counts":counts,"total":total,"page":page,"page_size":50,"phase":self.scan.phase,"paused":self.paused,"running":self.auto_policy.is_some()||self.batch.is_some(),"pending":self.pending.is_some(),"active":active,"active_kind":active_kind,"message":self.notice,"policy":self.policy,"cooldown_until_ms":self.pacing.until_ms,"cooldown_reason":self.pacing.reason,"attempts_this_hour":self.pacing.attempts.iter().filter(|t|**t>now-3_600_000).count(),"checked":self.accounts.values().filter(|a|a.checked_at_ms.is_some()).count(),"removed_total":self.removed,"uncertain":self.uncertain,"scan_complete":self.scan.following_complete&&matches!(self.scan.phase.as_str(),"review"|"inspect"|"done")}),
        )
    }
}
