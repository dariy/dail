//! `ApprovalRequest`: an agent's request to send a draft, waiting for the user
//! (`ARCHITECTURE.md §9`, `AGENTS.md §4` "Outbound policy").

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ctx::Origin;
use crate::ids::{AccountId, ApprovalId, DraftId};

/// `send_draft` from an agent (`Origin::Mcp`) records one of these instead of sending. The
/// user approves it (`approve_request`, which sends the draft as it is then) or rejects it
/// (`reject_request`). It goes away with its draft, and when the agent changes the draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ApprovalRequest {
    /// Identifier.
    pub id: ApprovalId,
    /// The draft the agent wants to send.
    pub draft: DraftId,
    /// The draft's account.
    pub account: AccountId,
    /// Who asked.
    pub origin: Origin,
    /// When the request was made, in Unix milliseconds.
    pub requested_ms: i64,
}
