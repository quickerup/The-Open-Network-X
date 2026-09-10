pub mod adnl_transport;
pub mod dht_daemon;
pub mod rldp;

pub use adnl_transport::{
    apply_aes256_ctr, derive_symmetric_key_iv, AdnlTransportNode, FastPacket, FullPacket,
    PeerSession,
};
pub use dht_daemon::{DhtContact, DhtDaemon, DhtRpc, DhtRpcResponse, DhtTransport};
pub use rldp::{RldpConfig, RldpSender};
use onx_primitives::{
    domain_hash,
    hash::{DomainTag, VALIDATOR_SIGN_V1},
    PublicKey, Signature, Uint256, Uint64,
};
use std::fmt;

/// Errors in networking operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkError {
    TruncatedPacket,
    AbstractAddressMismatch,
    DecryptionFailed,
    InvalidSignature,
    ExpiredTimestamp,
    ZeroChannelAbuse,
    MalformedRldpFrame,
    PayloadDigestMismatch,
    RldpTimeout,
    DhtRecordExpired,
    DhtMalformedRecord,
    DhtUnknownRecordKind,
    DhtRpcFailed,
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedPacket => {
                write!(f, "Packet size is smaller than required header length")
            }
            Self::AbstractAddressMismatch => {
                write!(f, "Abstract address preimage SHA-256 hash mismatch")
            }
            Self::DecryptionFailed => write!(f, "Payload decryption or integrity check failed"),
            Self::InvalidSignature => write!(f, "Invalid packet digital signature"),
            Self::ExpiredTimestamp => write!(f, "Packet timestamp is outside valid window"),
            Self::ZeroChannelAbuse => {
                write!(f, "Non-bootstrap payload transmitted over zero-channel")
            }
            Self::MalformedRldpFrame => write!(f, "Malformed RLDP chunk or message frame"),
            Self::PayloadDigestMismatch => write!(f, "RLDP payload SHA-256 digest mismatch"),
            Self::RldpTimeout => write!(f, "RLDP transfer timed out before acknowledgement"),
            Self::DhtRecordExpired => write!(f, "DHT record has expired"),
            Self::DhtMalformedRecord => write!(f, "Malformed DHT record"),
            Self::DhtUnknownRecordKind => write!(f, "Unknown DHT record kind"),
            Self::DhtRpcFailed => write!(f, "DHT RPC failed"),
        }
    }
}

impl std::error::Error for NetworkError {}

/// ADNL KeyDescription structure per docs/specification/networking-adnl.md §4.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyDescription {
    pub constructor_tag: u32, // 0x48a30121 for Ed25519
    pub public_key: PublicKey,
}

impl KeyDescription {
    pub const ED25519_TAG: u32 = 0x48a30121;
    pub const DOMAIN_TAG: DomainTag = DomainTag::from_ascii("ONX_ADNL_KEY_DESC_V1");

    pub fn new_ed25519(public_key: PublicKey) -> Self {
        Self {
            constructor_tag: Self::ED25519_TAG,
            public_key,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(36);
        buf.extend_from_slice(&self.constructor_tag.to_be_bytes());
        buf.extend_from_slice(&self.public_key.encode());
        buf
    }

    pub fn compute_abstract_address(&self) -> Uint256 {
        Uint256(domain_hash(&Self::DOMAIN_TAG, &self.to_bytes()))
    }
}

/// Point-to-point ADNL channel ID derivation per docs/specification/networking-adnl.md §3.3.
pub fn derive_channel_id(
    shared_secret: &[u8; 32],
    sender_address: Uint256,
    recipient_address: Uint256,
) -> Uint256 {
    let tag = DomainTag::from_ascii("ONX_ADNL_CHANNEL_V1");
    let mut buf = Vec::with_capacity(96);
    buf.extend_from_slice(shared_secret);
    buf.extend_from_slice(&sender_address.encode());
    buf.extend_from_slice(&recipient_address.encode());
    Uint256(domain_hash(&tag, &buf))
}

/// Kademlia XOR distance metric per docs/specification/networking-dht.md §3.
pub fn xor_distance(key1: Uint256, key2: Uint256) -> Uint256 {
    let mut res = [0u8; 32];
    for (i, byte) in res.iter_mut().enumerate() {
        *byte = key1.0[i] ^ key2.0[i];
    }
    Uint256(res)
}

/// DHT Record structure per docs/specification/networking-dht.md §4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DhtRecord {
    pub key: Uint256,
    pub value: Vec<u8>,
    pub expiry: Uint64,
    pub owner_address: Uint256,
    pub signature: Signature,
}

impl DhtRecord {
    pub const DOMAIN_TAG: DomainTag = DomainTag::from_ascii("ONX_DHT_RECORD_V1");

    pub fn to_signable_bytes(
        key: Uint256,
        value: &[u8],
        expiry: Uint64,
        owner_address: Uint256,
    ) -> Vec<u8> {
        let mut buf = Vec::with_capacity(72 + value.len());
        buf.extend_from_slice(&key.encode());
        buf.extend_from_slice(&(value.len() as u32).to_be_bytes());
        buf.extend_from_slice(value);
        buf.extend_from_slice(&expiry.encode());
        buf.extend_from_slice(&owner_address.encode());
        buf
    }

    /// Encodes the canonical DHT record wire layout from the DHT specification.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes =
            Self::to_signable_bytes(self.key, &self.value, self.expiry, self.owner_address);
        bytes.extend_from_slice(&self.signature.encode());
        bytes
    }

    /// Decodes one complete record, rejecting truncated and trailing bytes.
    pub fn decode_exact(bytes: &[u8]) -> Result<Self, NetworkError> {
        const FIXED_WITHOUT_VALUE: usize = 32 + 4 + 8 + 32 + 64;
        if bytes.len() < FIXED_WITHOUT_VALUE {
            return Err(NetworkError::DhtMalformedRecord);
        }
        let mut cursor = bytes;
        let take = |cursor: &mut &[u8], count: usize| -> Result<Vec<u8>, NetworkError> {
            if cursor.len() < count {
                return Err(NetworkError::DhtMalformedRecord);
            }
            let (head, tail) = cursor.split_at(count);
            *cursor = tail;
            Ok(head.to_vec())
        };
        let key = Uint256::decode_exact(&take(&mut cursor, 32)?)
            .map_err(|_| NetworkError::DhtMalformedRecord)?;
        let length =
            u32::from_be_bytes(take(&mut cursor, 4)?.try_into().expect("four bytes")) as usize;
        // The fixed tail must remain available and arithmetic must not overflow.
        if cursor.len()
            < length
                .checked_add(8 + 32 + 64)
                .ok_or(NetworkError::DhtMalformedRecord)?
        {
            return Err(NetworkError::DhtMalformedRecord);
        }
        let value = take(&mut cursor, length)?;
        let expiry = Uint64::decode_exact(&take(&mut cursor, 8)?)
            .map_err(|_| NetworkError::DhtMalformedRecord)?;
        let owner_address = Uint256::decode_exact(&take(&mut cursor, 32)?)
            .map_err(|_| NetworkError::DhtMalformedRecord)?;
        let signature = Signature::decode_exact(&take(&mut cursor, 64)?)
            .map_err(|_| NetworkError::DhtMalformedRecord)?;
        if !cursor.is_empty() {
            return Err(NetworkError::DhtMalformedRecord);
        }
        Ok(Self {
            key,
            value,
            expiry,
            owner_address,
            signature,
        })
    }

    pub fn verify_signature(&self, owner_public_key: &PublicKey) -> Result<(), NetworkError> {
        let bytes = Self::to_signable_bytes(self.key, &self.value, self.expiry, self.owner_address);
        if owner_public_key
            .verify(
                &VALIDATOR_SIGN_V1,
                &domain_hash(&Self::DOMAIN_TAG, &bytes),
                &self.signature,
            )
            .is_err()
        {
            Err(NetworkError::InvalidSignature)
        } else {
            Ok(())
        }
    }
}

/// Overlay network announcement per docs/specification/networking-overlay.md §4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayAnnouncement {
    pub overlay_id: Uint256,
    pub candidate_hash: Uint256,
    pub epoch: Uint64,
    pub chunk_count: u32,
    pub reconstruction_threshold: u32,
    pub sender: Uint256,
    pub signature: Signature,
}

/// RLDP Message Chunk per docs/specification/networking-adnl.md §4.3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RldpChunk {
    pub transfer_id: Uint256,
    pub total_size: u32,
    pub chunk_size: u16,
    pub chunk_count: u16,
    pub seq_no: u16,
    pub chunk_data: Vec<u8>,
}
