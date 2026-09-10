//! Error types for data structures serialization and validation.

use onx_primitives::PrimitiveError;

/// Errors that can occur during data structure encoding, decoding, or validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataStructureError {
    /// Wrapper for underlying primitive serialization errors.
    Primitive(PrimitiveError),

    /// Invalid shard marker bit (missing required marker bit '1').
    InvalidShardMarker,

    /// Shard prefix length exceeds max limit (L > 60).
    ShardPrefixLengthExceeded { length: u8 },

    /// Account address does not match active shard prefix.
    AddressShardMismatch,

    /// Unrecognized or invalid message type tag.
    InvalidMessageType { tag: u8 },

    /// Header magic constructor mismatch. Expected 0x1F2E3D4C.
    HeaderMagicMismatch { magic: u32 },

    /// Merge parent reference inconsistency (prev_ref_hash_2 set without MERGE_RESULT or zero with MERGE_RESULT).
    MergeParentReferenceInconsistency,

    /// Truncated input bytes.
    TruncatedInput { expected: usize, got: usize },

    /// Unexpected trailing bytes.
    TrailingBytes { remaining: usize },
}

impl From<PrimitiveError> for DataStructureError {
    fn from(err: PrimitiveError) -> Self {
        DataStructureError::Primitive(err)
    }
}

impl std::fmt::Display for DataStructureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataStructureError::Primitive(e) => write!(f, "Primitive error: {}", e),
            DataStructureError::InvalidShardMarker => {
                write!(f, "Invalid shard identifier: missing binary '1' marker bit")
            }
            DataStructureError::ShardPrefixLengthExceeded { length } => {
                write!(
                    f,
                    "Shard prefix length {} exceeds maximum of 60 bits",
                    length
                )
            }
            DataStructureError::AddressShardMismatch => {
                write!(f, "Account address does not match active shard prefix")
            }
            DataStructureError::InvalidMessageType { tag } => {
                write!(f, "Invalid message type tag: 0x{:02x}", tag)
            }
            DataStructureError::HeaderMagicMismatch { magic } => {
                write!(
                    f,
                    "Header magic constructor mismatch: expected 0x1F2E3D4C, got 0x{:08x}",
                    magic
                )
            }
            DataStructureError::MergeParentReferenceInconsistency => {
                write!(
                    f,
                    "Merge parent reference inconsistency between prev_ref_hash_2 and MERGE_RESULT flag"
                )
            }
            DataStructureError::TruncatedInput { expected, got } => {
                write!(
                    f,
                    "Truncated input: expected at least {} bytes, got {}",
                    expected, got
                )
            }
            DataStructureError::TrailingBytes { remaining } => {
                write!(f, "Unexpected trailing bytes: {} bytes left", remaining)
            }
        }
    }
}

impl std::error::Error for DataStructureError {}
