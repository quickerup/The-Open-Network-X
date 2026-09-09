use onx_data_structures::AccountId;
use onx_state_model::{
    AccountState, BlockCommitBatch, Cell, ShardStateTree, StorageEngine, StorageStat,
};
use tempfile::TempDir;

#[test]
fn test_storage_crud_operations() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("onx_test_db");

    let engine = StorageEngine::open(&db_path).unwrap();

    // 1. Account CRUD
    let account_bytes = [1u8; 32];
    let account_id = AccountId::from_bytes(account_bytes);
    let account_state = AccountState::Active {
        balance_nanos: 500_000_000,
        last_trans_lt: 100,
        code_hash: [2u8; 32],
        data_hash: [3u8; 32],
        storage_stat: StorageStat {
            cell_count: 5,
            byte_count: 200,
        },
    };

    engine.put_account(&account_id, &account_state).unwrap();
    let fetched = engine.get_account(&account_id).unwrap().unwrap();
    assert_eq!(fetched, account_state);

    // 2. Cell CRUD & Deduplication
    let cell1 = Cell::new(vec![1, 2, 3, 4], vec![]).unwrap();
    let hash1 = engine.put_cell(&cell1).unwrap();
    assert_eq!(hash1, cell1.hash());

    let fetched_cell = engine.get_cell(&hash1).unwrap().unwrap();
    assert_eq!(fetched_cell, cell1);

    // Re-inserting same cell increments ref count
    engine.put_cell(&cell1).unwrap();
    assert_eq!(engine.get_ref_count(&hash1).unwrap(), 2);

    engine.remove_cell_ref(&hash1).unwrap();
    assert_eq!(engine.get_ref_count(&hash1).unwrap(), 1);
    assert!(engine.get_cell(&hash1).unwrap().is_some());

    engine.remove_cell_ref(&hash1).unwrap();
    assert_eq!(engine.get_ref_count(&hash1).unwrap(), 0);
    assert!(engine.get_cell(&hash1).unwrap().is_none());

    // 3. Block Headers and Shard States CRUD
    let header_key = b"block_100";
    let header_val = b"header_data_100";
    engine.put_block_header(header_key, header_val).unwrap();
    assert_eq!(
        engine.get_block_header(header_key).unwrap().unwrap(),
        header_val
    );

    let shard_key = b"shard_0";
    let shard_val = b"shard_data_0";
    engine.put_shard_state(shard_key, shard_val).unwrap();
    assert_eq!(
        engine.get_shard_state(shard_key).unwrap().unwrap(),
        shard_val
    );
}

#[test]
fn test_atomic_block_batch_commit() {
    let engine = StorageEngine::open_temporary().unwrap();

    let account_id_1 = AccountId::from_bytes([10u8; 32]);
    let account_id_2 = AccountId::from_bytes([20u8; 32]);

    let state_1 = AccountState::Active {
        balance_nanos: 1_000,
        last_trans_lt: 10,
        code_hash: [0u8; 32],
        data_hash: [0u8; 32],
        storage_stat: StorageStat {
            cell_count: 1,
            byte_count: 10,
        },
    };

    let cell = Cell::new(vec![9, 9, 9], vec![]).unwrap();
    let cell_hash = cell.hash();

    let mut batch = BlockCommitBatch::default();
    batch
        .accounts_to_upsert
        .insert(account_id_1, state_1.clone());
    batch.cells_to_add.insert(cell_hash, cell.clone());
    batch
        .block_headers
        .insert(b"blk_1".to_vec(), b"hdr_1".to_vec());

    engine.commit_block_batch(batch).unwrap();

    assert_eq!(engine.get_account(&account_id_1).unwrap().unwrap(), state_1);
    assert_eq!(engine.get_cell(&cell_hash).unwrap().unwrap(), cell);
    assert_eq!(
        engine.get_block_header(b"blk_1").unwrap().unwrap(),
        b"hdr_1".to_vec()
    );

    // Now delete account 1 and insert account 2 atomically
    let mut batch2 = BlockCommitBatch::default();
    batch2.accounts_to_delete.insert(account_id_1);
    batch2
        .accounts_to_upsert
        .insert(account_id_2, state_1.clone());
    batch2.cells_to_decrement_ref.push(cell_hash);

    engine.commit_block_batch(batch2).unwrap();

    assert!(engine.get_account(&account_id_1).unwrap().is_none());
    assert_eq!(engine.get_account(&account_id_2).unwrap().unwrap(), state_1);
    assert!(engine.get_cell(&cell_hash).unwrap().is_none()); // GC removed cell
}

#[test]
fn test_database_restart_and_state_root_rebuilding() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("onx_restart_db");

    let mut in_memory_tree = ShardStateTree::new();

    {
        let engine = StorageEngine::open(&db_path).unwrap();

        for i in 0..100u64 {
            let mut acc_bytes = [0u8; 32];
            acc_bytes[..8].copy_from_slice(&i.to_be_bytes());
            let account_id = AccountId::from_bytes(acc_bytes);

            let state = AccountState::Active {
                balance_nanos: (i as u128) * 1_000_000,
                last_trans_lt: i + 1,
                code_hash: [1u8; 32],
                data_hash: [2u8; 32],
                storage_stat: StorageStat {
                    cell_count: 2,
                    byte_count: 64,
                },
            };

            engine.put_account(&account_id, &state).unwrap();
            in_memory_tree.insert(account_id, state);
        }

        engine.flush().unwrap();
    }

    // Reopen DB from disk
    let reopened_engine = StorageEngine::open(&db_path).unwrap();
    let rebuilt_tree = reopened_engine.rebuild_shard_state_tree().unwrap();

    assert_eq!(
        rebuilt_tree.state_root_hash(),
        in_memory_tree.state_root_hash()
    );
    assert_eq!(rebuilt_tree.accounts().len(), 100);
}

#[test]
fn test_acceptance_100k_account_states_persist_and_rebuild() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("onx_100k_db");

    let num_accounts = 100_000;
    let mut expected_tree = ShardStateTree::new();

    {
        let engine = StorageEngine::open(&db_path).unwrap();

        let mut batch = BlockCommitBatch::default();

        for i in 0..num_accounts {
            let mut acc_bytes = [0u8; 32];
            acc_bytes[0..8].copy_from_slice(&(i as u64).to_be_bytes());
            let account_id = AccountId::from_bytes(acc_bytes);

            let state = AccountState::Active {
                balance_nanos: (i as u128) + 1_000,
                last_trans_lt: (i as u64) + 1,
                code_hash: [0xAA; 32],
                data_hash: [0xBB; 32],
                storage_stat: StorageStat {
                    cell_count: 1,
                    byte_count: 32,
                },
            };

            batch.accounts_to_upsert.insert(account_id, state.clone());
            expected_tree.insert(account_id, state);

            if batch.accounts_to_upsert.len() >= 10_000 {
                let current_batch = std::mem::take(&mut batch);
                engine.commit_block_batch(current_batch).unwrap();
            }
        }

        if !batch.accounts_to_upsert.is_empty() {
            engine.commit_block_batch(batch).unwrap();
        }

        engine.flush().unwrap();
    }

    // Restart the node (reopen DB from disk)
    let reopened_engine = StorageEngine::open(&db_path).unwrap();
    let rebuilt_tree = reopened_engine.rebuild_shard_state_tree().unwrap();

    assert_eq!(rebuilt_tree.accounts().len(), num_accounts);
    assert_eq!(
        rebuilt_tree.state_root_hash(),
        expected_tree.state_root_hash(),
        "Merkle state root mismatch after database restart with 100,000 accounts"
    );
}
