use crate::model::now_ms;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Pacing {
    #[serde(default)]
    pub attempts: Vec<i64>,
    pub until_ms: i64,
    pub failures: u32,
    pub reason: String,
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
        true
    }
    pub fn remaining_seconds(&self) -> i64 {
        ((self.until_ms - now_ms()).max(0) + 999) / 1000
    }
}
