use onx_data_structures::AccountId;
use onx_primitives::Uint256;
use onx_state_model::{
    AccountState, AccountStateRecord, AccountStatus, BagOfCells, Cell, MerkleProof, ShardStateTree,
    StateModelError,
};

#[test]
fn test_account_lifecycle_state_machine() {
    let uninit = AccountState::Uninitialized;
    assert_eq!(uninit.status(), AccountStatus::Uninitialized);

    let rec1 = AccountStateRecord {
        balance_nanos: 10_000_000,
        last_trans_lt: 100,
        code_hash: [0xaa; 32],
        data_hash: [0xbb; 32],
        cell_count: 2,
        byte_count: 64,
    };

    // Transition Uninitialized -> Active
    let active = uninit.transition_to(rec1.clone()).unwrap();
    assert_eq!(active.status(), AccountStatus::Active);

    // Balance credit/deduct
    let mut active_mut = active.clone();
    active_mut.deduct_balance(1_000_000).unwrap();
    if let AccountState::Active(ref r) = active_mut {
        assert_eq!(r.balance_nanos, 9_000_000);
    } else {
        panic!("expected active");
    }

    // Logical time regression check
    let mut regressed_rec = rec1.clone();
    regressed_rec.last_trans_lt = 90;
    assert!(matches!(
        active_mut.transition_to(regressed_rec),
        Err(StateModelError::LogicalTimeRegression { .. })
    ));

    // Transition to Frozen & Destroyed rejection
    let frozen = AccountState::Frozen {
        storage_hash: [0x11; 32],
    };
    assert_eq!(frozen.status(), AccountStatus::Frozen);
    assert!(frozen.transition_to(rec1.clone()).is_err());

    let destroyed = AccountState::Destroyed;
    assert_eq!(destroyed.status(), AccountStatus::Destroyed);
    assert!(destroyed.transition_to(rec1).is_err());
}

#[test]
fn test_cell_hashing_and_boc_graph_round_trip() {
    let c1 = Cell::new(vec![1, 2, 3, 4], vec![], false).unwrap();
    let c2 = Cell::new(vec![5, 6, 7], vec![c1.hash()], false).unwrap();

    let boc = BagOfCells::new(vec![c2.clone(), c1.clone()], vec![0]).unwrap();
    let root_hash = boc.state_root_hash().unwrap();
    assert_eq!(root_hash, c2.hash());

    let bytes = boc.to_bytes();
    let decoded_boc = BagOfCells::from_bytes(&bytes).unwrap();
    assert_eq!(boc, decoded_boc);
}

#[test]
fn test_shard_state_tree_and_merkle_proof_determinism() {
    let mut tree1 = ShardStateTree::new();
    let mut tree2 = ShardStateTree::new();

    let id_a = AccountId(Uint256([0x01; 32]));
    let id_b = AccountId(Uint256([0x02; 32]));

    let rec_a = AccountStateRecord {
        balance_nanos: 500,
        last_trans_lt: 10,
        code_hash: [0x10; 32],
        data_hash: [0x20; 32],
        cell_count: 1,
        byte_count: 10,
    };
    let rec_b = AccountStateRecord {
        balance_nanos: 1500,
        last_trans_lt: 12,
        code_hash: [0x30; 32],
        data_hash: [0x40; 32],
        cell_count: 3,
        byte_count: 50,
    };

    // Insert in different orders
    tree1.insert(id_a, AccountState::Active(rec_a.clone()));
    tree1.insert(id_b, AccountState::Active(rec_b.clone()));

    tree2.insert(id_b, AccountState::Active(rec_b));
    tree2.insert(id_a, AccountState::Active(rec_a));

    // Must yield identical root hashes
    let root1 = tree1.root_hash();
    let root2 = tree2.root_hash();
    assert_eq!(root1, root2);

    // Verify Merkle Proof
    let proof = MerkleProof::generate(&tree1, id_a).unwrap();
    assert!(proof.verify().unwrap());
    assert_eq!(proof.root_hash, root1);
}
