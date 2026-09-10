//! Shard Identifier implementation per docs/specification/data-structures.md §4.2.

use crate::address::{AccountId, FullAddress, WorkchainIdent};
use crate::error::DataStructureError;
use onx_primitives::Uint64;

/// Shard Identifier (`ShardIdent`, 12 bytes total).
///
/// Layout:
/// - `workchain_id`: 4 bytes (`int32` big-endian)
/// - `shard_prefix_ident`: 8 bytes (`uint64` big-endian)
///
/// Encoding rule for `shard_prefix_ident`:
/// A binary prefix string p of length L (0 <= L <= 60) is encoded by setting
/// bit (63 - L) to 1 (the marker bit) and lower bits to 0.
///
/// Example:
/// - Root shard (L=0): bit 63 is 1 -> 0x8000_0000_0000_0000
/// - Prefix '0' (L=1): bit 63 is 0, bit 62 is 1 -> 0x4000_0000_0000_0000
/// - Prefix '1' (L=1): bit 63 is 1, bit 62 is 1 -> 0xC000_0000_0000_0000
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShardIdent {
    pub workchain_id: WorkchainIdent,
    pub shard_prefix_ident: Uint64,
}

impl ShardIdent {
    /// Exact byte size of serialized ShardIdent (12 bytes).
    pub const BYTE_LENGTH: usize = 12;

    /// Root shard prefix ident value (`0x8000_0000_0000_0000`).
    pub const ROOT_PREFIX: u64 = 0x8000_0000_0000_0000;

    /// Creates a ShardIdent from workchain ID and raw `shard_prefix_ident` u64 value, validating marker bit and length limits.
    pub fn new(
        workchain_id: WorkchainIdent,
        prefix_ident: u64,
    ) -> Result<Self, DataStructureError> {
        let shard = Self {
            workchain_id,
            shard_prefix_ident: Uint64::from(prefix_ident),
        };
        shard.validate()?;
        Ok(shard)
    }

    /// Creates a root shard (`L=0`, `prefix_ident = 0x8000_0000_0000_0000`) for the given workchain.
    pub fn root(workchain_id: WorkchainIdent) -> Self {
        Self {
            workchain_id,
            shard_prefix_ident: Uint64::from(Self::ROOT_PREFIX),
        }
    }

    /// Constructs a ShardIdent from prefix bits (represented as a u64, aligned to MSB) and prefix length L (0 <= L <= 60).
    pub fn from_prefix_bits(
        workchain_id: WorkchainIdent,
        bits: u64,
        prefix_len: u8,
    ) -> Result<Self, DataStructureError> {
        if prefix_len > 60 {
            return Err(DataStructureError::ShardPrefixLengthExceeded { length: prefix_len });
        }

        let marker_bit = 1u64 << (63 - prefix_len);
        let mask = if prefix_len == 0 {
            0
        } else {
            (!0u64) << (64 - prefix_len)
        };

        let prefix_ident = (bits & mask) | marker_bit;
        Ok(Self {
            workchain_id,
            shard_prefix_ident: Uint64::from(prefix_ident),
        })
    }

    /// Returns the prefix length L (0 <= L <= 60) by locating the lowest set bit (the marker bit).
    pub fn prefix_len(&self) -> Result<u8, DataStructureError> {
        let val = self.shard_prefix_ident.0;
        if val == 0 {
            return Err(DataStructureError::InvalidShardMarker);
        }
        let trailing_zeros = val.trailing_zeros() as u8;
        let prefix_len = 63 - trailing_zeros;
        if prefix_len > 60 {
            return Err(DataStructureError::ShardPrefixLengthExceeded { length: prefix_len });
        }
        Ok(prefix_len)
    }

    /// Validates marker bit presence and prefix length L <= 60.
    pub fn validate(&self) -> Result<(), DataStructureError> {
        let _ = self.prefix_len()?;
        Ok(())
    }

    /// Checks whether a given AccountId belongs to this shard prefix.
    pub fn contains_account(&self, account_id: &AccountId) -> Result<bool, DataStructureError> {
        let len = self.prefix_len()?;
        if len == 0 {
            return Ok(true);
        }

        let acct_bytes = account_id.to_bytes();
        let acct_msb_u64 = u64::from_be_bytes(acct_bytes[0..8].try_into().unwrap());

        let mask = (!0u64) << (64 - len);
        let shard_bits = self.shard_prefix_ident.0 & mask;
        let acct_bits = acct_msb_u64 & mask;

        Ok(shard_bits == acct_bits)
    }

    /// Validates that an AccountId belongs to this shard prefix, returning `AddressShardMismatch` on mismatch.
    pub fn validate_account_id(&self, account_id: &AccountId) -> Result<(), DataStructureError> {
        if self.contains_account(account_id)? {
            Ok(())
        } else {
            Err(DataStructureError::AddressShardMismatch)
        }
    }

    /// Validates that a FullAddress belongs to this shard (workchain ID match and account ID prefix match),
    /// returning `AddressShardMismatch` on mismatch.
    pub fn validate_full_address(&self, address: &FullAddress) -> Result<(), DataStructureError> {
        if self.workchain_id != address.workchain_id {
            return Err(DataStructureError::AddressShardMismatch);
        }
        self.validate_account_id(&address.account_id)
    }

    /// Serializes ShardIdent to 12 bytes.
    pub fn to_bytes(&self) -> [u8; 12] {
        let mut buf = [0u8; 12];
        buf[0..4].copy_from_slice(&self.workchain_id.to_bytes());
        buf[4..12].copy_from_slice(&self.shard_prefix_ident.encode());
        buf
    }

    /// Deserializes ShardIdent from 12 bytes and validates marker bit and length.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DataStructureError> {
        if bytes.len() < 12 {
            return Err(DataStructureError::TruncatedInput {
                expected: 12,
                got: bytes.len(),
            });
        }
        if bytes.len() > 12 {
            return Err(DataStructureError::TrailingBytes {
                remaining: bytes.len() - 12,
            });
        }

        let mut wc_bytes = [0u8; 4];
        wc_bytes.copy_from_slice(&bytes[0..4]);
        let workchain_id = WorkchainIdent::from_bytes(wc_bytes);

        let shard_prefix_ident = Uint64::decode_exact(&bytes[4..12])?;

        let shard = Self {
            workchain_id,
            shard_prefix_ident,
        };
        shard.validate()?;
        Ok(shard)
    }
}
