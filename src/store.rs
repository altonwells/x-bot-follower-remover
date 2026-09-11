use crate::{
    model::Account,
    protocol::{Command, Work},
};
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;

pub struct Store {
    pub conn: Connection,
}
#[derive(Clone)]
pub struct Storage(std::sync::Arc<std::sync::Mutex<Store>>);
impl Storage {
    pub fn new(store: Store) -> Self {
        Self(std::sync::Arc::new(std::sync::Mutex::new(store)))
    }
    pub async fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Store) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let inner = self.0.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = inner
                .lock()
                .map_err(|_| anyhow::anyhow!("Storage lock poisoned"))?;
            f(&mut guard)
        })
        .await?
    }
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;
          CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS accounts(owner TEXT NOT NULL,id TEXT NOT NULL,data TEXT NOT NULL,kept INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(owner,id));
          CREATE TABLE IF NOT EXISTS actions(command_id TEXT PRIMARY KEY,owner TEXT NOT NULL,target TEXT NOT NULL,batch TEXT NOT NULL,state TEXT NOT NULL,work TEXT NOT NULL,message TEXT NOT NULL DEFAULT '');
          PRAGMA user_version=1;")?;
        Ok(Self { conn })
    }
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let raw: Option<String> = self
            .conn
            .query_row("SELECT value FROM settings WHERE key=?", [key], |r| {
                r.get(0)
            })
            .optional()?;
        raw.map(|s| Ok(serde_json::from_str(&s)?)).transpose()
    }
    pub fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings VALUES(?,?)",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }
    pub fn accounts(&self, owner: &str) -> Result<Vec<Account>> {
        let mut q = self
            .conn
            .prepare("SELECT data,kept FROM accounts WHERE owner=? ORDER BY id")?;
        let rows = q.query_map([owner], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?))
        })?;
        let mut out = vec![];
        for row in rows {
            let (s, k) = row?;
            let mut a: Account = serde_json::from_str(&s)?;
            a.kept = k;
            out.push(a);
        }
        Ok(out)
    }
    pub fn save_page<T: Serialize>(
        &mut self,
        owner: &str,
        accounts: &[Account],
        key: &str,
        progress: &T,
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        for a in accounts {
            tx.execute("INSERT INTO accounts(owner,id,data) VALUES(?,?,?) ON CONFLICT(owner,id) DO UPDATE SET data=excluded.data",params![owner,a.id,serde_json::to_string(a)?])?;
        }
        tx.execute(
            "INSERT OR REPLACE INTO settings VALUES(?,?)",
            params![key, serde_json::to_string(progress)?],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn keep(&self, owner: &str, id: &str, kept: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE accounts SET kept=? WHERE owner=? AND id=?",
            params![kept, owner, id],
        )?;
        Ok(())
    }
    pub fn start_action(&self, work: &Work) -> Result<()> {
        let Command::RemoveFollower {
            target_id: target,
            batch_id: batch,
            ..
        } = &work.command
        else {
            anyhow::bail!("Only removals have durable action receipts");
        };
        self.conn.execute("INSERT INTO actions(command_id,owner,target,batch,state,work) VALUES(?,?,?,?,'dispatched',?)",params![work.command_id,work.owner_id,target,batch,serde_json::to_string(work)?])?;
        Ok(())
    }
    pub fn action(&self, id: &str) -> Result<Option<Work>> {
        let row: Option<String> = self
            .conn
            .query_row("SELECT work FROM actions WHERE command_id=?", [id], |r| {
                r.get(0)
            })
            .optional()?;
        row.map(|s| Ok(serde_json::from_str(&s)?)).transpose()
    }
    pub fn finish_action(&self, id: &str, status: &str, message: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE actions SET state=?,message=? WHERE command_id=? AND state IN ('dispatched','uncertain')",
            params![status, message, id],
        )?;
        Ok(())
    }
    pub fn unresolved(&self, owner: &str) -> Result<Vec<Work>> {
        let mut q=self.conn.prepare("SELECT work FROM actions WHERE owner=? AND state IN ('dispatched','uncertain') ORDER BY rowid")?;
        let rows = q.query_map([owner], |r| r.get::<_, String>(0))?;
        rows.map(|s| Ok(serde_json::from_str(&s?)?)).collect()
    }
    pub fn finished_targets(&self, batch: &str) -> Result<Vec<String>> {
        let mut q = self.conn.prepare(
            "SELECT target FROM actions WHERE batch=? AND state NOT IN ('dispatched','uncertain','deferred')",
        )?;
        Ok(q.query_map([batch], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }
    pub fn action_counts(&self, owner: &str) -> Result<(usize, usize)> {
        Ok((
            self.conn.query_row(
                "SELECT COUNT(*) FROM actions WHERE owner=? AND state='verified_removed'",
                [owner],
                |r| r.get::<_, i64>(0),
            )? as usize,
            self.unresolved(owner)?.len(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keep_survives_refresh_and_owners_are_isolated() {
        let mut s = Store::open(Path::new(":memory:")).unwrap();
        let a = Account {
            id: "123".into(),
            ..Default::default()
        };
        s.save_page("one", std::slice::from_ref(&a), "scan", &1)
            .unwrap();
        s.keep("one", "123", true).unwrap();
        s.save_page("one", std::slice::from_ref(&a), "scan", &2)
            .unwrap();
        s.save_page("two", &[a], "scan2", &1).unwrap();
        assert!(s.accounts("one").unwrap()[0].kept);
        assert!(!s.accounts("two").unwrap()[0].kept);
    }
    #[test]
    fn duplicate_dispatch_rejected_and_uncertain_survives_reopen() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("db");
        let w = Work {
            command_id: "same".into(),
            owner_id: "owner".into(),
            command: Command::RemoveFollower {
                target_id: "123".into(),
                batch_id: "batch".into(),
                policy: crate::model::Policy::default(),
                deadline_ms: crate::model::now_ms() + 120_000,
            },
        };
        {
            let s = Store::open(&p).unwrap();
            s.start_action(&w).unwrap();
            assert!(s.start_action(&w).is_err());
        }
        assert_eq!(
            Store::open(&p).unwrap().unresolved("owner").unwrap().len(),
            1
        );
    }
}
