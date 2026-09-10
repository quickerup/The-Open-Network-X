//! Ed25519 keys and domain-separated signing/verification.
//!
//! Spec: `docs/specification/protocol-primitives.md`, §3.4, §4.4, §4.5;
//! `docs/decisions/ADR-0002-protocol-primitives-and-serialization.md`.
//! Public keys are 32-byte RFC 8032 encodings and signatures are 64-byte
//! `R || s` encodings. As with hashing, there is no API to sign or verify
//! without a [`DomainTag`]: every signature commits to the protocol
//! context it was produced for.

use crate::error::PrimitiveError;
use crate::hash::DomainTag;
use ed25519_dalek::Signer;

/// A 32-byte Ed25519 public key (RFC 8032 §5.1.5 encoding).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicKey(ed25519_dalek::VerifyingKey);

/// A 32-byte Ed25519 secret seed and its derived signing key.
#[derive(Clone)]
pub struct SecretKey(ed25519_dalek::SigningKey);

/// A 64-byte Ed25519 signature (RFC 8032 §5.1.6 encoding, `R || s`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature(ed25519_dalek::Signature);

impl PublicKey {
    pub const BYTE_LEN: usize = 32;

    /// Decodes a public key, rejecting truncated/over-length input and
    /// non-canonical point encodings (§5, rules 1, 2, 3).
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
        let key = ed25519_dalek::VerifyingKey::from_bytes(&buf)
            .map_err(|_| PrimitiveError::NonCanonicalEncoding)?;
        Ok(PublicKey(key))
    }

    pub fn encode(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Verifies `signature` over `message` in the protocol context named
    /// by `tag`. The domain tag is always prepended before verification
    /// (§4.5); a signature produced for a different tag will not verify
    /// even over an identical message.
    pub fn verify(
        &self,
        tag: &DomainTag,
        message: &[u8],
        signature: &Signature,
    ) -> Result<(), PrimitiveError> {
        let domain_message = domain_separated_message(tag, message);
        self.0
            .verify_strict(&domain_message, &signature.0)
            .map_err(|_| PrimitiveError::SignatureVerificationFailed)
    }
}

impl SecretKey {
    pub const SEED_LEN: usize = 32;

    /// Derives a signing key from a 32-byte secret seed (RFC 8032 §5.1.5).
    pub fn from_seed(bytes: &[u8]) -> Result<Self, PrimitiveError> {
        if bytes.len() < Self::SEED_LEN {
            return Err(PrimitiveError::Truncated {
                expected: Self::SEED_LEN,
                actual: bytes.len(),
            });
        }
        if bytes.len() > Self::SEED_LEN {
            return Err(PrimitiveError::TrailingBytes {
                consumed: Self::SEED_LEN,
                actual: bytes.len(),
            });
        }
        let mut buf = [0u8; 32];
        buf.copy_from_slice(bytes);
        Ok(SecretKey(ed25519_dalek::SigningKey::from_bytes(&buf)))
    }

    pub fn public_key(&self) -> PublicKey {
        PublicKey(self.0.verifying_key())
    }

    pub fn to_scalar_bytes(&self) -> [u8; 32] {
        self.0.to_scalar_bytes()
    }

    /// Signs `message` in the protocol context named by `tag`. The domain
    /// tag is always prepended before signing (§4.5).
    pub fn sign(&self, tag: &DomainTag, message: &[u8]) -> Signature {
        let domain_message = domain_separated_message(tag, message);
        Signature(self.0.sign(&domain_message))
    }
}

impl Signature {
    pub const BYTE_LEN: usize = 64;

    /// Decodes a signature, rejecting truncated/over-length input (§5,
    /// rules 1, 2). Non-canonical scalar components are rejected at
    /// verification time by [`PublicKey::verify`], per RFC 8032.
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
        let mut buf = [0u8; 64];
        buf.copy_from_slice(bytes);
        Ok(Signature(ed25519_dalek::Signature::from_bytes(&buf)))
    }

    pub fn encode(&self) -> [u8; 64] {
        self.0.to_bytes()
    }
}

fn domain_separated_message(tag: &DomainTag, message: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + message.len());
    out.extend_from_slice(tag.as_bytes());
    out.extend_from_slice(message);
    out
}
