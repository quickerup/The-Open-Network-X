//! ONX state model crate.
//!
//! Implements `docs/specification/state-model.md`: account lifecycle,
//! cell trees, Bag-of-Cells (BoC), shard state trees, and Merkle proofs.

pub mod account;
pub mod cell;
pub mod error;
pub mod shard_tree;

pub use account::{AccountState, AccountStateKind};
pub use cell::{BoC, Cell, CellHash, CELL_HASH_DOMAIN_TAG, CELL_MAX_DATA_BYTES, CELL_MAX_REFS};
pub use error::StateModelError;
pub use shard_tree::{MerkleProof, ShardStateTree, MERKLE_PROOF_MAGIC};
