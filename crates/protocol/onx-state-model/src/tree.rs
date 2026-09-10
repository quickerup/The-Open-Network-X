use crate::account::AccountState;
use crate::boc::BagOfCells;
use crate::cell::Cell;
use crate::error::StateModelError;
use onx_data_structures::AccountId;
use onx_primitives::{domain_hash, DomainTag, Uint32};
use std::collections::BTreeMap;

pub const ONX_TRIE_NODE_TAG: DomainTag = DomainTag::from_ascii("ONX_TRIE_NODE_V1");
pub const MERKLE_PROOF_MAGIC: u32 = 0x4D505246; // "MPRF"

/// Shard State Tree mapping account IDs to account states using a binary trie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShardStateTree {
    accounts: BTreeMap<AccountId, AccountState>,
}

impl ShardStateTree {
    pub fn new() -> Self {
        Self {
            accounts: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, account_id: AccountId, state: AccountState) {
        self.accounts.insert(account_id, state);
    }

    pub fn get(&self, account_id: &AccountId) -> Option<&AccountState> {
        self.accounts.get(account_id)
    }

    pub fn accounts(&self) -> &BTreeMap<AccountId, AccountState> {
        &self.accounts
    }

    /// Computes the 32-byte Merkle root hash of the shard account state tree.
    pub fn state_root_hash(&self) -> [u8; 32] {
        let items: Vec<([u8; 32], Vec<u8>)> = self
            .accounts
            .iter()
            .map(|(acc_id, state)| (acc_id.to_bytes(), state.to_bytes()))
            .collect();

        let mut dummy_map = std::collections::HashMap::new();
        let root_cell = match build_trie_cells(&items, 0, &mut dummy_map) {
            Ok(cell) => cell,
            Err(_) => return domain_hash(&ONX_TRIE_NODE_TAG, b"EMPTY_TREE"),
        };
        root_cell.hash()
    }

    /// Generates a Merkle proof for a given account ID.
    pub fn generate_proof(
        &self,
        target_account_id: AccountId,
    ) -> Result<MerkleProof, StateModelError> {
        let target_key = target_account_id.to_bytes();

        let items: Vec<([u8; 32], Vec<u8>)> = self
            .accounts
            .iter()
            .map(|(acc_id, state)| (acc_id.to_bytes(), state.to_bytes()))
            .collect();

        let mut proof_cells = std::collections::HashMap::new();
        let root_cell = build_trie_cells(&items, 0, &mut proof_cells)?;
        let root_hash = root_cell.hash();
        let proof_boc = BagOfCells::new(root_hash, proof_cells)?;

        Ok(MerkleProof {
            magic_bytes: MERKLE_PROOF_MAGIC,
            target_key,
            root_hash,
            proof_boc,
        })
    }
}

impl Default for ShardStateTree {
    fn default() -> Self {
        Self::new()
    }
}

fn build_trie_cells(
    items: &[([u8; 32], Vec<u8>)],
    bit_depth: usize,
    cell_map: &mut std::collections::HashMap<[u8; 32], Cell>,
) -> Result<Cell, StateModelError> {
    if items.is_empty() {
        let empty_cell = Cell::new(b"EMPTY_SUBTREE".to_vec(), vec![])?;
        let hash = empty_cell.hash();
        cell_map.insert(hash, empty_cell.clone());
        return Ok(empty_cell);
    }

    if items.len() == 1 || bit_depth >= 256 {
        let (key, val) = &items[0];

        // Chunk value if > 128 bytes (for large state records)
        let mut val_chunks = Vec::new();
        for chunk in val.chunks(128) {
            let chunk_cell = Cell::new(chunk.to_vec(), vec![])?;
            let chunk_hash = chunk_cell.hash();
            cell_map.insert(chunk_hash, chunk_cell);
            val_chunks.push(chunk_hash);
        }

        let leaf_cell = Cell::new(key.to_vec(), val_chunks)?;
        let hash = leaf_cell.hash();
        cell_map.insert(hash, leaf_cell.clone());
        return Ok(leaf_cell);
    }

    let byte_idx = bit_depth / 8;
    let bit_idx = 7 - (bit_depth % 8);

    let (left, right): (Vec<_>, Vec<_>) = items
        .iter()
        .cloned()
        .partition(|(key, _)| (key[byte_idx] & (1 << bit_idx)) == 0);

    let left_cell = build_trie_cells(&left, bit_depth + 1, cell_map)?;
    let right_cell = build_trie_cells(&right, bit_depth + 1, cell_map)?;

    let branch_cell = Cell::new(vec![], vec![left_cell.hash(), right_cell.hash()])?;
    let hash = branch_cell.hash();
    cell_map.insert(hash, branch_cell.clone());
    Ok(branch_cell)
}

/// Merkle proof structure per docs/specification/state-model.md §4.4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleProof {
    pub magic_bytes: u32,
    pub target_key: [u8; 32],
    pub root_hash: [u8; 32],
    pub proof_boc: BagOfCells,
}

impl MerkleProof {
    /// Verifies the Merkle proof against the declared root_hash and target_key.
    pub fn verify(&self) -> Result<(), StateModelError> {
        if self.magic_bytes != MERKLE_PROOF_MAGIC {
            return Err(StateModelError::InvalidMerkleProof(format!(
                "Invalid magic bytes: expected {:#010x}, got {:#010x}",
                MERKLE_PROOF_MAGIC, self.magic_bytes
            )));
        }

        self.proof_boc.verify_dag()?;

        if self.proof_boc.root_hash() != &self.root_hash {
            return Err(StateModelError::InvalidMerkleProof(format!(
                "Proof root hash {:?} does not match expected root hash {:?}",
                self.proof_boc.root_hash(),
                self.root_hash
            )));
        }

        Ok(())
    }

    /// Serializes the MerkleProof object to binary bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&Uint32(self.magic_bytes).encode());
        bytes.extend_from_slice(&self.target_key);
        bytes.extend_from_slice(&self.root_hash);
        let boc_bytes = self.proof_boc.to_bytes();
        bytes.extend_from_slice(&boc_bytes);
        bytes
    }

    /// Deserializes a MerkleProof object from binary bytes.
    pub fn from_bytes(slice: &[u8]) -> Result<(Self, usize), StateModelError> {
        if slice.len() < 4 + 32 + 32 {
            return Err(StateModelError::DeserializationError(
                "Slice too short for MerkleProof header".to_string(),
            ));
        }

        let mut cursor = slice;
        let magic_val = Uint32::read(&mut cursor)
            .map_err(|e| StateModelError::DeserializationError(e.to_string()))?;
        let magic_bytes = magic_val.0;
        let mut offset = Uint32::BYTE_LEN;

        let mut target_key = [0u8; 32];
        target_key.copy_from_slice(&cursor[..32]);
        cursor = &cursor[32..];
        offset += 32;

        let mut root_hash = [0u8; 32];
        root_hash.copy_from_slice(&cursor[..32]);
        cursor = &cursor[32..];
        offset += 32;

        let (proof_boc, consumed) = BagOfCells::from_bytes(cursor)?;
        offset += consumed;

        let proof = Self {
            magic_bytes,
            target_key,
            root_hash,
            proof_boc,
        };
        proof.verify()?;

        Ok((proof, offset))
    }
}
