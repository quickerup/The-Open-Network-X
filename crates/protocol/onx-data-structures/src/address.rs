//! Workchain, account, and address identifiers per docs/specification/data-structures.md §4.1.

use crate::error::DataStructureError;
use crate::shard::ShardIdent;
use onx_primitives::{Int32, Uint256};

/// Workchain identifier (32-bit signed big-endian integer).
///
/// Masterchain = -1 (`0xFFFFFFFF`).
/// Workchain 0 (Basic Workchain) = 0 (`0x00000000`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkchainIdent(pub Int32);

impl WorkchainIdent {
    /// The Masterchain workchain ID (-1).
    pub const MASTERCHAIN: Self = Self(Int32(-1));

    /// Workchain 0 (Basic Workchain).
    pub const BASIC: Self = Self(Int32(0));

    /// Constructs a workchain identifier from a signed 32-bit integer.
    pub const fn new(id: i32) -> Self {
        Self(Int32(id))
    }

    /// Returns true if this is the Masterchain (-1).
    pub const fn is_masterchain(&self) -> bool {
        self.0 .0 == -1
    }

    /// Serializes workchain identifier to 4 big-endian bytes.
    pub fn to_bytes(&self) -> [u8; 4] {
        self.0.encode()
    }

    /// Deserializes workchain identifier from 4 big-endian bytes.
    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        Self(Int32(i32::from_be_bytes(bytes)))
    }
}

/// Account identifier (256-bit unsigned integer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccountId(pub Uint256);

impl AccountId {
    /// Total byte length of AccountId (32 bytes).
    pub const BYTE_LENGTH: usize = Uint256::BYTE_LEN;

    /// Constructs an AccountId from a 32-byte array.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Uint256(bytes))
    }

    /// Returns the raw 32-byte representation.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.encode()
    }
}

/// Full account address (`workchain_id` + `account_id`, 36 bytes total).
///
/// Layout:
/// - `workchain_id`: 4 bytes (int32 big-endian)
/// - `account_id`: 32 bytes (uint256 big-endian)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FullAddress {
    pub workchain_id: WorkchainIdent,
    pub account_id: AccountId,
}

impl FullAddress {
    /// Exact byte size of a serialized FullAddress (36 bytes).
    pub const BYTE_LENGTH: usize = 36;

    /// Creates a new FullAddress.
    pub const fn new(workchain_id: WorkchainIdent, account_id: AccountId) -> Self {
        Self {
            workchain_id,
            account_id,
        }
    }

    /// Serializes FullAddress to 36 bytes.
    pub fn to_bytes(&self) -> [u8; 36] {
        let mut buf = [0u8; 36];
        buf[0..4].copy_from_slice(&self.workchain_id.to_bytes());
        buf[4..36].copy_from_slice(&self.account_id.to_bytes());
        buf
    }

    /// Deserializes FullAddress from exactly 36 bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DataStructureError> {
        if bytes.len() < 36 {
            return Err(DataStructureError::TruncatedInput {
                expected: 36,
                got: bytes.len(),
            });
        }
        if bytes.len() > 36 {
            return Err(DataStructureError::TrailingBytes {
                remaining: bytes.len() - 36,
            });
        }

        let mut wc_bytes = [0u8; 4];
        wc_bytes.copy_from_slice(&bytes[0..4]);
        let workchain_id = WorkchainIdent::from_bytes(wc_bytes);

        let mut acct_bytes = [0u8; 32];
        acct_bytes.copy_from_slice(&bytes[4..36]);
        let account_id = AccountId::from_bytes(acct_bytes);

        Ok(Self {
            workchain_id,
            account_id,
        })
    }

    /// Validates that this full address matches the given shard, returning `AddressShardMismatch` on mismatch.
    pub fn validate_against_shard(&self, shard: &ShardIdent) -> Result<(), DataStructureError> {
        shard.validate_full_address(self)
    }
}
