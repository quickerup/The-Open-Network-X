//! ONX transaction and messaging semantics.
//!
//! Implements `docs/specification/transactions.md` (ADR-0005): external and
//! internal message admission rules including tentative execution of
//! External Inbound messages, `extra_currencies` value-model validation,
//! the output-queue-only delivery architecture with per-account FIFO
//! ordering, and double-delivery prevention via `processed_msg_hashes`.
//!
//! Hypercube slow-path routing (§3.4) is implemented as deterministic shard
//! path planning; fast-path proof relay remains a node-runtime concern.

pub mod admission;
pub mod error;
pub mod output_queue;
pub mod processed;
pub mod router;

pub use admission::{
    admit_external_inbound, admit_external_outbound, admit_internal,
    validate_extra_currencies_sorted, validate_message_shape, TentativeExecutionOutcome,
    MAX_TENTATIVE_GAS,
};
pub use error::TransactionsError;
pub use output_queue::OutputQueue;
pub use processed::ProcessedMessageTracker;
pub use router::{
    are_neighbors, cross_workchain_expiry, is_cross_workchain_expired, plan_hypercube_route,
    validate_hypercube_route, RouteHop, RoutePlan, MAX_CROSS_WORKCHAIN_LT_WINDOW,
};
