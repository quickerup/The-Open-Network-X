//! ONX protocol primitives.
//!
//! Implements `docs/specification/protocol-primitives.md`: canonical
//! fixed-width integer encoding, canonical byte strings, domain-separated
//! SHA-256 hashing, and Ed25519 signing/verification. This crate has no
//! knowledge of higher-level protocol objects (accounts, transactions,
//! blocks); it only provides the primitives those specifications are
//! built from.
//!
//! Per `INSTRUCTIONS.md` §11 and §12, cryptographic and serialization
//! choices here are binding for all consensus-critical code and are
//! recorded in
//! `docs/decisions/ADR-0002-protocol-primitives-and-serialization.md`.

pub mod bytes;
pub mod error;
pub mod hash;
pub mod integers;
pub mod signature;

pub use bytes::{BoundedBytesU16, BoundedBytesU32, FixedBytes};
pub use error::PrimitiveError;
pub use hash::{domain_hash, sha256, DomainTag};
pub use integers::{
    Int128, Int16, Int256, Int32, Int64, Int8, Uint128, Uint16, Uint256, Uint32, Uint64, Uint8,
};
pub use signature::{PublicKey, SecretKey, Signature};
