use crate::account::AccountState;
use onx_data_structures::AccountId;
use onx_primitives::{domain_hash, DomainTag};
use std::collections::BTreeMap;

/// Domain separation tag for Shard State Merkle Tree node hashing per docs/specification/state-model.md §4.3.
pub const ONX_SHARD_TREE_NODE_TAG: DomainTag = DomainTag::from_ascii("ONX_SHARD_TREE_NODE_V1");

/// A node in the authenticated binary Merkle-Patricia tree mapping 256-bit Account IDs to Account states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MerkleNode {
    Empty,
    Leaf {
        account_id: AccountId,
        state: AccountState,
    },
    Branch {
        left: Box<MerkleNode>,
        right: Box<MerkleNode>,
    },
}

impl MerkleNode {
    /// Computes the domain-separated 32-byte SHA-256 hash of this tree node.
    pub fn hash(&self) -> [u8; 32] {
        match self {
            Self::Empty => domain_hash(&ONX_SHARD_TREE_NODE_TAG, b"EMPTY"),
            Self::Leaf { account_id, state } => {
                let mut buf = Vec::new();
                buf.extend_from_slice(b"LEAF");
                buf.extend_from_slice(&account_id.0.encode());
                buf.push(state.status() as u8);
                if let AccountState::Active(ref rec) = state {
                    buf.extend_from_slice(&rec.to_bytes());
                } else if let AccountState::Frozen { storage_hash } = state {
                    buf.extend_from_slice(storage_hash);
                }
                domain_hash(&ONX_SHARD_TREE_NODE_TAG, &buf)
            }
            Self::Branch { left, right } => {
                let mut buf = Vec::with_capacity(4 + 64);
                buf.extend_from_slice(b"NODE");
                buf.extend_from_slice(&left.hash());
                buf.extend_from_slice(&right.hash());
                domain_hash(&ONX_SHARD_TREE_NODE_TAG, &buf)
            }
        }
    }
}

/// ShardStateTree mapping 256-bit Account IDs to AccountStates using an authenticated Merkle tree (§3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShardStateTree {
    pub accounts: BTreeMap<AccountId, AccountState>,
}

impl ShardStateTree {
    pub fn new() -> Self {
        Self {
            accounts: BTreeMap::new(),
        }
    }

    /// Inserts or updates an account state.
    pub fn insert(&mut self, account_id: AccountId, state: AccountState) {
        self.accounts.insert(account_id, state);
    }

    /// Gets an account state if present.
    pub fn get(&self, account_id: &AccountId) -> Option<&AccountState> {
        self.accounts.get(account_id)
    }

    /// Builds the MerkleNode tree representation.
    pub fn build_merkle_tree(&self) -> MerkleNode {
        let items: Vec<(AccountId, AccountState)> =
            self.accounts.iter().map(|(k, v)| (*k, v.clone())).collect();

        Self::build_recursive(&items, 0)
    }

    fn build_recursive(items: &[(AccountId, AccountState)], depth: usize) -> MerkleNode {
        if items.is_empty() {
            return MerkleNode::Empty;
        }
        if items.len() == 1 || depth >= 256 {
            return MerkleNode::Leaf {
                account_id: items[0].0,
                state: items[0].1.clone(),
            };
        }

        // Split items based on bit at `depth` in AccountId key
        let mut left_items = Vec::new();
        let mut right_items = Vec::new();

        for (id, state) in items {
            let byte_idx = depth / 8;
            let bit_idx = 7 - (depth % 8);
            let bit = (id.0 .0[byte_idx] >> bit_idx) & 1;
            if bit == 0 {
                left_items.push((*id, state.clone()));
            } else {
                right_items.push((*id, state.clone()));
            }
        }

        MerkleNode::Branch {
            left: Box::new(Self::build_recursive(&left_items, depth + 1)),
            right: Box::new(Self::build_recursive(&right_items, depth + 1)),
        }
    }

    /// Computes the 32-byte state root hash for the shard account tree (§3.4).
    pub fn root_hash(&self) -> [u8; 32] {
        self.build_merkle_tree().hash()
    }
}

impl Default for ShardStateTree {
    fn default() -> Self {
        Self::new()
    }
}
