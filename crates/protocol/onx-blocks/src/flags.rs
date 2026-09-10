//! Split/merge announcement header flag bits within `BlockHeader.flags`,
//! per `docs/specification/blocks.md` §3.4, §4.2.

use crate::error::BlocksError;

/// This shard intends to split; announced several blocks before the split.
pub const SPLIT_PREPARE: u16 = 0x0001;
/// This is the last block of the pre-split shard; the next blocks belong to its two children.
pub const SPLIT_COMMIT: u16 = 0x0002;
/// This sibling shard intends to merge with its sibling.
pub const MERGE_PREPARE: u16 = 0x0004;
/// This is the last block of the pre-merge sibling shards; the next block belongs to the merged shard.
pub const MERGE_COMMIT: u16 = 0x0008;
/// This block is the first block of a shardchain formed by merging two
/// sibling shards, and carries a second parent reference (ADR-0016,
/// `data-structures.md` §4.4). Structural validation of the two-parent
/// relationship this flag gates is not yet implemented here: it requires
/// `BlockHeader`'s `prev_ref_hash_2` field, which
/// `crates/protocol/onx-data-structures` does not yet have (tracked in `ROADMAP.md`).
pub const MERGE_RESULT: u16 = 0x0010;

const ANNOUNCEMENT_MASK: u16 = SPLIT_PREPARE | SPLIT_COMMIT | MERGE_PREPARE | MERGE_COMMIT;
const RESERVED_MASK: u16 = !(ANNOUNCEMENT_MASK | MERGE_RESULT);

/// Validates `flags` well-formedness per §5 rule 5: at most one of
/// `SPLIT_PREPARE`/`SPLIT_COMMIT`/`MERGE_PREPARE`/`MERGE_COMMIT` may be set,
/// and reserved bits (5-15) must be zero. `MERGE_RESULT` is independent of
/// the four announcement flags and never conflicts with them.
pub fn validate_flags(flags: u16) -> Result<(), BlocksError> {
    if flags & RESERVED_MASK != 0 {
        return Err(BlocksError::ReservedFlagBitSet { flags });
    }
    if (flags & ANNOUNCEMENT_MASK).count_ones() > 1 {
        return Err(BlocksError::ConflictingSplitMergeFlags { flags });
    }
    Ok(())
}

/// True if `SPLIT_COMMIT` is set.
pub fn is_split_commit(flags: u16) -> bool {
    flags & SPLIT_COMMIT != 0
}

/// True if `MERGE_COMMIT` is set.
pub fn is_merge_commit(flags: u16) -> bool {
    flags & MERGE_COMMIT != 0
}

/// True if `MERGE_RESULT` is set.
pub fn is_merge_result(flags: u16) -> bool {
    flags & MERGE_RESULT != 0
}
