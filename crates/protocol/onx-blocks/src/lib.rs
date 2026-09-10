//! ONX block structural validity, masterchain coupling, and split/merge
//! announcement flags.
//!
//! Implements `docs/specification/blocks.md` (ADR-0006) and ADR-0016:
//! structural validity of ordinary and merge successor blocks (§3.1, §3.2),
//! masterchain coupling and canonicality via the `Masterchain Block Extra`
//! shard-configuration commitment (§3.3, §4.1), and the split/merge
//! announcement flag bit layout and its own well-formedness rules (§3.4, §4.2).

pub mod error;
pub mod flags;
pub mod masterchain;
pub mod sync;
pub mod validity;

pub use error::BlocksError;
pub use masterchain::{validate_master_ref, MasterchainBlockExtra, ShardEntry};
pub use sync::{BlockSyncEngine, SyncCandidate, SyncError};
pub use validity::{validate_block_successor, validate_ordinary_successor, RecomputedRoots};
