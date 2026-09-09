use crate::account::AccountState;
use crate::cell::Cell;
use crate::error::StateModelError;
use crate::tree::ShardStateTree;
use onx_data_structures::AccountId;
use sled::Transactional;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Column Family / Sled Tree constants.
pub const CF_CELLS: &[u8] = b"cells";
pub const CF_ACCOUNTS: &[u8] = b"accounts";
pub const CF_BLOCK_HEADERS: &[u8] = b"block_headers";
pub const CF_SHARD_STATES: &[u8] = b"shard_states";
pub const CF_CELL_REF_COUNTS: &[u8] = b"cell_ref_counts";

/// Persistent, disk-backed key-value storage engine using Sled.
#[derive(Clone)]
pub struct StorageEngine {
    db: sled::Db,
    cells: sled::Tree,
    accounts: sled::Tree,
    block_headers: sled::Tree,
    shard_states: sled::Tree,
    cell_ref_counts: sled::Tree,
}

/// An atomic batch of updates to commit to storage for a block.
#[derive(Debug, Default)]
pub struct BlockCommitBatch {
    pub cells_to_add: HashMap<[u8; 32], Cell>,
    pub accounts_to_upsert: HashMap<AccountId, AccountState>,
    pub accounts_to_delete: HashSet<AccountId>,
    pub block_headers: HashMap<Vec<u8>, Vec<u8>>,
    pub shard_states: HashMap<Vec<u8>, Vec<u8>>,
    pub cells_to_decrement_ref: Vec<[u8; 32]>,
}

impl StorageEngine {
    /// Opens or creates a persistent storage engine at the given path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StateModelError> {
        let db = sled::open(path).map_err(|e| StateModelError::StorageError(e.to_string()))?;
        Self::from_db(db)
    }

    /// Creates an in-memory storage engine for testing.
    pub fn open_temporary() -> Result<Self, StateModelError> {
        let config = sled::Config::new().temporary(true);
        let db = config
            .open()
            .map_err(|e| StateModelError::StorageError(e.to_string()))?;
        Self::from_db(db)
    }

    fn from_db(db: sled::Db) -> Result<Self, StateModelError> {
        let cells = db
            .open_tree(CF_CELLS)
            .map_err(|e| StateModelError::StorageError(e.to_string()))?;
        let accounts = db
            .open_tree(CF_ACCOUNTS)
            .map_err(|e| StateModelError::StorageError(e.to_string()))?;
        let block_headers = db
            .open_tree(CF_BLOCK_HEADERS)
            .map_err(|e| StateModelError::StorageError(e.to_string()))?;
        let shard_states = db
            .open_tree(CF_SHARD_STATES)
            .map_err(|e| StateModelError::StorageError(e.to_string()))?;
        let cell_ref_counts = db
            .open_tree(CF_CELL_REF_COUNTS)
            .map_err(|e| StateModelError::StorageError(e.to_string()))?;

        Ok(Self {
            db,
            cells,
            accounts,
            block_headers,
            shard_states,
            cell_ref_counts,
        })
    }

    /// Flushes all pending writes to disk.
    pub fn flush(&self) -> Result<(), StateModelError> {
        self.db
            .flush()
            .map_err(|e| StateModelError::StorageError(e.to_string()))?;
        Ok(())
    }

    // --- Cell operations ---

    /// Inserts a cell indexed by its 32-byte SHA-256 hash. Increments reference count if already present or new.
    pub fn put_cell(&self, cell: &Cell) -> Result<[u8; 32], StateModelError> {
        let hash = cell.hash();
        let bytes = cell.to_bytes();

        self.cells
            .insert(hash, bytes)
            .map_err(|e| StateModelError::StorageError(e.to_string()))?;

        self.increment_ref_count(&hash, 1)?;

        Ok(hash)
    }

    /// Gets a cell by its 32-byte SHA-256 hash.
    pub fn get_cell(&self, hash: &[u8; 32]) -> Result<Option<Cell>, StateModelError> {
        let val = self
            .cells
            .get(hash)
            .map_err(|e| StateModelError::StorageError(e.to_string()))?;

        match val {
            Some(bytes) => {
                let (cell, consumed) = Cell::from_bytes(&bytes)?;
                if consumed != bytes.len() {
                    return Err(StateModelError::TrailingBytes {
                        remaining: bytes.len() - consumed,
                    });
                }
                Ok(Some(cell))
            }
            None => Ok(None),
        }
    }

    /// Decrements reference count for cell and removes if ref count reaches zero (GC).
    pub fn remove_cell_ref(&self, hash: &[u8; 32]) -> Result<(), StateModelError> {
        let ref_cnt = self.get_ref_count(hash)?;
        if ref_cnt <= 1 {
            self.cell_ref_counts
                .remove(hash)
                .map_err(|e| StateModelError::StorageError(e.to_string()))?;
            self.cells
                .remove(hash)
                .map_err(|e| StateModelError::StorageError(e.to_string()))?;
        } else {
            self.decrement_ref_count(hash, 1)?;
        }
        Ok(())
    }

    // --- Account operations ---

    /// Puts or updates an account state.
    pub fn put_account(
        &self,
        account_id: &AccountId,
        state: &AccountState,
    ) -> Result<(), StateModelError> {
        let key = account_id.to_bytes();
        let val = state.to_bytes();
        self.accounts
            .insert(key, val)
            .map_err(|e| StateModelError::StorageError(e.to_string()))?;
        Ok(())
    }

    /// Gets an account state by AccountId.
    pub fn get_account(
        &self,
        account_id: &AccountId,
    ) -> Result<Option<AccountState>, StateModelError> {
        let key = account_id.to_bytes();
        let val = self
            .accounts
            .get(key)
            .map_err(|e| StateModelError::StorageError(e.to_string()))?;

        match val {
            Some(bytes) => {
                let (state, consumed) = AccountState::from_bytes(&bytes)?;
                if consumed != bytes.len() {
                    return Err(StateModelError::TrailingBytes {
                        remaining: bytes.len() - consumed,
                    });
                }
                Ok(Some(state))
            }
            None => Ok(None),
        }
    }

    /// Deletes an account state.
    pub fn delete_account(&self, account_id: &AccountId) -> Result<(), StateModelError> {
        let key = account_id.to_bytes();
        self.accounts
            .remove(key)
            .map_err(|e| StateModelError::StorageError(e.to_string()))?;
        Ok(())
    }

    /// Rebuilds the in-memory ShardStateTree from all accounts stored in the DB.
    pub fn rebuild_shard_state_tree(&self) -> Result<ShardStateTree, StateModelError> {
        let mut tree = ShardStateTree::new();
        for item in self.accounts.iter() {
            let (key, val) = item.map_err(|e| StateModelError::StorageError(e.to_string()))?;
            if key.len() != 32 {
                continue;
            }
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&key);
            let account_id = AccountId::from_bytes(arr);

            let (state, consumed) = AccountState::from_bytes(&val)?;
            if consumed != val.len() {
                return Err(StateModelError::TrailingBytes {
                    remaining: val.len() - consumed,
                });
            }
            tree.insert(account_id, state);
        }
        Ok(tree)
    }

    // --- Block Headers operations ---

    pub fn put_block_header(&self, key: &[u8], header_bytes: &[u8]) -> Result<(), StateModelError> {
        self.block_headers
            .insert(key, header_bytes)
            .map_err(|e| StateModelError::StorageError(e.to_string()))?;
        Ok(())
    }

    pub fn get_block_header(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StateModelError> {
        let val = self
            .block_headers
            .get(key)
            .map_err(|e| StateModelError::StorageError(e.to_string()))?;
        Ok(val.map(|v| v.to_vec()))
    }

    // --- Shard States operations ---

    pub fn put_shard_state(
        &self,
        key: &[u8],
        shard_state_bytes: &[u8],
    ) -> Result<(), StateModelError> {
        self.shard_states
            .insert(key, shard_state_bytes)
            .map_err(|e| StateModelError::StorageError(e.to_string()))?;
        Ok(())
    }

    pub fn get_shard_state(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StateModelError> {
        let val = self
            .shard_states
            .get(key)
            .map_err(|e| StateModelError::StorageError(e.to_string()))?;
        Ok(val.map(|v| v.to_vec()))
    }

    // --- Atomic Batch Writes ---

    /// Commits a block update batch atomically across column family trees.
    pub fn commit_block_batch(&self, batch: BlockCommitBatch) -> Result<(), StateModelError> {
        // Execute atomic transaction using Sled transaction on trees
        let cells = &self.cells;
        let accounts = &self.accounts;
        let block_headers = &self.block_headers;
        let shard_states = &self.shard_states;
        let cell_ref_counts = &self.cell_ref_counts;

        (
            cells,
            accounts,
            block_headers,
            shard_states,
            cell_ref_counts,
        )
            .transaction(|(t_cells, t_accounts, t_headers, t_shards, t_refs)| {
                // 1. Insert cells and update ref counts
                for (hash, cell) in &batch.cells_to_add {
                    t_cells.insert(hash, cell.to_bytes())?;

                    let cur_ref_bytes = t_refs.get(hash)?;
                    let cur_ref = cur_ref_bytes
                        .as_ref()
                        .map(|b| u64::from_be_bytes(b.as_ref().try_into().unwrap()))
                        .unwrap_or(0);
                    let new_ref = cur_ref.saturating_add(1);
                    t_refs.insert(hash, &new_ref.to_be_bytes())?;
                }

                // 2. Upsert accounts
                for (acc_id, state) in &batch.accounts_to_upsert {
                    t_accounts.insert(acc_id.to_bytes().as_ref(), state.to_bytes())?;
                }

                // 3. Delete accounts
                for acc_id in &batch.accounts_to_delete {
                    t_accounts.remove(acc_id.to_bytes().as_ref())?;
                }

                // 4. Block headers
                for (k, v) in &batch.block_headers {
                    t_headers.insert(k.as_slice(), v.as_slice())?;
                }

                // 5. Shard states
                for (k, v) in &batch.shard_states {
                    t_shards.insert(k.as_slice(), v.as_slice())?;
                }

                // 6. Decrement ref counts and GC cells
                for hash in &batch.cells_to_decrement_ref {
                    let cur_ref_bytes = t_refs.get(hash)?;
                    if let Some(b) = cur_ref_bytes {
                        let cur_ref = u64::from_be_bytes(b.as_ref().try_into().unwrap());
                        if cur_ref <= 1 {
                            t_refs.remove(hash)?;
                            t_cells.remove(hash)?;
                        } else {
                            let new_ref = cur_ref - 1;
                            t_refs.insert(hash, &new_ref.to_be_bytes())?;
                        }
                    }
                }

                Ok(())
            })
            .map_err(|e: sled::transaction::TransactionError<std::io::Error>| {
                StateModelError::StorageError(e.to_string())
            })?;

        Ok(())
    }

    // --- Internal Helpers ---

    pub fn get_ref_count(&self, hash: &[u8; 32]) -> Result<u64, StateModelError> {
        let val = self
            .cell_ref_counts
            .get(hash)
            .map_err(|e| StateModelError::StorageError(e.to_string()))?;

        match val {
            Some(bytes) => {
                if bytes.len() == 8 {
                    Ok(u64::from_be_bytes(bytes.as_ref().try_into().unwrap()))
                } else {
                    Ok(0)
                }
            }
            None => Ok(0),
        }
    }

    fn increment_ref_count(&self, hash: &[u8; 32], delta: u64) -> Result<u64, StateModelError> {
        let cur = self.get_ref_count(hash)?;
        let next = cur.saturating_add(delta);
        self.cell_ref_counts
            .insert(hash, &next.to_be_bytes())
            .map_err(|e| StateModelError::StorageError(e.to_string()))?;
        Ok(next)
    }

    fn decrement_ref_count(&self, hash: &[u8; 32], delta: u64) -> Result<u64, StateModelError> {
        let cur = self.get_ref_count(hash)?;
        let next = cur.saturating_sub(delta);
        if next == 0 {
            self.cell_ref_counts
                .remove(hash)
                .map_err(|e| StateModelError::StorageError(e.to_string()))?;
        } else {
            self.cell_ref_counts
                .insert(hash, &next.to_be_bytes())
                .map_err(|e| StateModelError::StorageError(e.to_string()))?;
        }
        Ok(next)
    }
}
