//! Output-queue-only delivery architecture with per-account FIFO ordering,
//! per `docs/specification/transactions.md` §3.3 and §5 rule 6.

use crate::error::TransactionsError;
use onx_data_structures::{FullAddress, Message};
use std::collections::{BTreeMap, VecDeque};

type Lane = VecDeque<Message>;
type LaneKey = (FullAddress, FullAddress);

/// A shardchain output queue (`out_queue`) per §3.3. Shardchains have no
/// persistent input queues — inbound messages are executed on block
/// inclusion, and anything that doesn't fit stays here, in the originating
/// shard's output queue, until delivered.
///
/// Messages are kept in per-`(src_address, dest_address)` FIFO lanes, using
/// a `BTreeMap` (rather than a hash map) so lane iteration order — and thus
/// which message [`Self::pop_next`] selects on a logical-time tie — is
/// deterministic across validators.
#[derive(Debug, Clone, Default)]
pub struct OutputQueue {
    lanes: BTreeMap<LaneKey, Lane>,
}

impl OutputQueue {
    /// Creates an empty output queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueues `message`, rejecting it with `LogicalTimeRegression` if its
    /// `created_lt` does not strictly exceed the last message already
    /// queued for the same `(src_address, dest_address)` pair — the
    /// invariant that makes per-pair FIFO delivery order well-defined
    /// (§3.3 rule 2).
    pub fn enqueue(&mut self, message: Message) -> Result<(), TransactionsError> {
        let key = (message.src_address, message.dest_address);
        let lane = self.lanes.entry(key).or_default();
        if let Some(last) = lane.back() {
            if message.created_lt.0 <= last.created_lt.0 {
                return Err(TransactionsError::LogicalTimeRegression {
                    previous_lt: last.created_lt.0,
                    next_lt: message.created_lt.0,
                });
            }
        }
        lane.push_back(message);
        Ok(())
    }

    /// Returns the next message due for delivery without removing it: the
    /// lowest-`created_lt` message at the head of any lane. Because each
    /// lane is already internally ordered by strictly increasing
    /// `created_lt` (enforced at enqueue time), always selecting the global
    /// minimum head also satisfies the block logical-time ordering
    /// invariant (§3.3 rule 1) without starving any lane.
    pub fn peek_next(&self) -> Option<&Message> {
        self.lanes
            .values()
            .filter_map(|lane| lane.front())
            .min_by_key(|message| message.created_lt.0)
    }

    /// Removes and returns the next message due for delivery (see
    /// [`Self::peek_next`]), pruning its lane if that empties it.
    pub fn pop_next(&mut self) -> Option<Message> {
        let key = *self
            .lanes
            .iter()
            .filter_map(|(key, lane)| lane.front().map(|message| (key, message.created_lt.0)))
            .min_by_key(|(_, lt)| *lt)
            .map(|(key, _)| key)?;
        let lane = self.lanes.get_mut(&key)?;
        let message = lane.pop_front();
        if lane.is_empty() {
            self.lanes.remove(&key);
        }
        message
    }

    /// Validates that `sequence` — an already-delivered or proposed delivery
    /// order drawn from one or more output queues — does not violate strict
    /// per-`(src_address, dest_address)` FIFO ordering: `created_lt` must
    /// strictly increase each time the same pair recurs (§5 rule 6). Used to
    /// check a delivery order presented by an untrusted source (e.g. a block
    /// candidate) rather than one built via [`Self::enqueue`]/[`Self::pop_next`].
    pub fn validate_delivery_order(sequence: &[Message]) -> Result<(), TransactionsError> {
        let mut last_lt_per_pair: BTreeMap<LaneKey, u64> = BTreeMap::new();
        for message in sequence {
            let key = (message.src_address, message.dest_address);
            if let Some(&previous_lt) = last_lt_per_pair.get(&key) {
                if message.created_lt.0 <= previous_lt {
                    return Err(TransactionsError::FifoOrderViolation {
                        previous_lt,
                        next_lt: message.created_lt.0,
                    });
                }
            }
            last_lt_per_pair.insert(key, message.created_lt.0);
        }
        Ok(())
    }

    /// True if no lane holds any messages.
    pub fn is_empty(&self) -> bool {
        self.lanes.is_empty()
    }

    /// Total number of messages queued across all lanes.
    pub fn len(&self) -> usize {
        self.lanes.values().map(VecDeque::len).sum()
    }
}
