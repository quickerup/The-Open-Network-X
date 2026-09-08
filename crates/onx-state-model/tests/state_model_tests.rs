use onx_data_structures::AccountId;
use onx_primitives::{Uint128, Uint256, Uint32, Uint64};
use onx_state_model::{
    AccountState, AccountStateKind, BoC, Cell, MerkleProof, ShardStateTree, StateModelError,
};

#[test]
fn test_account_state_lifecycle_transitions() {
    assert!(AccountStateKind::Uninitialized.can_transition_to(AccountStateKind::Active));
    assert!(AccountStateKind::Active.can_transition_to(AccountStateKind::Frozen));
    assert!(AccountStateKind::Frozen.can_transition_to(AccountStateKind::Active));
    assert!(AccountStateKind::Active.can_transition_to(AccountStateKind::Destroyed));

    assert!(!AccountStateKind::Uninitialized.can_transition_to(AccountStateKind::Frozen));
    assert!(!AccountStateKind::Destroyed.can_transition_to(AccountStateKind::Active));
}

#[test]
fn test_account_state_serialization_and_logical_time_monotonicity() {
    let mut state = AccountState::new_active(
        Uint128(1_000_000_000),
        Uint64(100),
        Uint256([0x11; 32]),
        Uint256([0x22; 32]),
        Uint32(5),
        Uint64(500),
    );

    let bytes = state.to_bytes();
    assert_eq!(bytes.len(), AccountState::BINARY_SIZE);

    let decoded = AccountState::from_bytes(&bytes).unwrap();
    assert_eq!(state, decoded);

    // Monotonic logical time update
    assert!(state.update_logical_time(Uint64(101)).is_ok());
    assert_eq!(state.last_trans_lt, Uint64(101));

    // Logical time regression error
    let reg_err = state.update_logical_time(Uint64(100)).unwrap_err();
    assert!(matches!(
        reg_err,
        StateModelError::LogicalTimeRegression {
            prior: 101,
            current: 100
        }
    ));
}

#[test]
fn test_cell_creation_and_hashing_determinism() {
    let data = vec![1, 2, 3, 4];
    let cell1 = Cell::new(false, data.clone(), vec![]).unwrap();
    let cell2 = Cell::new(false, data, vec![]).unwrap();

    let hash1 = cell1.cell_hash();
    let hash2 = cell2.cell_hash();

    assert_eq!(hash1, hash2);

    // Ensure hash is domain separated and non-zero
    assert_ne!(hash1.0, [0u8; 32]);
}

#[test]
fn test_cell_boundary_validation() {
    // Data length > 128
    let oversized_data = vec![0u8; 129];
    let err_data = Cell::new(false, oversized_data, vec![]).unwrap_err();
    assert!(matches!(
        err_data,
        StateModelError::InvalidCellDataLength(129)
    ));

    // Ref count > 4
    let oversized_refs = vec![Uint256([0x01; 32]); 5];
    let err_refs = Cell::new(false, vec![], oversized_refs).unwrap_err();
    assert!(matches!(err_refs, StateModelError::InvalidCellRefCount(5)));
}

#[test]
fn test_boc_round_trip_and_cycle_detection() {
    let leaf_cell = Cell::new(false, vec![10, 20], vec![]).unwrap();
    let leaf_hash = leaf_cell.cell_hash();

    let root_cell = Cell::new(false, vec![30], vec![leaf_hash]).unwrap();

    let boc = BoC::new(vec![root_cell.clone(), leaf_cell.clone()]).unwrap();
    let bytes = boc.to_bytes();

    let decoded_boc = BoC::from_bytes(&bytes).unwrap();
    assert_eq!(boc, decoded_boc);

    // Self-referential cycle detection test
    let base_hash = Cell::new(false, vec![1], vec![]).unwrap().cell_hash();
    let self_cycle_cell = Cell::new(false, vec![1], vec![base_hash]).unwrap();

    let cyclic_err = BoC::new(vec![self_cycle_cell]).unwrap_err();
    assert_eq!(cyclic_err, StateModelError::CyclicCellReference);
}

#[test]
fn test_shard_state_tree_and_merkle_proof_verification() {
    let mut tree = ShardStateTree::new();

    let account_id_1 = AccountId(Uint256([0x01; 32]));
    let state_1 = AccountState::new_active(
        Uint128(500),
        Uint64(10),
        Uint256([0xaa; 32]),
        Uint256([0xbb; 32]),
        Uint32(2),
        Uint64(200),
    );

    let account_id_2 = AccountId(Uint256([0x02; 32]));
    let state_2 = AccountState::new_active(
        Uint128(1500),
        Uint64(12),
        Uint256([0xcc; 32]),
        Uint256([0xdd; 32]),
        Uint32(3),
        Uint64(300),
    );

    tree.set_account(account_id_1, state_1);
    tree.set_account(account_id_2, state_2);

    let root_hash = tree.state_root_hash().unwrap();
    assert_ne!(root_hash.0, [0u8; 32]);

    // Create and verify valid Merkle proof
    let proof = tree.create_merkle_proof(account_id_1).unwrap();
    assert!(proof.verify().unwrap());

    // Proof binary round-trip
    let proof_bytes = proof.to_bytes();
    let decoded_proof = MerkleProof::from_bytes(&proof_bytes).unwrap();
    assert_eq!(proof, decoded_proof);

    // Rejection of forged proof with modified root hash
    let mut forged_proof = proof;
    forged_proof.root_hash = Uint256([0xff; 32]);
    assert!(forged_proof.verify().is_err());
}
