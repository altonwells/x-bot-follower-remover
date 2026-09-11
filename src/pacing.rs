use crate::model::{Policy, now_ms};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Pacing {
    #[serde(default)]
    pub attempts: Vec<i64>,
    pub until_ms: i64,
    pub failures: u32,
    pub reason: String,
    #[serde(default)]
    pub since_rest: u32,
}
impl Pacing {
    pub fn retry(&mut self, server_ms: Option<i64>, reason: &str) {
        self.failures = (self.failures + 1).min(8);
        let fallback = 30_000_i64 * 2_i64.pow(self.failures - 1);
        self.until_ms = self
            .until_ms
            .max(now_ms() + fallback.min(3_600_000))
            .max(server_ms.unwrap_or(0));
        self.reason = reason.into();
    }
    pub fn reserve(&mut self, limit: usize) -> bool {
        let now = now_ms();
        self.attempts.retain(|t| *t > now - 3_600_000);
        if self.attempts.len() >= limit.max(1) {
            self.until_ms = self.until_ms.max(self.attempts[0] + 3_600_000);
            self.reason = "Hourly attempt budget".into();
            return false;
        }
        self.attempts.push(now);
        self.since_rest = self.since_rest.saturating_add(1);
        if self.until_ms <= now {
            self.reason.clear();
        }
        true
    }
    pub fn after_attempt(&mut self, policy: &Policy) {
        if policy.rest_every > 0 && self.since_rest >= policy.rest_every {
            self.since_rest = 0;
            let until = now_ms() + i64::from(policy.rest_seconds) * 1000;
            if until > self.until_ms {
                self.until_ms = until;
                self.reason = "Batch rest".into();
            }
        }
    }
    pub fn remaining_seconds(&self) -> i64 {
        ((self.until_ms - now_ms()).max(0) + 999) / 1000
    }
}
