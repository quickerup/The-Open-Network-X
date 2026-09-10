//! Masterchain coupling, canonicality, and the `Masterchain Block Extra`
//! shard-configuration commitment, per `docs/specification/blocks.md` §3.3,
//! §4.1, and §5 rules 7-8.

use crate::error::BlocksError;
use onx_data_structures::{BlockHeader, DataStructureError, ShardIdent};
use onx_primitives::{Uint256, Uint32};

/// One entry of a `Masterchain Block Extra`'s shard configuration: the most
/// recent block hash and sequence number of one active shard, per §4.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShardEntry {
    pub shard: ShardIdent,
    pub block_hash: Uint256,
    pub seq_no: Uint32,
}

impl ShardEntry {
    /// Exact byte length of a serialized `ShardEntry` (48 bytes).
    pub const BYTE_LENGTH: usize = ShardIdent::BYTE_LENGTH + Uint256::BYTE_LEN + Uint32::BYTE_LEN;

    /// Serializes this entry to 48 canonical binary bytes.
    pub fn to_bytes(&self) -> [u8; Self::BYTE_LENGTH] {
        let mut buf = [0u8; Self::BYTE_LENGTH];
        buf[0..12].copy_from_slice(&self.shard.to_bytes());
        buf[12..44].copy_from_slice(&self.block_hash.encode());
        buf[44..48].copy_from_slice(&self.seq_no.encode());
        buf
    }

    /// Deserializes one entry from exactly 48 bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BlocksError> {
        if bytes.len() < Self::BYTE_LENGTH {
            return Err(DataStructureError::TruncatedInput {
                expected: Self::BYTE_LENGTH,
                got: bytes.len(),
            }
            .into());
        }
        if bytes.len() > Self::BYTE_LENGTH {
            return Err(DataStructureError::TrailingBytes {
                remaining: bytes.len() - Self::BYTE_LENGTH,
            }
            .into());
        }
        let shard = ShardIdent::from_bytes(&bytes[0..12])?;
        let block_hash = Uint256::decode_exact(&bytes[12..44]).map_err(DataStructureError::from)?;
        let seq_no = Uint32::decode_exact(&bytes[44..48]).map_err(DataStructureError::from)?;
        Ok(Self {
            shard,
            block_hash,
            seq_no,
        })
    }
}

/// Present only in masterchain blocks, in addition to the shared
/// `BlockHeader`: commits to the most recent block hash and sequence number
/// of every active shard, per §4.1.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MasterchainBlockExtra {
    pub shard_entries: Vec<ShardEntry>,
}

impl MasterchainBlockExtra {
    /// Serializes to a count-prefixed array of `ShardEntry` per §4.1.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + self.shard_entries.len() * ShardEntry::BYTE_LENGTH);
        buf.extend_from_slice(&Uint32(self.shard_entries.len() as u32).encode());
        for entry in &self.shard_entries {
            buf.extend_from_slice(&entry.to_bytes());
        }
        buf
    }

    /// Deserializes from exactly one canonical binary representation and
    /// validates §5 rule 7 (sorted, non-duplicate `shard_entries`).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BlocksError> {
        if bytes.len() < 4 {
            return Err(DataStructureError::TruncatedInput {
                expected: 4,
                got: bytes.len(),
            }
            .into());
        }
        let count = Uint32::decode_exact(&bytes[0..4])
            .map_err(DataStructureError::from)?
            .0 as usize;
        let entries_len = count.checked_mul(ShardEntry::BYTE_LENGTH).ok_or(
            DataStructureError::TruncatedInput {
                expected: usize::MAX,
                got: bytes.len(),
            },
        )?;
        let expected =
            4usize
                .checked_add(entries_len)
                .ok_or(DataStructureError::TruncatedInput {
                    expected: usize::MAX,
                    got: bytes.len(),
                })?;
        if bytes.len() < expected {
            return Err(DataStructureError::TruncatedInput {
                expected,
                got: bytes.len(),
            }
            .into());
        }
        if bytes.len() > expected {
            return Err(DataStructureError::TrailingBytes {
                remaining: bytes.len() - expected,
            }
            .into());
        }

        let mut shard_entries = Vec::with_capacity(count);
        let mut offset = 4;
        for _ in 0..count {
            let entry = ShardEntry::from_bytes(&bytes[offset..offset + ShardEntry::BYTE_LENGTH])?;
            shard_entries.push(entry);
            offset += ShardEntry::BYTE_LENGTH;
        }

        let extra = Self { shard_entries };
        extra.validate()?;
        Ok(extra)
    }

    /// Validates §5 rule 7: `shard_entries` sorted by each shard's
    /// big-endian byte encoding, with no duplicate shard.
    ///
    /// Compares `ShardIdent::to_bytes()` output directly rather than
    /// `ShardIdent`'s derived `Ord` impl: the latter compares
    /// `workchain_id` as a signed `Int32`, under which the masterchain
    /// (`workchain_id = -1`) sorts *before* workchain 0, whereas its
    /// big-endian byte encoding (`0xFFFFFFFF`) sorts *after* it. The spec's
    /// "big-endian byte encoding" ordering means the latter.
    pub fn validate(&self) -> Result<(), BlocksError> {
        for pair in self.shard_entries.windows(2) {
            let a = pair[0].shard.to_bytes();
            let b = pair[1].shard.to_bytes();
            if a == b {
                return Err(BlocksError::DuplicateShardEntry);
            }
            if a > b {
                return Err(BlocksError::UnsortedShardEntries);
            }
        }
        Ok(())
    }

    /// True if `shard`'s block `block_hash` at `seq_no` is committed in this
    /// masterchain block's shard configuration — i.e. per §3.3, that
    /// shardchain block (and its ancestors) are canonical relative to the
    /// masterchain block this `MasterchainBlockExtra` belongs to.
    pub fn is_canonical(&self, shard: &ShardIdent, block_hash: Uint256, seq_no: u32) -> bool {
        self.shard_entries.iter().any(|entry| {
            &entry.shard == shard && entry.block_hash == block_hash && entry.seq_no.0 == seq_no
        })
    }
}

/// Validates `header.master_ref_hash` per §3.3 and §5 rule 8.
///
/// For a masterchain block (`header.shard.workchain_id.is_masterchain()`),
/// `master_ref_hash` must be all-zero; `resolved_masterchain_block` is
/// ignored. For a shardchain block, `resolved_masterchain_block` must be
/// `Some` and its hash must equal `header.master_ref_hash` — a caller unable
/// to resolve the referenced masterchain block simply passes `None`,
/// modeling "does not correspond to a masterchain block the node holds or
/// can obtain and verify".
pub fn validate_master_ref(
    header: &BlockHeader,
    resolved_masterchain_block: Option<&BlockHeader>,
) -> Result<(), BlocksError> {
    if header.shard.workchain_id.is_masterchain() {
        if header.master_ref_hash != Uint256::ZERO {
            return Err(BlocksError::InvalidMasterchainReference);
        }
        return Ok(());
    }

    match resolved_masterchain_block {
        Some(masterchain_block)
            if masterchain_block.shard.workchain_id.is_masterchain()
                && header.master_ref_hash == masterchain_block.block_hash() =>
        {
            Ok(())
        }
        _ => Err(BlocksError::InvalidMasterchainReference),
    }
}
