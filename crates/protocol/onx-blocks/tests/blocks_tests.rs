//! Tests for block structural validity, masterchain coupling, and
//! split/merge flags per docs/specification/blocks.md §6.

use onx_blocks::{
    flags, validate_block_successor, validate_ordinary_successor, BlocksError,
    MasterchainBlockExtra, RecomputedRoots, ShardEntry,
};
use onx_data_structures::{BlockHeader, ShardIdent, WorkchainIdent};
use onx_primitives::{Uint16, Uint256, Uint32, Uint64};

fn header(shard: ShardIdent, seq_no: u32, flags_val: u16) -> BlockHeader {
    BlockHeader {
        magic_constructor: Uint32::from(onx_data_structures::BLOCK_HEADER_MAGIC),
        shard,
        seq_no: Uint32::from(seq_no),
        flags: Uint16::from(flags_val),
        gen_utime: Uint32::from(1_700_000_000u32),
        start_lt: Uint64::from(100u64),
        end_lt: Uint64::from(200u64),
        prev_key_block: Uint32::from(0u32),
        prev_ref_hash: Uint256([0u8; 32]),
        prev_ref_hash_2: Uint256([0u8; 32]),
        master_ref_hash: Uint256([0u8; 32]),
        state_root_hash: Uint256([0xAAu8; 32]),
        in_msg_root_hash: Uint256([0xBBu8; 32]),
        out_msg_root_hash: Uint256([0xCCu8; 32]),
    }
}

fn roots_for(header: &BlockHeader) -> RecomputedRoots {
    RecomputedRoots {
        state_root_hash: header.state_root_hash,
        in_msg_root_hash: header.in_msg_root_hash,
        out_msg_root_hash: header.out_msg_root_hash,
    }
}

fn successor_of(parent: &BlockHeader, seq_no: u32) -> BlockHeader {
    let mut child = header(parent.shard, seq_no, 0);
    child.prev_ref_hash = parent.block_hash();
    child.gen_utime = Uint32::from(parent.gen_utime.0 + 10);
    child.start_lt = Uint64::from(parent.end_lt.0 + 1);
    child.end_lt = Uint64::from(parent.end_lt.0 + 100);
    child
}

#[test]
fn test_ordinary_successor_accepted() {
    let parent = header(ShardIdent::root(WorkchainIdent::BASIC), 10, 0);
    let child = successor_of(&parent, 11);
    assert_eq!(
        validate_ordinary_successor(&child, &parent, roots_for(&child)),
        Ok(())
    );
}

#[test]
fn test_merge_block_successor_accepted() {
    let parent1 = header(ShardIdent::root(WorkchainIdent::BASIC), 10, 0);
    let parent2 = header(ShardIdent::root(WorkchainIdent::BASIC), 12, 0);

    let mut merge_child = header(
        ShardIdent::root(WorkchainIdent::BASIC),
        13,
        flags::MERGE_RESULT,
    );
    merge_child.prev_ref_hash = parent1.block_hash();
    merge_child.prev_ref_hash_2 = parent2.block_hash();
    merge_child.gen_utime = Uint32::from(1_700_000_010u32);
    merge_child.start_lt = Uint64::from(201u64);
    merge_child.end_lt = Uint64::from(300u64);

    assert_eq!(
        validate_block_successor(
            &merge_child,
            &parent1,
            Some(&parent2),
            roots_for(&merge_child)
        ),
        Ok(())
    );
}

#[test]
fn test_unresolvable_parent_rejected() {
    let parent = header(ShardIdent::root(WorkchainIdent::BASIC), 10, 0);
    let mut child = successor_of(&parent, 11);
    child.prev_ref_hash = Uint256([0xFFu8; 32]); // does not match parent.block_hash()
    assert_eq!(
        validate_ordinary_successor(&child, &parent, roots_for(&child)),
        Err(BlocksError::UnresolvableParent)
    );
}

#[test]
fn test_sequence_discontinuity_rejected() {
    let parent = header(ShardIdent::root(WorkchainIdent::BASIC), 10, 0);
    let child = successor_of(&parent, 12); // should be 11
    assert_eq!(
        validate_ordinary_successor(&child, &parent, roots_for(&child)),
        Err(BlocksError::SequenceDiscontinuity {
            expected: 11,
            actual: 12,
        })
    );
}

#[test]
fn test_non_monotonic_time_rejected() {
    let parent = header(ShardIdent::root(WorkchainIdent::BASIC), 10, 0);

    let mut regressed_time = successor_of(&parent, 11);
    regressed_time.gen_utime = Uint32::from(parent.gen_utime.0 - 1);
    assert_eq!(
        validate_ordinary_successor(&regressed_time, &parent, roots_for(&regressed_time)),
        Err(BlocksError::NonMonotonicTime)
    );

    let mut before_parent_end = successor_of(&parent, 11);
    before_parent_end.start_lt = Uint64::from(parent.end_lt.0 - 1);
    assert_eq!(
        validate_ordinary_successor(&before_parent_end, &parent, roots_for(&before_parent_end)),
        Err(BlocksError::NonMonotonicTime)
    );

    let mut start_after_end = successor_of(&parent, 11);
    start_after_end.start_lt = Uint64::from(start_after_end.end_lt.0 + 1);
    assert_eq!(
        validate_ordinary_successor(&start_after_end, &parent, roots_for(&start_after_end)),
        Err(BlocksError::NonMonotonicTime)
    );
}

#[test]
fn test_state_root_mismatch_rejected() {
    let parent = header(ShardIdent::root(WorkchainIdent::BASIC), 10, 0);
    let child = successor_of(&parent, 11);
    let wrong_roots = RecomputedRoots {
        state_root_hash: Uint256([0u8; 32]),
        in_msg_root_hash: child.in_msg_root_hash,
        out_msg_root_hash: child.out_msg_root_hash,
    };
    assert_eq!(
        validate_ordinary_successor(&child, &parent, wrong_roots),
        Err(BlocksError::StateRootMismatch)
    );
}

#[test]
fn test_successor_after_commit_rejected() {
    let parent = header(
        ShardIdent::root(WorkchainIdent::BASIC),
        10,
        flags::SPLIT_COMMIT,
    );
    let child = successor_of(&parent, 11);
    assert_eq!(
        validate_ordinary_successor(&child, &parent, roots_for(&child)),
        Err(BlocksError::InvalidSuccessorAfterCommit)
    );
}

#[test]
fn test_flags_conflicting_and_reserved_bits_rejected() {
    let parent = header(ShardIdent::root(WorkchainIdent::BASIC), 10, 0);

    let mut conflicting = successor_of(&parent, 11);
    conflicting.flags = Uint16::from(flags::SPLIT_PREPARE | flags::MERGE_PREPARE);
    assert_eq!(
        validate_ordinary_successor(&conflicting, &parent, roots_for(&conflicting)),
        Err(BlocksError::ConflictingSplitMergeFlags {
            flags: flags::SPLIT_PREPARE | flags::MERGE_PREPARE,
        })
    );

    let mut reserved_bit = successor_of(&parent, 11);
    reserved_bit.flags = Uint16::from(0x0020);
    assert_eq!(
        validate_ordinary_successor(&reserved_bit, &parent, roots_for(&reserved_bit)),
        Err(BlocksError::ReservedFlagBitSet { flags: 0x0020 })
    );

    // A single announcement flag, and MERGE_RESULT alongside it, are both well-formed on their own.
    let mut single_flag = successor_of(&parent, 11);
    single_flag.flags = Uint16::from(flags::SPLIT_PREPARE);
    assert_eq!(
        validate_ordinary_successor(&single_flag, &parent, roots_for(&single_flag)),
        Ok(())
    );
}

#[test]
fn test_masterchain_ref_validation() {
    let mut masterchain_block = header(ShardIdent::root(WorkchainIdent::MASTERCHAIN), 5, 0);
    masterchain_block.master_ref_hash = Uint256::ZERO;
    assert_eq!(
        onx_blocks::validate_master_ref(&masterchain_block, None),
        Ok(())
    );

    let mut bad_masterchain_header = masterchain_block.clone();
    bad_masterchain_header.master_ref_hash = Uint256([0x01; 32]);
    assert_eq!(
        onx_blocks::validate_master_ref(&bad_masterchain_header, None),
        Err(BlocksError::InvalidMasterchainReference)
    );

    let mut shardchain_header = header(ShardIdent::root(WorkchainIdent::BASIC), 5, 0);
    shardchain_header.master_ref_hash = masterchain_block.block_hash();
    assert_eq!(
        onx_blocks::validate_master_ref(&shardchain_header, Some(&masterchain_block)),
        Ok(())
    );

    // Unresolvable masterchain reference (caller couldn't resolve it).
    assert_eq!(
        onx_blocks::validate_master_ref(&shardchain_header, None),
        Err(BlocksError::InvalidMasterchainReference)
    );

    // Resolved block doesn't match the declared hash.
    let other_masterchain_block = header(ShardIdent::root(WorkchainIdent::MASTERCHAIN), 6, 0);
    assert_eq!(
        onx_blocks::validate_master_ref(&shardchain_header, Some(&other_masterchain_block)),
        Err(BlocksError::InvalidMasterchainReference)
    );
}

#[test]
fn test_masterchain_block_extra_round_trip_and_canonicality() {
    let shard_a =
        ShardIdent::from_prefix_bits(WorkchainIdent::BASIC, 0x0000_0000_0000_0000, 1).unwrap();
    let shard_b =
        ShardIdent::from_prefix_bits(WorkchainIdent::BASIC, 0x8000_0000_0000_0000, 1).unwrap();

    let entry_a = ShardEntry {
        shard: shard_a,
        block_hash: Uint256([0x11; 32]),
        seq_no: Uint32::from(1u32),
    };
    let entry_b = ShardEntry {
        shard: shard_b,
        block_hash: Uint256([0x22; 32]),
        seq_no: Uint32::from(2u32),
    };

    let extra = MasterchainBlockExtra {
        shard_entries: vec![entry_a, entry_b],
    };
    let bytes = extra.to_bytes();
    let decoded = MasterchainBlockExtra::from_bytes(&bytes).unwrap();
    assert_eq!(decoded, extra);

    assert!(extra.is_canonical(&shard_a, Uint256([0x11; 32]), 1));
    assert!(!extra.is_canonical(&shard_a, Uint256([0x99; 32]), 1));
    assert!(!extra.is_canonical(&shard_b, Uint256([0x11; 32]), 1));
}

#[test]
fn test_masterchain_block_extra_rejects_unsorted_and_duplicate_entries() {
    let shard_a =
        ShardIdent::from_prefix_bits(WorkchainIdent::BASIC, 0x0000_0000_0000_0000, 1).unwrap();
    let shard_b =
        ShardIdent::from_prefix_bits(WorkchainIdent::BASIC, 0x8000_0000_0000_0000, 1).unwrap();

    let entry_a = ShardEntry {
        shard: shard_a,
        block_hash: Uint256([0x11; 32]),
        seq_no: Uint32::from(1u32),
    };
    let entry_b = ShardEntry {
        shard: shard_b,
        block_hash: Uint256([0x22; 32]),
        seq_no: Uint32::from(2u32),
    };

    // Unsorted (b before a).
    let unsorted = MasterchainBlockExtra {
        shard_entries: vec![entry_b, entry_a],
    };
    assert_eq!(unsorted.validate(), Err(BlocksError::UnsortedShardEntries));
    assert_eq!(
        MasterchainBlockExtra::from_bytes(&unsorted.to_bytes()),
        Err(BlocksError::UnsortedShardEntries)
    );

    // Duplicate shard entry.
    let duplicate = MasterchainBlockExtra {
        shard_entries: vec![entry_a, entry_a],
    };
    assert_eq!(duplicate.validate(), Err(BlocksError::DuplicateShardEntry));
}
