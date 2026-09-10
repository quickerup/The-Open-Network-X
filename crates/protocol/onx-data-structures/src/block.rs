//! Block Header structure per docs/specification/data-structures.md §4.4.

use crate::error::DataStructureError;
use crate::shard::ShardIdent;
use onx_primitives::{
    domain_hash, hash::BLOCK_HEADER_V1, DomainTag, Uint16, Uint256, Uint32, Uint64,
};

/// Block Header fixed magic constructor (`0x1F2E3D4C`).
pub const BLOCK_HEADER_MAGIC: u32 = 0x1F2E3D4C;

/// Flag bit indicating a merge-result block (ADR-0016, data-structures.md §4.4).
pub const MERGE_RESULT_FLAG: u16 = 0x0010;

/// Fixed binary block header layout (`BlockHeader`, 242 bytes total, amended by ADR-0016).
///
/// Layout:
/// 1. `magic_constructor` : uint32  (4 bytes: 0x1F2E3D4C)
/// 2. `workchain_id`      : int32   (4 bytes)
/// 3. `shard_prefix`      : uint64  (8 bytes)
/// 4. `seq_no`            : uint32  (4 bytes, sequence number)
/// 5. `flags`             : uint16  (2 bytes; bit 4 = MERGE_RESULT)
/// 6. `gen_utime`         : uint32  (4 bytes, unix timestamp)
/// 7. `start_lt`          : uint64  (8 bytes)
/// 8. `end_lt`            : uint64  (8 bytes)
/// 9. `prev_key_block`    : uint32  (4 bytes)
/// 10. `prev_ref_hash`    : uint256 (32 bytes, parent block hash)
/// 11. `prev_ref_hash_2`  : uint256 (32 bytes, second parent block hash; zero unless MERGE_RESULT)
/// 12. `master_ref_hash`  : uint256 (32 bytes, latest masterchain block hash, zero if masterchain)
/// 13. `state_root_hash`  : uint256 (32 bytes, state Bag-of-Cells root hash)
/// 14. `in_msg_root_hash` : uint256 (32 bytes, input message Merkle tree root)
/// 15. `out_msg_root_hash`: uint256 (32 bytes, output message Merkle tree root)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockHeader {
    pub magic_constructor: Uint32,
    pub shard: ShardIdent,
    pub seq_no: Uint32,
    pub flags: Uint16,
    pub gen_utime: Uint32,
    pub start_lt: Uint64,
    pub end_lt: Uint64,
    pub prev_key_block: Uint32,
    pub prev_ref_hash: Uint256,
    pub prev_ref_hash_2: Uint256,
    pub master_ref_hash: Uint256,
    pub state_root_hash: Uint256,
    pub in_msg_root_hash: Uint256,
    pub out_msg_root_hash: Uint256,
}

impl BlockHeader {
    /// Exact byte length of serialized BlockHeader (242 bytes).
    pub const BYTE_LENGTH: usize = 4 + 12 + 4 + 2 + 4 + 8 + 8 + 4 + 32 + 32 + 32 + 32 + 32 + 32;

    /// Domain tag for block header hashing.
    pub const DOMAIN_TAG: DomainTag = BLOCK_HEADER_V1;

    /// Serializes BlockHeader to 242 canonical binary bytes.
    pub fn to_bytes(&self) -> [u8; Self::BYTE_LENGTH] {
        let mut buf = [0u8; Self::BYTE_LENGTH];
        let mut offset = 0;

        buf[offset..offset + 4].copy_from_slice(&self.magic_constructor.encode());
        offset += 4;

        buf[offset..offset + 12].copy_from_slice(&self.shard.to_bytes());
        offset += 12;

        buf[offset..offset + 4].copy_from_slice(&self.seq_no.encode());
        offset += 4;

        buf[offset..offset + 2].copy_from_slice(&self.flags.encode());
        offset += 2;

        buf[offset..offset + 4].copy_from_slice(&self.gen_utime.encode());
        offset += 4;

        buf[offset..offset + 8].copy_from_slice(&self.start_lt.encode());
        offset += 8;

        buf[offset..offset + 8].copy_from_slice(&self.end_lt.encode());
        offset += 8;

        buf[offset..offset + 4].copy_from_slice(&self.prev_key_block.encode());
        offset += 4;

        buf[offset..offset + 32].copy_from_slice(&self.prev_ref_hash.encode());
        offset += 32;

        buf[offset..offset + 32].copy_from_slice(&self.prev_ref_hash_2.encode());
        offset += 32;

        buf[offset..offset + 32].copy_from_slice(&self.master_ref_hash.encode());
        offset += 32;

        buf[offset..offset + 32].copy_from_slice(&self.state_root_hash.encode());
        offset += 32;

        buf[offset..offset + 32].copy_from_slice(&self.in_msg_root_hash.encode());
        offset += 32;

        buf[offset..offset + 32].copy_from_slice(&self.out_msg_root_hash.encode());

        buf
    }

    /// Deserializes BlockHeader from 242 canonical binary bytes and validates header magic constructor, shard ident, and merge parent reference consistency.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DataStructureError> {
        if bytes.len() < Self::BYTE_LENGTH {
            return Err(DataStructureError::TruncatedInput {
                expected: Self::BYTE_LENGTH,
                got: bytes.len(),
            });
        }
        if bytes.len() > Self::BYTE_LENGTH {
            return Err(DataStructureError::TrailingBytes {
                remaining: bytes.len() - Self::BYTE_LENGTH,
            });
        }

        let mut offset = 0;

        let magic_constructor = Uint32::decode_exact(&bytes[offset..offset + 4])?;
        offset += 4;

        if magic_constructor.0 != BLOCK_HEADER_MAGIC {
            return Err(DataStructureError::HeaderMagicMismatch {
                magic: magic_constructor.0,
            });
        }

        let shard = ShardIdent::from_bytes(&bytes[offset..offset + 12])?;
        offset += 12;

        let seq_no = Uint32::decode_exact(&bytes[offset..offset + 4])?;
        offset += 4;

        let flags = Uint16::decode_exact(&bytes[offset..offset + 2])?;
        offset += 2;

        let gen_utime = Uint32::decode_exact(&bytes[offset..offset + 4])?;
        offset += 4;

        let start_lt = Uint64::decode_exact(&bytes[offset..offset + 8])?;
        offset += 8;

        let end_lt = Uint64::decode_exact(&bytes[offset..offset + 8])?;
        offset += 8;

        let prev_key_block = Uint32::decode_exact(&bytes[offset..offset + 4])?;
        offset += 4;

        let prev_ref_hash = Uint256::decode_exact(&bytes[offset..offset + 32])?;
        offset += 32;

        let prev_ref_hash_2 = Uint256::decode_exact(&bytes[offset..offset + 32])?;
        offset += 32;

        let is_merge_result = (flags.0 & MERGE_RESULT_FLAG) != 0;
        let is_prev_2_zero = prev_ref_hash_2.0 == [0u8; 32];

        if !is_merge_result && !is_prev_2_zero {
            return Err(DataStructureError::MergeParentReferenceInconsistency);
        }
        if is_merge_result && is_prev_2_zero {
            return Err(DataStructureError::MergeParentReferenceInconsistency);
        }

        let master_ref_hash = Uint256::decode_exact(&bytes[offset..offset + 32])?;
        offset += 32;

        let state_root_hash = Uint256::decode_exact(&bytes[offset..offset + 32])?;
        offset += 32;

        let in_msg_root_hash = Uint256::decode_exact(&bytes[offset..offset + 32])?;
        offset += 32;

        let out_msg_root_hash = Uint256::decode_exact(&bytes[offset..offset + 32])?;

        Ok(Self {
            magic_constructor,
            shard,
            seq_no,
            flags,
            gen_utime,
            start_lt,
            end_lt,
            prev_key_block,
            prev_ref_hash,
            prev_ref_hash_2,
            master_ref_hash,
            state_root_hash,
            in_msg_root_hash,
            out_msg_root_hash,
        })
    }

    /// Computes the domain-separated SHA-256 block hash for this header.
    pub fn block_hash(&self) -> Uint256 {
        Uint256(domain_hash(&Self::DOMAIN_TAG, &self.to_bytes()))
    }
}
