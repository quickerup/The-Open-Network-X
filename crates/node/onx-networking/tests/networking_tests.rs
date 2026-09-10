//! Tests for ONX Networking specs per docs/specification/networking-adnl.md, networking-dht.md, and networking-overlay.md.

use onx_networking::{derive_channel_id, xor_distance, DhtRecord, KeyDescription, NetworkError};
use onx_primitives::{hash::VALIDATOR_SIGN_V1, SecretKey, Uint256, Uint64};

#[test]
fn test_abstract_address_derivation() {
    let sk = SecretKey::from_seed(&[5u8; 32]).unwrap();
    let pk = sk.public_key();
    let key_desc = KeyDescription::new_ed25519(pk);

    let address = key_desc.compute_abstract_address();
    assert_ne!(address.0, [0u8; 32]);

    let key_desc_2 = KeyDescription::new_ed25519(pk);
    assert_eq!(key_desc_2.compute_abstract_address(), address);
}

#[test]
fn test_channel_id_and_xor_distance() {
    let shared_secret = [0xAAu8; 32];
    let addr1 = Uint256([0x11; 32]);
    let addr2 = Uint256([0x22; 32]);

    let channel_id = derive_channel_id(&shared_secret, addr1, addr2);
    assert_ne!(channel_id.0, [0u8; 32]);

    let dist = xor_distance(addr1, addr2);
    assert_eq!(dist.0[0], 0x11 ^ 0x22);
}

#[test]
fn test_dht_record_signature_verification() {
    let sk = SecretKey::from_seed(&[12u8; 32]).unwrap();
    let pk = sk.public_key();

    let key = Uint256([0x33; 32]);
    let value = vec![0x01, 0x02, 0x03];
    let expiry = Uint64::from(1000u64);
    let owner_address = Uint256([0x44; 32]);

    let signable_bytes = DhtRecord::to_signable_bytes(key, &value, expiry, owner_address);
    let tag_hash = onx_primitives::domain_hash(&DhtRecord::DOMAIN_TAG, &signable_bytes);
    let signature = sk.sign(&VALIDATOR_SIGN_V1, &tag_hash);

    let record = DhtRecord {
        key,
        value,
        expiry,
        owner_address,
        signature,
    };

    assert_eq!(record.verify_signature(&pk), Ok(()));
    assert_eq!(
        DhtRecord::decode_exact(&record.to_bytes()),
        Ok(record.clone())
    );
    assert_eq!(
        DhtRecord::decode_exact(&[record.to_bytes(), vec![0]].concat()),
        Err(NetworkError::DhtMalformedRecord)
    );

    // Tampered value -> signature failure
    let mut tampered_record = record;
    tampered_record.value = vec![0x01, 0x02, 0x04];
    assert_eq!(
        tampered_record.verify_signature(&pk),
        Err(NetworkError::InvalidSignature)
    );
}
