//! Canonical fixed-width integer encoding.
//!
//! Spec: `docs/specification/protocol-primitives.md`, §3.1 and §4.1.
//! Every ONX consensus integer is a fixed-width, big-endian value occupying
//! exactly `N/8` bytes. Decoding rejects both truncated input and trailing
//! bytes (§5, rules 1 and 2) so that no two byte strings can decode to the
//! same value.

use crate::error::PrimitiveError;

macro_rules! define_fixed_integer {
    ($name:ident, $native:ty, $bytes:expr, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub $native);

        impl $name {
            /// Exact wire length in bytes for this type.
            pub const BYTE_LEN: usize = $bytes;
            pub const MIN: Self = Self(<$native>::MIN);
            pub const MAX: Self = Self(<$native>::MAX);
            pub const ZERO: Self = Self(0 as $native);

            /// Serializes to the canonical big-endian byte representation.
            pub fn encode(&self) -> [u8; $bytes] {
                self.0.to_be_bytes()
            }

            /// Decodes from a byte slice that must contain exactly
            /// [`Self::BYTE_LEN`] bytes. Rejects truncated and over-length
            /// input per §5, rules 1 and 2.
            pub fn decode_exact(bytes: &[u8]) -> Result<Self, PrimitiveError> {
                if bytes.len() < Self::BYTE_LEN {
                    return Err(PrimitiveError::Truncated {
                        expected: Self::BYTE_LEN,
                        actual: bytes.len(),
                    });
                }
                if bytes.len() > Self::BYTE_LEN {
                    return Err(PrimitiveError::TrailingBytes {
                        consumed: Self::BYTE_LEN,
                        actual: bytes.len(),
                    });
                }
                let mut buf = [0u8; $bytes];
                buf.copy_from_slice(bytes);
                Ok(Self(<$native>::from_be_bytes(buf)))
            }

            /// Reads exactly [`Self::BYTE_LEN`] bytes from the front of
            /// `cursor` and advances it, leaving any remaining bytes for the
            /// caller to interpret. Used when this integer is one field
            /// inside a larger, already length-checked structure.
            pub fn read(cursor: &mut &[u8]) -> Result<Self, PrimitiveError> {
                if cursor.len() < Self::BYTE_LEN {
                    return Err(PrimitiveError::Truncated {
                        expected: Self::BYTE_LEN,
                        actual: cursor.len(),
                    });
                }
                let (head, tail) = cursor.split_at(Self::BYTE_LEN);
                let mut buf = [0u8; $bytes];
                buf.copy_from_slice(head);
                *cursor = tail;
                Ok(Self(<$native>::from_be_bytes(buf)))
            }
        }

        impl From<$native> for $name {
            fn from(value: $native) -> Self {
                Self(value)
            }
        }

        impl From<$name> for $native {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

define_fixed_integer!(Uint8, u8, 1, "Canonical big-endian `uint8`.");
define_fixed_integer!(Uint16, u16, 2, "Canonical big-endian `uint16`.");
define_fixed_integer!(Uint32, u32, 4, "Canonical big-endian `uint32`.");
define_fixed_integer!(Uint64, u64, 8, "Canonical big-endian `uint64`.");
define_fixed_integer!(Uint128, u128, 16, "Canonical big-endian `uint128`.");

define_fixed_integer!(Int8, i8, 1, "Canonical big-endian two's-complement `int8`.");
define_fixed_integer!(
    Int16,
    i16,
    2,
    "Canonical big-endian two's-complement `int16`."
);
define_fixed_integer!(
    Int32,
    i32,
    4,
    "Canonical big-endian two's-complement `int32`."
);
define_fixed_integer!(
    Int64,
    i64,
    8,
    "Canonical big-endian two's-complement `int64`."
);
define_fixed_integer!(
    Int128,
    i128,
    16,
    "Canonical big-endian two's-complement `int128`."
);

/// Canonical big-endian `uint256`.
///
/// Rust has no native 256-bit integer type, so `Uint256` stores the 32
/// canonical big-endian bytes directly. Arithmetic is intentionally not
/// implemented here: the protocol-primitives specification only defines
/// serialization for this width, not arithmetic semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uint256(pub [u8; 32]);

/// Canonical big-endian two's-complement `int256`.
///
/// See [`Uint256`] for why this stores raw bytes rather than a native
/// integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Int256(pub [u8; 32]);

macro_rules! define_wide_integer {
    ($name:ident, $min:expr, $max:expr) => {
        impl $name {
            pub const BYTE_LEN: usize = 32;
            pub const ZERO: Self = Self([0u8; 32]);
            pub const MIN: Self = Self($min);
            pub const MAX: Self = Self($max);

            pub fn encode(&self) -> [u8; 32] {
                self.0
            }

            pub fn decode_exact(bytes: &[u8]) -> Result<Self, PrimitiveError> {
                if bytes.len() < Self::BYTE_LEN {
                    return Err(PrimitiveError::Truncated {
                        expected: Self::BYTE_LEN,
                        actual: bytes.len(),
                    });
                }
                if bytes.len() > Self::BYTE_LEN {
                    return Err(PrimitiveError::TrailingBytes {
                        consumed: Self::BYTE_LEN,
                        actual: bytes.len(),
                    });
                }
                let mut buf = [0u8; 32];
                buf.copy_from_slice(bytes);
                Ok(Self(buf))
            }

            pub fn read(cursor: &mut &[u8]) -> Result<Self, PrimitiveError> {
                if cursor.len() < Self::BYTE_LEN {
                    return Err(PrimitiveError::Truncated {
                        expected: Self::BYTE_LEN,
                        actual: cursor.len(),
                    });
                }
                let (head, tail) = cursor.split_at(Self::BYTE_LEN);
                let mut buf = [0u8; 32];
                buf.copy_from_slice(head);
                *cursor = tail;
                Ok(Self(buf))
            }
        }
    };
}

define_wide_integer!(Uint256, [0u8; 32], [0xffu8; 32]);
define_wide_integer!(
    Int256,
    {
        let mut m = [0u8; 32];
        m[0] = 0x80;
        m
    },
    {
        let mut m = [0xffu8; 32];
        m[0] = 0x7f;
        m
    }
);
