//! Error types for block structural validity, masterchain coupling, and
//! split/merge flag validation per `docs/specification/blocks.md` §5.

use onx_data_structures::DataStructureError;
use std::fmt;

/// Errors arising from block validity, masterchain coupling, or split/merge
/// flag checks per `docs/specification/blocks.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlocksError {
    /// Wrapper for underlying data-structure decode/validation errors.
    DataStructure(DataStructureError),
    /// §5 rule 1: the declared parent reference does not match the resolved
    /// parent block actually supplied.
    UnresolvableParent,
    /// §5 rule 2: `seq_no` is not exactly one greater than the parent's.
    SequenceDiscontinuity { expected: u32, actual: u32 },
    /// §5 rule 3: `gen_utime` regressed, or `start_lt`/`end_lt` are non-monotonic.
    NonMonotonicTime,
    /// §5 rule 4: a recomputed root does not match the header's declared value.
    StateRootMismatch,
    /// §5 rule 5: more than one split/merge announcement flag is set, or a
    /// reserved bit is set.
    ConflictingSplitMergeFlags { flags: u16 },
    /// §5 rule 5: a reserved `flags` bit is set.
    ReservedFlagBitSet { flags: u16 },
    /// §5 rule 6: a block claims an ordinary (non-split/non-merge) or
    /// single-sibling successor relationship to a parent whose header had
    /// `SPLIT_COMMIT` or `MERGE_COMMIT` set.
    InvalidSuccessorAfterCommit,
    /// §5 rule 7: `Masterchain Block Extra`'s `shard_entries` are not sorted
    /// by shard byte encoding.
    UnsortedShardEntries,
    /// §5 rule 7: `Masterchain Block Extra` contains more than one entry for
    /// the same shard.
    DuplicateShardEntry,
    /// §5 rule 8: `master_ref_hash` does not correspond to a resolvable,
    /// correctly-typed masterchain/shardchain reference.
    InvalidMasterchainReference,
}

impl From<DataStructureError> for BlocksError {
    fn from(err: DataStructureError) -> Self {
        Self::DataStructure(err)
    }
}

impl fmt::Display for BlocksError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataStructure(e) => write!(f, "data structure error: {}", e),
            Self::UnresolvableParent => write!(
                f,
                "declared parent reference does not match the resolved parent block"
            ),
            Self::SequenceDiscontinuity { expected, actual } => write!(
                f,
                "sequence discontinuity: expected seq_no {}, got {}",
                expected, actual
            ),
            Self::NonMonotonicTime => write!(
                f,
                "non-monotonic time: gen_utime regressed or start_lt/end_lt is inconsistent"
            ),
            Self::StateRootMismatch => write!(
                f,
                "recomputed root does not match header's declared value"
            ),
            Self::ConflictingSplitMergeFlags { flags } => write!(
                f,
                "more than one split/merge announcement flag set: {:#06x}",
                flags
            ),
            Self::ReservedFlagBitSet { flags } => {
                write!(f, "reserved flags bit set: {:#06x}", flags)
            }
            Self::InvalidSuccessorAfterCommit => write!(
                f,
                "block claims an ordinary successor relationship to a SPLIT_COMMIT/MERGE_COMMIT parent"
            ),
            Self::UnsortedShardEntries => {
                write!(f, "Masterchain Block Extra shard_entries are not sorted")
            }
            Self::DuplicateShardEntry => write!(
                f,
                "Masterchain Block Extra contains more than one entry for the same shard"
            ),
            Self::InvalidMasterchainReference => {
                write!(f, "master_ref_hash does not resolve correctly")
            }
        }
    }
}

impl std::error::Error for BlocksError {}
