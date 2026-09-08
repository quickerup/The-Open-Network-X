use crate::{
    account::AccountState,
    error::StateModelError,
    tree::{MerkleNode, ShardStateTree, ONX_SHARD_TREE_NODE_TAG},
};
use onx_data_structures::AccountId;
use onx_primitives::{domain_hash, integers::Uint16};

/// Magic bytes header for Merkle proofs ("MPRF" = 0x4D505246) per docs/specification/state-model.md §4.4.
pub const MERKLE_PROOF_MAGIC: [u8; 4] = *b"MPRF";

/// A Merkle proof for verifying an account state against a shard state root hash (§4.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleProof {
    pub target_key: AccountId,
    pub root_hash: [u8; 32],
    pub account_state: Option<AccountState>,
    pub audit_path: Vec<[u8; 32]>,
}

impl MerkleProof {
    /// Generates a MerkleProof for `target_key` from a `ShardStateTree`.
    pub fn generate(tree: &ShardStateTree, target_key: AccountId) -> Result<Self, StateModelError> {
        let root_node = tree.build_merkle_tree();
        let root_hash = root_node.hash();
        let account_state = tree.get(&target_key).cloned();

        let mut audit_path = Vec::new();
        Self::collect_path(&root_node, &target_key, 0, &mut audit_path);

        Ok(Self {
            target_key,
            root_hash,
            account_state,
            audit_path,
        })
    }

    fn collect_path(
        node: &MerkleNode,
        target_key: &AccountId,
        depth: usize,
        path: &mut Vec<[u8; 32]>,
    ) -> bool {
        match node {
            MerkleNode::Empty => false,
            MerkleNode::Leaf { account_id, .. } => account_id == target_key,
            MerkleNode::Branch { left, right } => {
                let byte_idx = depth / 8;
                let bit_idx = 7 - (depth % 8);
                let bit = (target_key.0 .0[byte_idx] >> bit_idx) & 1;

                if bit == 0 {
                    if Self::collect_path(left, target_key, depth + 1, path) {
                        path.push(right.hash());
                        true
                    } else {
                        false
                    }
                } else if Self::collect_path(right, target_key, depth + 1, path) {
                    path.push(left.hash());
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Verifies that this Merkle proof evaluates to the claimed `root_hash`.
    pub fn verify(&self) -> Result<bool, StateModelError> {
        let mut current_hash = match &self.account_state {
            Some(state) => MerkleNode::Leaf {
                account_id: self.target_key,
                state: state.clone(),
            }
            .hash(),
            None => MerkleNode::Empty.hash(),
        };

        // audit_path contains siblings from deepest level (index 0) up to root (index len - 1)
        for (i, sibling_hash) in self.audit_path.iter().enumerate() {
            let depth = self.audit_path.len() - 1 - i;
            let byte_idx = depth / 8;
            let bit_idx = 7 - (depth % 8);
            let bit = (self.target_key.0 .0[byte_idx] >> bit_idx) & 1;

            let mut buf = Vec::with_capacity(4 + 64);
            buf.extend_from_slice(b"NODE");
            if bit == 0 {
                buf.extend_from_slice(&current_hash);
                buf.extend_from_slice(sibling_hash);
            } else {
                buf.extend_from_slice(sibling_hash);
                buf.extend_from_slice(&current_hash);
            }
            current_hash = domain_hash(&ONX_SHARD_TREE_NODE_TAG, &buf);
        }

        Ok(current_hash == self.root_hash)
    }

    /// Serializes the Merkle proof object into canonical binary form (§4.4).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&MERKLE_PROOF_MAGIC);
        buf.extend_from_slice(&self.target_key.0 .0);
        buf.extend_from_slice(&self.root_hash);
        buf.extend_from_slice(&Uint16::from(self.audit_path.len() as u16).encode());

        for sibling in &self.audit_path {
            buf.extend_from_slice(sibling);
        }

        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::AccountStateRecord;
    use onx_primitives::Uint256;

    #[test]
    fn test_merkle_proof_generation_and_verification_multi_element() {
        let mut tree = ShardStateTree::new();

        // Construct 4 account IDs with different prefixes to ensure multi-level tree branch
        let mut key1 = [0u8; 32];
        key1[0] = 0b0000_0000;
        let id1 = AccountId(Uint256(key1));

        let mut key2 = [0u8; 32];
        key2[0] = 0b1000_0000;
        let id2 = AccountId(Uint256(key2));

        let mut key3 = [0u8; 32];
        key3[0] = 0b0100_0000;
        let id3 = AccountId(Uint256(key3));

        let state = AccountState::Active(AccountStateRecord {
            balance_nanos: 1000,
            last_trans_lt: 1,
            code_hash: [0; 32],
            data_hash: [0; 32],
            cell_count: 1,
            byte_count: 10,
        });

        tree.insert(id1, state.clone());
        tree.insert(id2, state.clone());
        tree.insert(id3, state.clone());

        let proof1 = MerkleProof::generate(&tree, id1).unwrap();
        assert!(proof1.verify().unwrap());

        let proof2 = MerkleProof::generate(&tree, id2).unwrap();
        assert!(proof2.verify().unwrap());

        let proof3 = MerkleProof::generate(&tree, id3).unwrap();
        assert!(proof3.verify().unwrap());
    }
}
