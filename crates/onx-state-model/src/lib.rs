//! ONX protocol state model crate.

pub mod account;
pub mod boc;
pub mod cell;
pub mod error;
pub mod proof;
pub mod tree;

pub use account::{AccountState, AccountStateRecord, AccountStatus};
pub use boc::{BagOfCells, BOC_MAGIC};
pub use cell::{Cell, MAX_CELL_DATA_LEN, MAX_CELL_REFS, ONX_CELL_HASH_V1_TAG};
pub use error::StateModelError;
pub use proof::{MerkleProof, MERKLE_PROOF_MAGIC};
pub use tree::{MerkleNode, ShardStateTree, ONX_SHARD_TREE_NODE_TAG};
