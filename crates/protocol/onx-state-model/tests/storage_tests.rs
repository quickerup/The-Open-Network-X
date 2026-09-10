use onx_data_structures::{AccountId, BlockHeader, ShardIdent, WorkchainIdent, BLOCK_HEADER_MAGIC};
use onx_primitives::{Uint16, Uint256, Uint32, Uint64};
use onx_state_model::{AccountState, BlockCommitment, Cell, ShardStateTree, StateStorage};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "onx-state-storage-{name}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
fn shard() -> ShardIdent {
    ShardIdent::root(WorkchainIdent::BASIC)
}
fn header(root: [u8; 32]) -> BlockHeader {
    BlockHeader {
        magic_constructor: Uint32(BLOCK_HEADER_MAGIC),
        shard: shard(),
        seq_no: Uint32(1),
        flags: Uint16(0),
        gen_utime: Uint32(0),
        start_lt: Uint64(0),
        end_lt: Uint64(0),
        prev_key_block: Uint32(0),
        prev_ref_hash: Uint256([0; 32]),
        prev_ref_hash_2: Uint256([0; 32]),
        master_ref_hash: Uint256([0; 32]),
        state_root_hash: Uint256(root),
        in_msg_root_hash: Uint256([0; 32]),
        out_msg_root_hash: Uint256([0; 32]),
    }
}
fn commitment(tree: ShardStateTree) -> BlockCommitment {
    BlockCommitment {
        header: header(tree.state_root_hash()),
        state: tree,
        cell_bocs: vec![],
    }
}

#[test]
fn restart_reloads_identical_state_root() {
    let path = temp_path("restart");
    let mut state = ShardStateTree::new();
    for i in 0..256u16 {
        let mut id = [0; 32];
        id[..2].copy_from_slice(&i.to_be_bytes());
        state.insert(AccountId::from_bytes(id), AccountState::Uninitialized);
    }
    let root = state.state_root_hash();
    {
        let db = StateStorage::open(&path).unwrap();
        db.commit_block(commitment(state)).unwrap();
    }
    let db = StateStorage::open(&path).unwrap();
    assert_eq!(db.shard_state_root(shard()).unwrap(), Some(root));
    assert_eq!(
        db.load_shard_state(shard()).unwrap().state_root_hash(),
        root
    );
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn cell_deduplication_and_gc_are_persistent() {
    let path = temp_path("gc");
    let cell = Cell::new(b"deduplicated".to_vec(), vec![]).unwrap();
    let h = cell.hash();
    let db = StateStorage::open(&path).unwrap();
    db.put_cell(&cell).unwrap();
    db.put_cell(&cell).unwrap();
    assert_eq!(db.cell_refcount(&h).unwrap(), Some(0));
    db.retain_cell_root(h).unwrap();
    assert_eq!(db.cell_refcount(&h).unwrap(), Some(1));
    db.release_cell_root(h).unwrap();
    assert_eq!(db.get_cell(&h).unwrap(), None);
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn journal_recovery_replays_complete_commit_after_interruption() {
    let path = temp_path("journal");
    let db = StateStorage::open(&path).unwrap();
    let journal = path.join(".commit-journal");
    fs::write(
        &journal,
        "P shard_states/000000000000000000000000 deadbeef\n",
    )
    .unwrap();
    drop(db);
    let db = StateStorage::open(&path).unwrap();
    assert!(path.join("shard_states/000000000000000000000000").exists());
    assert!(!journal.exists());
    drop(db);
    fs::remove_dir_all(path).unwrap();
}
