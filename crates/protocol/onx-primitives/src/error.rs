//! Rejection reasons for malformed protocol primitives.
//!
//! Spec: `docs/specification/protocol-primitives.md`, §5 (Malformed-input
//! behavior). Every variant here corresponds to one of the five mandatory
//! rejection rules; there is deliberately no "best effort" recovery path.

use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveError {
    /// Fewer bytes were supplied than the fixed-width type, key, or
    /// signature requires.
    Truncated { expected: usize, actual: usize },
    /// Extra, unparsed bytes trailed a fixed-size buffer or a
    /// length-prefixed field.
    TrailingBytes { consumed: usize, actual: usize },
    /// A declared length-prefix value exceeds the configured bound for the
    /// container.
    LengthOutOfRange { declared: usize, max: usize },
    /// An Ed25519 public key or signature used a non-canonical point or
    /// scalar encoding (RFC 8032).
    NonCanonicalEncoding,
    /// A domain separation tag was missing, the wrong length, or did not
    /// match the expected protocol context.
    InvalidDomainTag,
    /// Signature verification failed for a reason other than encoding.
    SignatureVerificationFailed,
}

impl fmt::Display for PrimitiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrimitiveError::Truncated { expected, actual } => write!(
                f,
                "truncated input: expected {expected} bytes, got {actual}"
            ),
            PrimitiveError::TrailingBytes { consumed, actual } => write!(
                f,
                "trailing bytes: consumed {consumed} of {actual} available bytes"
            ),
            PrimitiveError::LengthOutOfRange { declared, max } => {
                write!(f, "length prefix {declared} exceeds maximum {max}")
            }
            PrimitiveError::NonCanonicalEncoding => {
                write!(f, "non-canonical point or scalar encoding")
            }
            PrimitiveError::InvalidDomainTag => write!(f, "invalid domain separation tag"),
            PrimitiveError::SignatureVerificationFailed => {
                write!(f, "signature verification failed")
            }
        }
    }
}

impl std::error::Error for PrimitiveError {}
