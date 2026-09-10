//! Double-delivery prevention via tracked message representation hashes,
//! per `docs/specification/transactions.md` §3.5 and §5 rule 5.

use crate::error::TransactionsError;
use onx_data_structures::Message;
use std::collections::BTreeSet;

/// Tracks recently delivered message representation hashes
/// (`processed_msg_hashes`) for an account or shardchain state, rejecting
/// re-delivery of a message whose hash is already recorded.
///
/// Uses a `BTreeSet` rather than a hash-based set so iteration order (e.g.
/// when this tracker is itself serialized into consensus state) is
/// deterministic across validators.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessedMessageTracker {
    hashes: BTreeSet<[u8; 32]>,
}

impl ProcessedMessageTracker {
    /// Creates an empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks `message_hash` against the tracked set and records it if
    /// absent. Rejects with `DuplicateDelivery` if the hash is already
    /// present, per §5 rule 5.
    pub fn admit(&mut self, message_hash: [u8; 32]) -> Result<(), TransactionsError> {
        if !self.hashes.insert(message_hash) {
            return Err(TransactionsError::DuplicateDelivery);
        }
        Ok(())
    }

    /// Convenience wrapper over [`Self::admit`] that computes `message`'s
    /// domain-separated representation hash (`ONX_MSG_HASH_V1`) itself.
    pub fn admit_message(&mut self, message: &Message) -> Result<(), TransactionsError> {
        self.admit(message.message_hash())
    }

    /// Returns true if `message_hash` has already been recorded as delivered.
    pub fn contains(&self, message_hash: &[u8; 32]) -> bool {
        self.hashes.contains(message_hash)
    }

    /// Prunes `message_hash` from the tracked set once the originating shard
    /// confirms removal from its output queue via a masterchain reference,
    /// per §3.5 rule 2. A no-op if the hash isn't tracked.
    pub fn prune(&mut self, message_hash: &[u8; 32]) {
        self.hashes.remove(message_hash);
    }

    /// Number of currently tracked message hashes.
    pub fn len(&self) -> usize {
        self.hashes.len()
    }

    /// True if no message hashes are currently tracked.
    pub fn is_empty(&self) -> bool {
        self.hashes.is_empty()
    }
}
