use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn new_id() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("operating system randomness unavailable");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Policy {
    pub inactive_days: u32,
    pub skip_verified: bool,
    pub skip_following: bool,
    pub include_zero_posts: bool,
    pub delay_seconds: u32,
    pub batch_limit: usize,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            inactive_days: 90,
            skip_verified: true,
            skip_following: true,
            include_zero_posts: true,
            delay_seconds: 60,
            batch_limit: 50,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub handle: String,
    pub name: String,
    pub bio: String,
    pub followers: Option<u64>,
    pub following_count: Option<u64>,
    pub posts: Option<u64>,
    pub verified: Option<bool>,
    pub protected: Option<bool>,
    pub follows_me: Option<bool>,
    pub i_follow: Option<bool>,
    pub last_activity_ms: Option<i64>,
    pub coverage_since_ms: Option<i64>,
    pub checked_at_ms: Option<i64>,
    pub observed_at_ms: i64,
    pub activity_note: String,
    #[serde(default)]
    pub kept: bool,
}

impl Account {
    pub fn basic_candidate(&self, p: &Policy) -> bool {
        !self.kept
            && self.follows_me == Some(true)
            && (!p.skip_verified || self.verified != Some(true))
            && (!p.skip_following || self.i_follow != Some(true))
    }

    pub fn basic_reason(&self, p: &Policy) -> Result<(), &'static str> {
        if self.kept {
            return Err("Kept");
        }
        if self.follows_me != Some(true) {
            return Err("Follower relationship unknown / absent");
        }
        if p.skip_verified && self.verified != Some(false) {
            return Err(if self.verified == Some(true) {
                "Verified"
            } else {
                "Verification not checked"
            });
        }
        if p.skip_following && self.i_follow != Some(false) {
            return Err(if self.i_follow == Some(true) {
                "You follow this account"
            } else {
                "Following relationship unknown"
            });
        }
        if self.protected != Some(false) {
            return Err("Protected / visibility unknown");
        }
        Ok(())
    }

    pub fn reason(&self, p: &Policy, now: i64) -> Result<&'static str, &'static str> {
        self.basic_reason(p)?;
        let Some(checked) = self.checked_at_ms else {
            return Err("Needs activity check");
        };
        if checked > now + 60_000 || now - checked > 86_400_000 {
            return Err("Activity evidence expired");
        }
        if p.include_zero_posts && self.posts == Some(0) {
            return Ok("Zero current posts");
        }
        let cutoff = now - i64::from(p.inactive_days) * 86_400_000;
        if self.last_activity_ms.is_some_and(|t| t > cutoff) {
            return Err("Recently active");
        }
        if self.coverage_since_ms.is_some_and(|t| t <= cutoff) {
            return Ok("Inactive");
        }
        Err("Activity unknown")
    }
}

pub fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| {
            !c.is_control() && !matches!(*c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .take(4000)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn candidate() -> Account {
        Account {
            id: "9007199254740993".into(),
            follows_me: Some(true),
            i_follow: Some(false),
            verified: Some(false),
            protected: Some(false),
            posts: Some(4),
            checked_at_ms: Some(20_000_000_000),
            coverage_since_ms: Some(0),
            ..Default::default()
        }
    }
    #[test]
    fn excludes_unknown_mutual_verified_and_kept() {
        let p = Policy::default();
        let mut a = candidate();
        assert!(a.reason(&p, 20_000_000_000).is_ok());
        a.i_follow = None;
        assert!(a.reason(&p, 20_000_000_000).is_err());
        a.i_follow = Some(false);
        a.verified = Some(true);
        assert!(a.reason(&p, 20_000_000_000).is_err());
        a.verified = Some(false);
        a.kept = true;
        assert!(a.reason(&p, 20_000_000_000).is_err());
    }
    #[test]
    fn failed_or_stale_activity_never_qualifies() {
        let p = Policy::default();
        let mut a = candidate();
        a.coverage_since_ms = None;
        assert!(a.reason(&p, 20_000_000_000).is_err());
        a.posts = Some(0);
        assert!(a.reason(&p, 20_000_000_000).is_ok());
        a.checked_at_ms = None;
        assert!(a.reason(&p, 20_000_000_000).is_err());
    }
    #[test]
    fn recent_repost_protects_account() {
        let mut a = candidate();
        a.last_activity_ms = Some(19_999_999_999);
        assert_eq!(
            a.reason(&Policy::default(), 20_000_000_000),
            Err("Recently active")
        );
    }
    #[test]
    fn terminal_controls_are_removed() {
        assert!(!clean("\x1b]52;evil\x07").contains('\x1b'));
    }
}
