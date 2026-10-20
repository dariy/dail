//! Typed IDs. Every ID is a newtype over [`Uuid`] and is `Copy + Ord + Serialize + JsonSchema`.
//!
//! New IDs are never drawn from an RNG: they are derived from [`Ctx::id_seed`](crate::Ctx) through
//! an [`IdGen`], so replaying the same inputs yields the same IDs.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! typed_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            /// Wraps an existing UUID.
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// The underlying UUID.
            pub const fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl From<Uuid> for $name {
            fn from(uuid: Uuid) -> Self {
                Self(uuid)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

typed_id!(
    /// A configured mail account.
    AccountId
);
typed_id!(
    /// A mailbox (IMAP folder) of one account.
    MailboxId
);
typed_id!(
    /// One message copy, i.e. one `(mailbox, uid)` pair.
    MessageId
);
typed_id!(
    /// A conversation that groups messages across mailboxes.
    ThreadId
);
typed_id!(
    /// A local draft.
    DraftId
);
typed_id!(
    /// A pending remote operation in the outbox.
    OpId
);
typed_id!(
    /// An agent-initiated request that waits for user approval.
    ApprovalId
);

/// Deterministic ID generator seeded from [`Ctx::id_seed`](crate::Ctx).
///
/// The n-th ID of a dispatch is `seed + n`, so one seed yields a stream of distinct IDs.
#[derive(Debug, Clone)]
pub struct IdGen {
    seed: u128,
    next: u64,
}

impl IdGen {
    /// Starts a new stream from `seed`.
    pub const fn new(seed: u128) -> Self {
        Self { seed, next: 0 }
    }

    /// The next UUID in the stream.
    pub fn next_uuid(&mut self) -> Uuid {
        let n = self.next;
        self.next = self.next.wrapping_add(1);
        Uuid::from_u128(self.seed.wrapping_add(u128::from(n)))
    }

    /// The next ID of type `T` in the stream.
    pub fn next_id<T: From<Uuid>>(&mut self) -> T {
        T::from(self.next_uuid())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_gen_is_deterministic_and_distinct() {
        let mut a = IdGen::new(42);
        let mut b = IdGen::new(42);
        let first: ThreadId = a.next_id();
        let second: ThreadId = a.next_id();
        assert_ne!(first, second);
        assert_eq!(first, b.next_id::<ThreadId>());
        assert_eq!(second, b.next_id::<ThreadId>());
    }

    #[test]
    fn ids_serialize_as_plain_uuid_strings() {
        let id = AccountId::from_uuid(Uuid::from_u128(1));
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"00000000-0000-0000-0000-000000000001\"");
        assert_eq!(serde_json::from_str::<AccountId>(&json).unwrap(), id);
        assert_eq!(id.to_string(), "00000000-0000-0000-0000-000000000001");
    }
}
