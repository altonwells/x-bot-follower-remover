use crate::model::{Account, Policy};
use serde::{Deserialize, Serialize};

pub const VERSION: u8 = 1;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    GetSession,
    ScanPage {
        list: String,
        cursor: Option<String>,
    },
    InspectAccount {
        target_id: String,
        policy: Policy,
    },
    RemoveFollower {
        target_id: String,
        batch_id: String,
        policy: Policy,
        deadline_ms: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        approved_account: Option<Box<Account>>,
    },
    Reconcile {
        target_id: String,
        original_command_id: String,
    },
    OpenProfile {
        target_id: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Work {
    pub command_id: String,
    pub owner_id: String,
    pub command: Command,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkResult {
    Session {
        owner_id: String,
        handle: String,
        capabilities: Vec<String>,
    },
    Page {
        list: String,
        accounts: Vec<Account>,
        next_cursor: Option<String>,
        complete: bool,
    },
    Account {
        account: Account,
    },
    Action {
        target_id: String,
        status: String,
        message: String,
    },
    Deferred {
        target_id: String,
        code: String,
        message: String,
        retry_at_ms: i64,
    },
    Opened,
    Error {
        code: String,
        message: String,
        retry_at_ms: Option<i64>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Manager {
        session_id: String,
        request_id: String,
        owner_id: String,
        action: crate::manager::Action,
    },
    Hello {
        v: u8,
        token: String,
        extension_id: String,
        #[serde(default)]
        extension_version: Option<String>,
    },
    Heartbeat {
        session_id: String,
    },
    XPageReady {
        session_id: String,
    },
    Result {
        session_id: String,
        command_id: String,
        result: WorkResult,
    },
    Recovery {
        session_id: String,
        work: Work,
        result: WorkResult,
    },
}
