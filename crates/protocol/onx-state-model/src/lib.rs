//! ONX state model and account lifecycle implementation.
//!
//! Implements `docs/specification/state-model.md`: account states, lifecycle transitions,
//! Cell binary serialization, domain-separated Cell representation hashing,
//! Bag-of-Cells (BoC) graphs, and Merkle proof structures.

pub mod account;
pub mod boc;
pub mod cell;
pub mod error;
pub mod storage;
pub mod tree;

pub use account::{AccountState, AccountType, StorageStat};
pub use boc::BagOfCells;
pub use cell::{Cell, MAX_CELL_DATA_BYTES, MAX_CELL_REFS, ONX_CELL_HASH_V1_TAG};
pub use error::StateModelError;
pub use storage::{BlockCommitment, StateStorage, StorageError, COLUMN_FAMILIES};
pub use tree::{MerkleProof, ShardStateTree, MERKLE_PROOF_MAGIC};
