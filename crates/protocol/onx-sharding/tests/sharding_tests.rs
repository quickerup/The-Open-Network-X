//! Tests for ONX Dynamic Sharding per docs/specification/sharding.md.

use onx_data_structures::{
    AccountId, FullAddress, Message, MessageType, ShardIdent, WorkchainIdent,
};
use onx_primitives::{Uint128, Uint256, Uint64};
use onx_sharding::{
    merge_shard_states, merge_trigger_met, split_shard_state, split_trigger_met,
    validate_transition_commit, LoadSample, ShardState, ShardTreeNode, ShardingError,
    TransitionKind, MERGE_LOAD_WINDOW_BLOCKS, SPLIT_LOAD_WINDOW_BLOCKS,
};
use onx_state_model::AccountState;
use std::collections::BTreeMap;

#[test]
fn test_shard_tree_leaf_split() {
    let root_shard = ShardIdent::root(WorkchainIdent::BASIC);
    let root_node = ShardTreeNode::new_leaf(root_shard);

    let split_node = root_node.split_leaf().unwrap();

    match split_node {
        ShardTreeNode::Internal { shard, left, right } => {
            assert_eq!(shard, root_shard);
            match (*left, *right) {
                (ShardTreeNode::Leaf { shard: s0, .. }, ShardTreeNode::Leaf { shard: s1, .. }) => {
                    assert_eq!(s0.prefix_len().unwrap(), 1);
                    assert_eq!(s1.prefix_len().unwrap(), 1);
                }
                _ => panic!("Expected child leaf nodes"),
            }
        }
        _ => panic!("Expected internal node after split"),
    }
}

#[test]
fn test_load_based_trigger_conditions() {
    let byte_limit = 1_000_000u64;
    let gas_limit = 10_000_000u64;

    // 75% load -> should split
    assert!(ShardTreeNode::should_split(
        750_000, byte_limit, 7_500_000, gas_limit
    ));

    // 70% load -> should not split
    assert!(!ShardTreeNode::should_split(
        700_000, byte_limit, 7_000_000, gas_limit
    ));

    // 20% load -> should merge
    assert!(ShardTreeNode::should_merge(
        200_000, byte_limit, 2_000_000, gas_limit
    ));

    // 25% load -> should not merge
    assert!(!ShardTreeNode::should_merge(
        250_000, byte_limit, 2_500_000, gas_limit
    ));
}

fn account(first_byte: u8) -> AccountId {
    let mut bytes = [0u8; 32];
    bytes[0] = first_byte;
    AccountId::from_bytes(bytes)
}

fn message(destination: AccountId, created_lt: u64, marker: u8) -> Message {
    let source = account(0x80);
    Message {
        msg_type: MessageType::Internal,
        src_address: FullAddress::new(WorkchainIdent::BASIC, source),
        dest_address: FullAddress::new(WorkchainIdent::BASIC, destination),
        amount_nanos: Uint128::from(1u128),
        extra_currencies: vec![],
        created_lt: Uint64::from(created_lt),
        body_cell_hash: Uint256([marker; 32]),
    }
}

#[test]
fn split_partitions_accounts_and_destination_queues_deterministically() {
    let root = ShardIdent::root(WorkchainIdent::BASIC);
    let low = account(0x10);
    let high = account(0xF0);
    let mut accounts = BTreeMap::new();
    accounts.insert(low, AccountState::Uninitialized);
    accounts.insert(high, AccountState::Destroyed);
    let state = ShardState::new(
        root,
        accounts,
        vec![message(high, 9, 2), message(low, 7, 1), message(high, 3, 3)],
    );

    let (left, right) = split_shard_state(state).unwrap();
    assert!(left.accounts.contains_key(&low));
    assert!(right.accounts.contains_key(&high));
    assert_eq!(
        left.pending_messages
            .iter()
            .map(|m| m.created_lt.0)
            .collect::<Vec<_>>(),
        vec![7]
    );
    assert_eq!(
        right
            .pending_messages
            .iter()
            .map(|m| m.created_lt.0)
            .collect::<Vec<_>>(),
        vec![3, 9]
    );
}

#[test]
fn merge_restores_parent_and_interleaves_pending_messages_by_logical_time() {
    let root = ShardIdent::root(WorkchainIdent::BASIC);
    let low = account(0x10);
    let high = account(0xF0);
    let mut accounts = BTreeMap::new();
    accounts.insert(low, AccountState::Uninitialized);
    accounts.insert(high, AccountState::Destroyed);
    let (left, right) = split_shard_state(ShardState::new(
        root,
        accounts,
        vec![message(low, 11, 1), message(high, 4, 2), message(low, 6, 3)],
    ))
    .unwrap();

    let merged = merge_shard_states(right, left).unwrap();
    assert_eq!(merged.shard, root);
    assert_eq!(merged.accounts.len(), 2);
    assert_eq!(
        merged
            .pending_messages
            .iter()
            .map(|m| m.created_lt.0)
            .collect::<Vec<_>>(),
        vec![4, 6, 11]
    );
}

#[test]
fn split_rejects_account_or_pending_message_outside_source_shard() {
    let left = ShardIdent::from_prefix_bits(WorkchainIdent::BASIC, 0, 1).unwrap();
    let high = account(0xF0);
    let mut accounts = BTreeMap::new();
    accounts.insert(high, AccountState::Uninitialized);
    assert_eq!(
        split_shard_state(ShardState::new(left, accounts, vec![])).unwrap_err(),
        ShardingError::AccountOutsideShard
    );

    assert_eq!(
        split_shard_state(ShardState::new(
            left,
            BTreeMap::new(),
            vec![message(high, 1, 1)]
        ))
        .unwrap_err(),
        ShardingError::MessageOutsideShard
    );
}

#[test]
fn merge_rejects_non_sibling_shards() {
    let first = ShardIdent::from_prefix_bits(WorkchainIdent::BASIC, 0, 2).unwrap();
    let second =
        ShardIdent::from_prefix_bits(WorkchainIdent::BASIC, 0xC000_0000_0000_0000, 2).unwrap();
    assert_eq!(
        merge_shard_states(
            ShardState::new(first, BTreeMap::new(), vec![]),
            ShardState::new(second, BTreeMap::new(), vec![])
        )
        .unwrap_err(),
        ShardingError::InvalidMergeSiblings
    );
}

#[test]
fn load_windows_require_exact_length_and_all_samples_to_cross_threshold() {
    let split_sample = LoadSample {
        block_bytes: 75,
        gas_used: 75,
    };
    let mut split_samples = vec![split_sample; SPLIT_LOAD_WINDOW_BLOCKS];
    assert!(split_trigger_met(&split_samples, 100, 100));
    split_samples[17].gas_used = 74;
    assert!(!split_trigger_met(&split_samples, 100, 100));
    assert!(!split_trigger_met(
        &vec![split_sample; SPLIT_LOAD_WINDOW_BLOCKS - 1],
        100,
        100
    ));

    let merge_sample = LoadSample {
        block_bytes: 20,
        gas_used: 20,
    };
    let mut merge_samples = vec![merge_sample; MERGE_LOAD_WINDOW_BLOCKS];
    assert!(merge_trigger_met(&merge_samples, 100, 100));
    merge_samples[1].block_bytes = 21;
    assert!(!merge_trigger_met(&merge_samples, 100, 100));
}

#[test]
fn transition_commit_requires_exact_prepare_lead_and_bounded_assignment_drift() {
    assert!(validate_transition_commit(TransitionKind::Split, &[92], 100, 7, 8).is_ok());
    assert!(validate_transition_commit(TransitionKind::Merge, &[92, 92], 100, 8, 7).is_ok());

    assert_eq!(
        validate_transition_commit(TransitionKind::Split, &[93], 100, 7, 7).unwrap_err(),
        ShardingError::AnnouncementSequenceViolation
    );
    assert_eq!(
        validate_transition_commit(TransitionKind::Merge, &[92], 100, 7, 7).unwrap_err(),
        ShardingError::AnnouncementSequenceViolation
    );
    assert_eq!(
        validate_transition_commit(TransitionKind::Split, &[92], 100, 7, 9).unwrap_err(),
        ShardingError::TaskGroupDriftExceeded
    );
}
