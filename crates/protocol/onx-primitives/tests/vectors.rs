//! Test vectors for `docs/specification/protocol-primitives.md`, §6.

use onx_primitives::{
    domain_hash, sha256, BoundedBytesU16, DomainTag, FixedBytes, Int256, Int32, Int8,
    PrimitiveError, PublicKey, SecretKey, Signature, Uint256, Uint32, Uint64, Uint8,
};

// --- §6.1 Integer serialization: zero, max, min, boundary cases ---

#[test]
fn uint8_boundaries() {
    assert_eq!(Uint8(0).encode(), [0x00]);
    assert_eq!(Uint8(0xff).encode(), [0xff]);
    assert_eq!(Uint8::decode_exact(&[0x2a]).unwrap(), Uint8(0x2a));
}

#[test]
fn uint32_boundaries() {
    assert_eq!(Uint32(0x0102_0304).encode(), [0x01, 0x02, 0x03, 0x04]);
    assert_eq!(Uint32::MIN.encode(), [0x00, 0x00, 0x00, 0x00]);
    assert_eq!(Uint32::MAX.encode(), [0xff, 0xff, 0xff, 0xff]);
    let round_trip = Uint32::decode_exact(&Uint32(0xdead_beef).encode()).unwrap();
    assert_eq!(round_trip, Uint32(0xdead_beef));
}

#[test]
fn uint64_round_trip() {
    for value in [0u64, 1, u64::MAX, 1u64 << 32, 0x0102_0304_0506_0708] {
        let encoded = Uint64(value).encode();
        assert_eq!(encoded.len(), 8);
        assert_eq!(Uint64::decode_exact(&encoded).unwrap(), Uint64(value));
    }
}

#[test]
fn int8_two_complement_boundaries() {
    assert_eq!(Int8(-1).encode(), [0xff]);
    assert_eq!(Int8::MIN.encode(), [0x80]);
    assert_eq!(Int8::MAX.encode(), [0x7f]);
}

#[test]
fn int32_round_trip() {
    for value in [i32::MIN, -1, 0, 1, i32::MAX] {
        let encoded = Int32(value).encode();
        assert_eq!(Int32::decode_exact(&encoded).unwrap(), Int32(value));
    }
}

#[test]
fn uint256_boundaries() {
    assert_eq!(Uint256::ZERO.encode(), [0u8; 32]);
    assert_eq!(Uint256::MAX.encode(), [0xffu8; 32]);

    let mut bytes = [0u8; 32];
    bytes[31] = 0x01;
    assert_eq!(Uint256::decode_exact(&bytes).unwrap(), Uint256(bytes));
}

#[test]
fn int256_boundaries() {
    let mut min = [0u8; 32];
    min[0] = 0x80;
    assert_eq!(Int256::MIN.encode(), min);

    let mut max = [0xffu8; 32];
    max[0] = 0x7f;
    assert_eq!(Int256::MAX.encode(), max);
}

// --- §5, rules 1 and 2: truncated and trailing-byte rejection ---

#[test]
fn rejects_truncated_fixed_integer() {
    let err = Uint32::decode_exact(&[0x01, 0x02, 0x03]).unwrap_err();
    assert_eq!(
        err,
        PrimitiveError::Truncated {
            expected: 4,
            actual: 3
        }
    );
}

#[test]
fn rejects_trailing_bytes_on_fixed_integer() {
    let err = Uint32::decode_exact(&[0x01, 0x02, 0x03, 0x04, 0x05]).unwrap_err();
    assert_eq!(
        err,
        PrimitiveError::TrailingBytes {
            consumed: 4,
            actual: 5
        }
    );
}

#[test]
fn rejects_truncated_uint256() {
    let err = Uint256::decode_exact(&[0u8; 31]).unwrap_err();
    assert_eq!(
        err,
        PrimitiveError::Truncated {
            expected: 32,
            actual: 31
        }
    );
}

#[test]
fn rejects_truncated_fixed_bytes() {
    let err = FixedBytes::<4>::decode_exact(&[0u8; 2]).unwrap_err();
    assert_eq!(
        err,
        PrimitiveError::Truncated {
            expected: 4,
            actual: 2
        }
    );
}

// --- §4.2 / §5 rule 5: bounded variable-length byte strings ---

#[test]
fn bounded_bytes_u16_round_trip() {
    let value = BoundedBytesU16::new(vec![1, 2, 3], 100).unwrap();
    let encoded = value.encode();
    assert_eq!(&encoded[..2], &[0x00, 0x03]); // length prefix
    assert_eq!(&encoded[2..], &[1, 2, 3]);

    let decoded = BoundedBytesU16::decode_exact(&encoded, 100).unwrap();
    assert_eq!(decoded.as_bytes(), &[1, 2, 3]);
}

#[test]
fn bounded_bytes_rejects_length_over_max() {
    let err = BoundedBytesU16::new(vec![0u8; 10], 5).unwrap_err();
    assert_eq!(
        err,
        PrimitiveError::LengthOutOfRange {
            declared: 10,
            max: 5
        }
    );
}

#[test]
fn bounded_bytes_rejects_declared_length_past_available_bytes() {
    // Length prefix claims 10 bytes of payload but only 2 are present.
    let bytes = [0x00, 0x0a, 0xaa, 0xbb];
    let err = BoundedBytesU16::decode_exact(&bytes, 100).unwrap_err();
    assert_eq!(
        err,
        PrimitiveError::Truncated {
            expected: 10,
            actual: 2
        }
    );
}

#[test]
fn bounded_bytes_rejects_trailing_bytes() {
    // Valid length-3 payload followed by one extra unparsed byte.
    let bytes = [0x00, 0x03, 1, 2, 3, 0xff];
    let err = BoundedBytesU16::decode_exact(&bytes, 100).unwrap_err();
    assert_eq!(
        err,
        PrimitiveError::TrailingBytes {
            consumed: 5,
            actual: 6
        }
    );
}

// --- §6.2 SHA-256 vector verification ---

#[test]
fn sha256_nist_vector_empty_string() {
    // FIPS 180-4 test vector: SHA-256("").
    assert_eq!(
        hex::encode(sha256(b"")),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn sha256_nist_vector_abc() {
    // FIPS 180-4 test vector: SHA-256("abc").
    assert_eq!(
        hex::encode(sha256(b"abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn domain_hash_differs_by_tag() {
    let a = domain_hash(&DomainTag::from_ascii("ONX_TX_BODY_V1"), b"same payload");
    let b = domain_hash(&DomainTag::from_ascii("ONX_BLK_HDR_V1"), b"same payload");
    assert_ne!(a, b, "different domain tags must not collide");
}

#[test]
fn domain_hash_is_deterministic() {
    let tag = DomainTag::from_ascii("ONX_TX_BODY_V1");
    assert_eq!(domain_hash(&tag, b"payload"), domain_hash(&tag, b"payload"));
}

#[test]
fn domain_hash_differs_from_raw_sha256() {
    let tag = DomainTag::from_ascii("ONX_TX_BODY_V1");
    assert_ne!(domain_hash(&tag, b"payload"), sha256(b"payload"));
}

// --- §6.3 Ed25519 signing/verification ---

#[test]
fn ed25519_sign_and_verify_round_trip() {
    let seed = [0x11u8; 32];
    let secret = SecretKey::from_seed(&seed).unwrap();
    let public = secret.public_key();
    let tag = DomainTag::from_ascii("ONX_TX_BODY_V1");
    let message = b"transfer 10 onyx";

    let signature = secret.sign(&tag, message);
    assert!(public.verify(&tag, message, &signature).is_ok());
}

#[test]
fn ed25519_rejects_wrong_domain_tag() {
    let seed = [0x22u8; 32];
    let secret = SecretKey::from_seed(&seed).unwrap();
    let public = secret.public_key();
    let message = b"transfer 10 onyx";

    let signature = secret.sign(&DomainTag::from_ascii("ONX_TX_BODY_V1"), message);
    let verified = public.verify(
        &DomainTag::from_ascii("ONX_BLK_HDR_V1"),
        message,
        &signature,
    );
    assert_eq!(
        verified.unwrap_err(),
        PrimitiveError::SignatureVerificationFailed
    );
}

#[test]
fn ed25519_rejects_tampered_message() {
    let seed = [0x33u8; 32];
    let secret = SecretKey::from_seed(&seed).unwrap();
    let public = secret.public_key();
    let tag = DomainTag::from_ascii("ONX_TX_BODY_V1");

    let signature = secret.sign(&tag, b"transfer 10 onyx");
    let verified = public.verify(&tag, b"transfer 99 onyx", &signature);
    assert_eq!(
        verified.unwrap_err(),
        PrimitiveError::SignatureVerificationFailed
    );
}

#[test]
fn ed25519_rejects_truncated_public_key() {
    let err = PublicKey::decode_exact(&[0u8; 31]).unwrap_err();
    assert_eq!(
        err,
        PrimitiveError::Truncated {
            expected: 32,
            actual: 31
        }
    );
}

#[test]
fn ed25519_rejects_truncated_signature() {
    let err = Signature::decode_exact(&[0u8; 63]).unwrap_err();
    assert_eq!(
        err,
        PrimitiveError::Truncated {
            expected: 64,
            actual: 63
        }
    );
}

/// §5 rule 3: a non-canonical scalar `s >= L` must be rejected even though
/// `s * B == (s mod L) * B` makes the underlying curve arithmetic accept
/// it. We construct one by taking a valid signature and adding the group
/// order `L` to its `s` component (RFC 8032 §5.1, little-endian encoding),
/// which yields a different byte encoding of the same point equation but
/// is not the canonical encoding RFC 8032 requires.
#[test]
fn ed25519_rejects_non_canonical_signature_scalar() {
    const L: [u8; 32] = [
        0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde,
        0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x10,
    ];

    let seed = [0x44u8; 32];
    let secret = SecretKey::from_seed(&seed).unwrap();
    let public = secret.public_key();
    let tag = DomainTag::from_ascii("ONX_TX_BODY_V1");
    let message = b"non-canonical test";

    let signature = secret.sign(&tag, message);
    assert!(public.verify(&tag, message, &signature).is_ok());

    let mut bytes = signature.encode();
    let s = &mut bytes[32..64];

    let mut carry = 0u16;
    for i in 0..32 {
        let sum = s[i] as u16 + L[i] as u16 + carry;
        s[i] = (sum & 0xff) as u8;
        carry = sum >> 8;
    }
    assert_eq!(
        carry, 0,
        "s + L must not overflow 256 bits for a valid s < L"
    );

    let non_canonical = Signature::decode_exact(&bytes).unwrap();
    let verified = public.verify(&tag, message, &non_canonical);
    assert_eq!(
        verified.unwrap_err(),
        PrimitiveError::SignatureVerificationFailed,
        "verify_strict must reject a non-canonical s >= L"
    );
}

/// §6.3: RFC 8032 §7.1 test vector 1 verifies against the underlying
/// primitive directly (no domain separation applied), confirming the
/// dependency's raw Ed25519 semantics are RFC 8032 compliant. ONX's public
/// API always domain-separates (see `ed25519_sign_and_verify_round_trip`
/// above); this test exists only to pin the raw primitive's correctness.
#[test]
fn ed25519_rfc8032_test_vector_1_raw_primitive() {
    let secret_key_bytes =
        hex_decode("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
    let public_key_bytes =
        hex_decode("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a");
    let signature_bytes = hex_decode(
        "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
    );

    let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret_key_bytes.try_into().unwrap());
    assert_eq!(
        signing_key.verifying_key().to_bytes().to_vec(),
        public_key_bytes
    );

    let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes.try_into().unwrap());
    use ed25519_dalek::Verifier;
    assert!(signing_key.verifying_key().verify(b"", &signature).is_ok());
}

fn hex_decode(s: &str) -> Vec<u8> {
    hex::decode(s).unwrap()
}
