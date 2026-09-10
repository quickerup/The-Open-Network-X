//! ONX canonical data structures.
//!
//! Implements `docs/specification/data-structures.md`: workchain,
//! account, and shard identifiers, messages, block headers, and canonical
//! binary layouts.

pub mod address;
pub mod block;
pub mod error;
pub mod message;
pub mod shard;

pub use address::{AccountId, FullAddress, WorkchainIdent};
pub use block::{BlockHeader, BLOCK_HEADER_MAGIC, MERGE_RESULT_FLAG};
pub use error::DataStructureError;
pub use message::{Message, MessageType};
pub use shard::ShardIdent;
