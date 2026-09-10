//! Deterministic header synchronization for ONX block chains.
//!
//! A batch is processed in ancestor-first order and is committed only when
//! every header, masterchain reference, and shard canonicality proof checks
//! out. This keeps a partially downloaded batch from changing local sync
//! state. See `docs/specification/blocks.md` §3.1–§3.3.

use crate::{
    flags, validate_block_successor, validate_master_ref, BlocksError, MasterchainBlockExtra,
    RecomputedRoots,
};
use onx_data_structures::BlockHeader;
use onx_primitives::Uint256;
use std::{collections::HashMap, fmt};

/// A downloaded header together with the data required for deterministic
/// relative-validity and masterchain-canonicality checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncCandidate {
    pub header: BlockHeader,
    pub recomputed_roots: RecomputedRoots,
    /// Required for masterchain headers and forbidden for shardchain headers.
    pub masterchain_extra: Option<MasterchainBlockExtra>,
    /// The held masterchain block whose shard configuration commits this
    /// shardchain header. It is required for shardchain candidates and
    /// forbidden for masterchain candidates.
    pub canonicality_proof: Option<Uint256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StoredBlock {
    header: BlockHeader,
    masterchain_extra: Option<MasterchainBlockExtra>,
}

/// In-memory, deterministic sync index. Storage and network fetching are
/// deliberately outside this type: callers supply the verified download data.
#[derive(Debug, Clone, Default)]
pub struct BlockSyncEngine {
    blocks: HashMap<Uint256, StoredBlock>,
}

/// Reasons an imported synchronization batch is rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    Block(BlocksError),
    DuplicateBlock(Uint256),
    MissingParent(Uint256),
    MissingMasterchainProof(Uint256),
    MissingMasterchainExtra,
    UnexpectedMasterchainExtra,
    MissingCanonicalityProof,
    UnexpectedCanonicalityProof,
    InvalidCanonicalityProof(Uint256),
}

impl From<BlocksError> for SyncError {
    fn from(error: BlocksError) -> Self {
        Self::Block(error)
    }
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Block(error) => write!(f, "block validation failed: {error}"),
            Self::DuplicateBlock(hash) => write!(f, "block {hash:?} is already indexed"),
            Self::MissingParent(hash) => write!(f, "parent block {hash:?} is unavailable"),
            Self::MissingMasterchainProof(hash) => {
                write!(f, "masterchain block {hash:?} is unavailable")
            }
            Self::MissingMasterchainExtra => {
                write!(f, "masterchain block has no shard configuration")
            }
            Self::UnexpectedMasterchainExtra => {
                write!(f, "shardchain block carries masterchain-only data")
            }
            Self::MissingCanonicalityProof => {
                write!(f, "shardchain block has no canonicality proof")
            }
            Self::UnexpectedCanonicalityProof => {
                write!(f, "masterchain block carries a canonicality proof")
            }
            Self::InvalidCanonicalityProof(hash) => {
                write!(
                    f,
                    "masterchain block {hash:?} does not commit the shard block"
                )
            }
        }
    }
}

impl std::error::Error for SyncError {}

impl BlockSyncEngine {
    /// Adds a previously verified anchor (typically genesis or a trusted
    /// checkpoint). Subsequent batches are always fully verified.
    pub fn insert_trusted(
        &mut self,
        header: BlockHeader,
        extra: Option<MasterchainBlockExtra>,
    ) -> Result<(), SyncError> {
        let candidate = SyncCandidate {
            canonicality_proof: if header.shard.workchain_id.is_masterchain() {
                None
            } else {
                Some(Uint256::ZERO)
            },
            recomputed_roots: RecomputedRoots {
                state_root_hash: header.state_root_hash,
                in_msg_root_hash: header.in_msg_root_hash,
                out_msg_root_hash: header.out_msg_root_hash,
            },
            header,
            masterchain_extra: extra,
        };
        // Trusted anchors do not require parent/master-reference resolution,
        // but their role-specific serialization remains checked.
        self.validate_role_fields(&candidate)?;
        let hash = candidate.header.block_hash();
        if self.blocks.contains_key(&hash) {
            return Err(SyncError::DuplicateBlock(hash));
        }
        self.blocks.insert(
            hash,
            StoredBlock {
                header: candidate.header,
                masterchain_extra: candidate.masterchain_extra,
            },
        );
        Ok(())
    }

    /// Validates and atomically indexes an ancestor-first download batch.
    pub fn import_batch(&mut self, candidates: &[SyncCandidate]) -> Result<(), SyncError> {
        let mut staged = self.blocks.clone();
        for candidate in candidates {
            self.validate_candidate(candidate, &mut staged)?;
        }
        self.blocks = staged;
        Ok(())
    }

    /// Validates and indexes a descendant-first response from a backwards
    /// header request. The response is reversed before normal validation, so
    /// parent links receive exactly the same checks as forward synchronization.
    pub fn import_reverse_batch(&mut self, candidates: &[SyncCandidate]) -> Result<(), SyncError> {
        let mut ancestor_first = candidates.to_vec();
        ancestor_first.reverse();
        self.import_batch(&ancestor_first)
    }

    /// Returns a held header by hash.
    pub fn header(&self, hash: &Uint256) -> Option<&BlockHeader> {
        self.blocks.get(hash).map(|block| &block.header)
    }

    fn validate_role_fields(&self, candidate: &SyncCandidate) -> Result<(), SyncError> {
        if candidate.header.shard.workchain_id.is_masterchain() {
            let extra = candidate
                .masterchain_extra
                .as_ref()
                .ok_or(SyncError::MissingMasterchainExtra)?;
            extra.validate()?;
            if candidate.canonicality_proof.is_some() {
                return Err(SyncError::UnexpectedCanonicalityProof);
            }
        } else {
            if candidate.masterchain_extra.is_some() {
                return Err(SyncError::UnexpectedMasterchainExtra);
            }
            if candidate.canonicality_proof.is_none() {
                return Err(SyncError::MissingCanonicalityProof);
            }
        }
        Ok(())
    }

    fn validate_candidate(
        &self,
        candidate: &SyncCandidate,
        staged: &mut HashMap<Uint256, StoredBlock>,
    ) -> Result<(), SyncError> {
        self.validate_role_fields(candidate)?;
        let hash = candidate.header.block_hash();
        if staged.contains_key(&hash) {
            return Err(SyncError::DuplicateBlock(hash));
        }

        let parent = staged
            .get(&candidate.header.prev_ref_hash)
            .ok_or(SyncError::MissingParent(candidate.header.prev_ref_hash))?;
        let parent_2 = if flags::is_merge_result(candidate.header.flags.0) {
            Some(
                &staged
                    .get(&candidate.header.prev_ref_hash_2)
                    .ok_or(SyncError::MissingParent(candidate.header.prev_ref_hash_2))?
                    .header,
            )
        } else {
            None
        };
        validate_block_successor(
            &candidate.header,
            &parent.header,
            parent_2,
            candidate.recomputed_roots,
        )?;

        if candidate.header.shard.workchain_id.is_masterchain() {
            validate_master_ref(&candidate.header, None)?;
        } else {
            let master = staged.get(&candidate.header.master_ref_hash).ok_or(
                SyncError::MissingMasterchainProof(candidate.header.master_ref_hash),
            )?;
            validate_master_ref(&candidate.header, Some(&master.header))?;
            let proof_hash = candidate.canonicality_proof.expect("checked above");
            let proof = staged
                .get(&proof_hash)
                .ok_or(SyncError::MissingMasterchainProof(proof_hash))?;
            let extra = proof
                .masterchain_extra
                .as_ref()
                .ok_or(SyncError::MissingMasterchainExtra)?;
            if !extra.is_canonical(&candidate.header.shard, hash, candidate.header.seq_no.0) {
                return Err(SyncError::InvalidCanonicalityProof(proof_hash));
            }
        }
        staged.insert(
            hash,
            StoredBlock {
                header: candidate.header.clone(),
                masterchain_extra: candidate.masterchain_extra.clone(),
            },
        );
        Ok(())
    }
}
