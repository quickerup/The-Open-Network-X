//! Canonical byte string encoding.
//!
//! Spec: `docs/specification/protocol-primitives.md`, §3.2 and §4.2.
//! A byte string is a bitstring whose bit length is a multiple of 8; ONX
//! only needs the byte-aligned case at this layer. Fixed-length byte
//! strings are serialized directly; bounded variable-length byte strings
//! carry a big-endian length prefix (`uint16` or `uint32`, chosen by the
//! containing structure) followed immediately by the payload.

use crate::error::PrimitiveError;

/// A fixed-length byte string of exactly `N` bytes, serialized directly
/// with no length prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FixedBytes<const N: usize>(pub [u8; N]);

impl<const N: usize> FixedBytes<N> {
    pub const BYTE_LEN: usize = N;

    pub fn encode(&self) -> [u8; N] {
        self.0
    }

    /// Decodes from a slice that must contain exactly `N` bytes.
    pub fn decode_exact(bytes: &[u8]) -> Result<Self, PrimitiveError> {
        if bytes.len() < N {
            return Err(PrimitiveError::Truncated {
                expected: N,
                actual: bytes.len(),
            });
        }
        if bytes.len() > N {
            return Err(PrimitiveError::TrailingBytes {
                consumed: N,
                actual: bytes.len(),
            });
        }
        let mut buf = [0u8; N];
        buf.copy_from_slice(bytes);
        Ok(Self(buf))
    }

    /// Reads exactly `N` bytes from the front of `cursor` and advances it.
    pub fn read(cursor: &mut &[u8]) -> Result<Self, PrimitiveError> {
        if cursor.len() < N {
            return Err(PrimitiveError::Truncated {
                expected: N,
                actual: cursor.len(),
            });
        }
        let (head, tail) = cursor.split_at(N);
        let mut buf = [0u8; N];
        buf.copy_from_slice(head);
        *cursor = tail;
        Ok(Self(buf))
    }
}

macro_rules! define_bounded_bytes {
    ($name:ident, $prefix_native:ty, $prefix_bytes:expr, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name {
            payload: Vec<u8>,
            max_len: usize,
        }

        impl $name {
            /// Wraps `payload` for encoding, bounding it by `max_len`.
            /// `max_len` must not exceed the length prefix's numeric range.
            pub fn new(payload: Vec<u8>, max_len: usize) -> Result<Self, PrimitiveError> {
                if payload.len() > max_len {
                    return Err(PrimitiveError::LengthOutOfRange {
                        declared: payload.len(),
                        max: max_len,
                    });
                }
                Ok(Self { payload, max_len })
            }

            pub fn as_bytes(&self) -> &[u8] {
                &self.payload
            }

            pub fn into_bytes(self) -> Vec<u8> {
                self.payload
            }

            /// Serializes as a big-endian length prefix followed by the
            /// payload.
            pub fn encode(&self) -> Vec<u8> {
                let mut out = Vec::with_capacity($prefix_bytes + self.payload.len());
                out.extend_from_slice(&(self.payload.len() as $prefix_native).to_be_bytes());
                out.extend_from_slice(&self.payload);
                out
            }

            /// Reads a length-prefixed byte string from the front of
            /// `cursor`, rejecting a declared length that exceeds
            /// `max_len` (§5, rule 5) or that runs past the available
            /// bytes (§5, rule 1).
            pub fn read(cursor: &mut &[u8], max_len: usize) -> Result<Self, PrimitiveError> {
                if cursor.len() < $prefix_bytes {
                    return Err(PrimitiveError::Truncated {
                        expected: $prefix_bytes,
                        actual: cursor.len(),
                    });
                }
                let (len_bytes, rest) = cursor.split_at($prefix_bytes);
                let mut len_buf = [0u8; $prefix_bytes];
                len_buf.copy_from_slice(len_bytes);
                let declared = <$prefix_native>::from_be_bytes(len_buf) as usize;
                if declared > max_len {
                    return Err(PrimitiveError::LengthOutOfRange {
                        declared,
                        max: max_len,
                    });
                }
                if rest.len() < declared {
                    return Err(PrimitiveError::Truncated {
                        expected: declared,
                        actual: rest.len(),
                    });
                }
                let (payload, tail) = rest.split_at(declared);
                *cursor = tail;
                Ok(Self {
                    payload: payload.to_vec(),
                    max_len,
                })
            }

            /// Decodes from a slice that must be consumed exactly: no
            /// trailing bytes after the declared payload (§5, rule 2).
            pub fn decode_exact(bytes: &[u8], max_len: usize) -> Result<Self, PrimitiveError> {
                let mut cursor = bytes;
                let value = Self::read(&mut cursor, max_len)?;
                if !cursor.is_empty() {
                    return Err(PrimitiveError::TrailingBytes {
                        consumed: bytes.len() - cursor.len(),
                        actual: bytes.len(),
                    });
                }
                Ok(value)
            }
        }
    };
}

define_bounded_bytes!(
    BoundedBytesU16,
    u16,
    2,
    "A variable-length byte string with a `uint16` big-endian length prefix."
);
define_bounded_bytes!(
    BoundedBytesU32,
    u32,
    4,
    "A variable-length byte string with a `uint32` big-endian length prefix."
);
