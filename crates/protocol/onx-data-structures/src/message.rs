//! Core message structures per docs/specification/data-structures.md §4.3.

use crate::address::FullAddress;
use crate::error::DataStructureError;
use crate::shard::ShardIdent;
use onx_primitives::{domain_hash, DomainTag, Uint128, Uint256, Uint32, Uint64};

/// Message type discriminant byte tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum MessageType {
    /// Internal message between accounts (0x01).
    Internal = 0x01,
    /// External inbound message from outside network (0x02).
    ExternalInbound = 0x02,
    /// External outbound message to outside network (0x03).
    ExternalOutbound = 0x03,
}

impl MessageType {
    /// Converts a byte tag to MessageType or returns an error.
    pub fn from_tag(tag: u8) -> Result<Self, DataStructureError> {
        match tag {
            0x01 => Ok(Self::Internal),
            0x02 => Ok(Self::ExternalInbound),
            0x03 => Ok(Self::ExternalOutbound),
            _ => Err(DataStructureError::InvalidMessageType { tag }),
        }
    }

    /// Returns byte tag value.
    pub const fn tag(&self) -> u8 {
        *self as u8
    }
}

/// Core Message structure (`Message`).
///
/// Binary layout has a 133-byte fixed portion plus 20 bytes per extra currency:
/// `msg_type`, source and destination addresses, `amount_nanos`, a uint32
/// extra-currency count, `(currency_id, value)` pairs, `created_lt`, and body hash.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Message {
    pub msg_type: MessageType,
    pub src_address: FullAddress,
    pub dest_address: FullAddress,
    pub amount_nanos: Uint128,
    /// Canonically encoded `(currency_id, value)` pairs.
    ///
    /// TODO(ONX-ARCH-004): Apply the Transactions specification's semantic
    /// rules for validation, defaults, and balance-check interaction here.
    pub extra_currencies: Vec<(Uint32, Uint128)>,
    pub created_lt: Uint64,
    pub body_cell_hash: Uint256,
}

impl Message {
    /// Domain tag for the canonical message representation hash.
    pub const DOMAIN_TAG: DomainTag = DomainTag::from_ascii("ONX_MSG_HASH_V1");

    /// Fixed byte size excluding the count-prefixed extra-currency pairs.
    pub const FIXED_BYTE_LENGTH: usize = 1 + 36 + 36 + 16 + 4 + 8 + 32;

    /// Serializes Message using the count-prefixed layout in the Transactions specification.
    pub fn to_bytes(&self) -> Vec<u8> {
        let extra_len = self
            .extra_currencies
            .len()
            .checked_mul(20)
            .expect("extra currency length overflow");
        let mut buf = Vec::with_capacity(Self::FIXED_BYTE_LENGTH + extra_len);
        buf.push(self.msg_type.tag());
        buf.extend_from_slice(&self.src_address.to_bytes());
        buf.extend_from_slice(&self.dest_address.to_bytes());
        buf.extend_from_slice(&self.amount_nanos.encode());
        buf.extend_from_slice(&Uint32(self.extra_currencies.len() as u32).encode());
        for (currency_id, value) in &self.extra_currencies {
            buf.extend_from_slice(&currency_id.encode());
            buf.extend_from_slice(&value.encode());
        }
        buf.extend_from_slice(&self.created_lt.encode());
        buf.extend_from_slice(&self.body_cell_hash.encode());
        buf
    }

    /// Computes the domain-separated canonical representation hash.
    pub fn message_hash(&self) -> [u8; 32] {
        domain_hash(&Self::DOMAIN_TAG, &self.to_bytes())
    }

    /// Deserializes Message from exactly one canonical binary representation.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DataStructureError> {
        if bytes.len() < Self::FIXED_BYTE_LENGTH {
            return Err(DataStructureError::TruncatedInput {
                expected: Self::FIXED_BYTE_LENGTH,
                got: bytes.len(),
            });
        }
        let msg_type = MessageType::from_tag(bytes[0])?;
        let src_address = FullAddress::from_bytes(&bytes[1..37])?;
        let dest_address = FullAddress::from_bytes(&bytes[37..73])?;
        let amount_nanos = Uint128::decode_exact(&bytes[73..89])?;
        let count = Uint32::decode_exact(&bytes[89..93])?.0 as usize;
        let pairs_len = count
            .checked_mul(20)
            .ok_or(DataStructureError::TruncatedInput {
                expected: usize::MAX,
                got: bytes.len(),
            })?;
        let expected = Self::FIXED_BYTE_LENGTH.checked_add(pairs_len).ok_or(
            DataStructureError::TruncatedInput {
                expected: usize::MAX,
                got: bytes.len(),
            },
        )?;
        if bytes.len() < expected {
            return Err(DataStructureError::TruncatedInput {
                expected,
                got: bytes.len(),
            });
        }
        if bytes.len() > expected {
            return Err(DataStructureError::TrailingBytes {
                remaining: bytes.len() - expected,
            });
        }
        let mut extra_currencies = Vec::with_capacity(count);
        let mut offset = 93;
        for _ in 0..count {
            let currency_id = Uint32::decode_exact(&bytes[offset..offset + 4])?;
            let value = Uint128::decode_exact(&bytes[offset + 4..offset + 20])?;
            extra_currencies.push((currency_id, value));
            offset += 20;
        }
        let created_lt = Uint64::decode_exact(&bytes[offset..offset + 8])?;
        let body_cell_hash = Uint256::decode_exact(&bytes[offset + 8..offset + 40])?;
        Ok(Self {
            msg_type,
            src_address,
            dest_address,
            amount_nanos,
            extra_currencies,
            created_lt,
            body_cell_hash,
        })
    }

    /// Validates that the message source address matches the given shard.
    pub fn validate_source_shard(&self, shard: &ShardIdent) -> Result<(), DataStructureError> {
        self.src_address.validate_against_shard(shard)
    }

    /// Validates that the message destination address matches the given shard.
    pub fn validate_dest_shard(&self, shard: &ShardIdent) -> Result<(), DataStructureError> {
        self.dest_address.validate_against_shard(shard)
    }

    /// Validates message address alignment with `shard` according to `msg_type`:
    /// - `MessageType::Internal`: requires at least one endpoint (`src_address` or `dest_address`) to match `shard`.
    /// - `MessageType::ExternalInbound`: requires `dest_address` to match `shard`.
    /// - `MessageType::ExternalOutbound`: requires `src_address` to match `shard`.
    pub fn validate_against_shard(&self, shard: &ShardIdent) -> Result<(), DataStructureError> {
        match self.msg_type {
            MessageType::Internal => {
                let src_ok = self.validate_source_shard(shard).is_ok();
                let dest_ok = self.validate_dest_shard(shard).is_ok();
                if src_ok || dest_ok {
                    Ok(())
                } else {
                    Err(DataStructureError::AddressShardMismatch)
                }
            }
            MessageType::ExternalInbound => self.validate_dest_shard(shard),
            MessageType::ExternalOutbound => self.validate_source_shard(shard),
        }
    }
}
