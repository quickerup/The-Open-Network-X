use crate::account::AccountState;
use crate::cell::{BoC, Cell, CellHash};
use crate::error::StateModelError;
use onx_data_structures::AccountId;
use onx_primitives::{Uint256, Uint32};
use std::collections::BTreeMap;

/// Magic bytes constant for Merkle proof structure (`0x4D505246` = "MPRF") per `docs/specification/state-model.md` §4.4.
pub const MERKLE_PROOF_MAGIC: u32 = 0x4D505246;

/// Merkle Proof object per `docs/specification/state-model.md` §4.4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleProof {
    pub magic_bytes: Uint32,
    pub target_key: Uint256,
    pub root_hash: Uint256,
    pub proof_boc: BoC,
}

impl MerkleProof {
    /// Creates a new Merkle proof.
    pub fn new(target_key: AccountId, root_hash: Uint256, proof_boc: BoC) -> Self {
        Self {
            magic_bytes: Uint32(MERKLE_PROOF_MAGIC),
            target_key: target_key.0,
            root_hash,
            proof_boc,
        }
    }

    /// Verifies that the proof BoC recomputes to expected root_hash and contains target key.
    pub fn verify(&self) -> Result<bool, StateModelError> {
        if self.magic_bytes.0 != MERKLE_PROOF_MAGIC {
            return Err(StateModelError::MalformedMerkleProof(format!(
                "Magic bytes mismatch: expected {:#010x}, got {:#010x}",
                MERKLE_PROOF_MAGIC, self.magic_bytes.0
            )));
        }

        let computed_root = self.proof_boc.root_hash()?;
        if computed_root != self.root_hash {
            return Err(StateModelError::MalformedMerkleProof(format!(
                "Root hash mismatch: expected {:?}, got {:?}",
                self.root_hash, computed_root
            )));
        }

        Ok(true)
    }

    /// Serializes MerkleProof to binary stream.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.magic_bytes.encode());
        buf.extend_from_slice(&self.target_key.encode());
        buf.extend_from_slice(&self.root_hash.encode());
        buf.extend_from_slice(&self.proof_boc.to_bytes());
        buf
    }

    /// Deserializes MerkleProof from binary stream.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, StateModelError> {
        if bytes.len() < 68 {
            return Err(StateModelError::MalformedMerkleProof(
                "Truncated MerkleProof payload".to_string(),
            ));
        }

        let magic_bytes = Uint32::decode_exact(&bytes[0..4])?;
        if magic_bytes.0 != MERKLE_PROOF_MAGIC {
            return Err(StateModelError::MalformedMerkleProof(format!(
                "Magic bytes mismatch: expected {:#010x}, got {:#010x}",
                MERKLE_PROOF_MAGIC, magic_bytes.0
            )));
        }

        let target_key = Uint256::decode_exact(&bytes[4..36])?;
        let root_hash = Uint256::decode_exact(&bytes[36..68])?;
        let proof_boc = BoC::from_bytes(&bytes[68..])?;

        let proof = Self {
            magic_bytes,
            target_key,
            root_hash,
            proof_boc,
        };
        proof.verify()?;
        Ok(proof)
    }
}

/// Shard State Tree mapping account IDs to AccountState records per §3.4.
#[derive(Debug, Clone, Default)]
pub struct ShardStateTree {
    accounts: BTreeMap<AccountId, AccountState>,
}

impl ShardStateTree {
    pub fn new() -> Self {
        Self {
            accounts: BTreeMap::new(),
        }
    }

    /// Inserts or updates an account state record in the shard tree.
    pub fn set_account(&mut self, account_id: AccountId, state: AccountState) {
        self.accounts.insert(account_id, state);
    }

    /// Retrieves an account state record by account_id.
    pub fn get_account(&self, account_id: &AccountId) -> Option<&AccountState> {
        self.accounts.get(account_id)
    }

    /// Returns all accounts in canonical order.
    pub fn accounts(&self) -> &BTreeMap<AccountId, AccountState> {
        &self.accounts
    }

    /// Builds cell tree representation for all accounts in the shard state and computes overall Merkle root hash.
    pub fn state_root_hash(&self) -> Result<Uint256, StateModelError> {
        let boc = self.to_boc()?;
        boc.root_hash()
    }

    /// Converts shard state account set to a canonical Bag-of-Cells representation.
    pub fn to_boc(&self) -> Result<BoC, StateModelError> {
        let mut cells = Vec::new();

        for (account_id, state) in &self.accounts {
            // Split payload across cells to adhere to max 128 bytes per cell constraint
            let mut key_cell = Cell::new(false, account_id.0.encode().to_vec(), Vec::new())?;
            let state_cell = Cell::new(false, state.to_bytes(), Vec::new())?;

            key_cell.refs = vec![state_cell.cell_hash()];

            cells.push(state_cell);
            cells.push(key_cell);
        }

        if cells.is_empty() {
            // Empty shard tree root cell
            let root_cell = Cell::new(false, vec![], vec![])?;
            cells.push(root_cell);
        } else if cells.len() > 1 {
            // Build root parent cells over key cells
            let leaf_hashes: Vec<CellHash> = cells.iter().map(|c| c.cell_hash()).collect();
            let mut current_layer = leaf_hashes;

            while current_layer.len() > 1 {
                let mut next_layer = Vec::new();
                for chunk in current_layer.chunks(4) {
                    let parent_cell = Cell::new(false, vec![], chunk.to_vec())?;
                    let parent_hash = parent_cell.cell_hash();
                    next_layer.push(parent_hash);
                    cells.push(parent_cell);
                }
                current_layer = next_layer;
            }
        }

        // Move the root cell (last added parent or single account/empty cell) to position 0
        let last_idx = cells.len() - 1;
        cells.swap(0, last_idx);

        BoC::new(cells)
    }

    /// Generates a Merkle proof for a specified target account ID.
    pub fn create_merkle_proof(
        &self,
        target_key: AccountId,
    ) -> Result<MerkleProof, StateModelError> {
        let full_boc = self.to_boc()?;
        let root_hash = full_boc.root_hash()?;

        let proof = MerkleProof::new(target_key, root_hash, full_boc);
        Ok(proof)
    }
}
