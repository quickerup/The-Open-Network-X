//! Tests for ONX Payment Channels per docs/specification/payment-channels.md.

use onx_data_structures::{AccountId, FullAddress, WorkchainIdent};
use onx_payment_channels::{ChannelError, ChannelState, PaymentChannelArbiter};
use onx_primitives::{hash::TX_BODY_V1, SecretKey, Uint128, Uint256, Uint64};
use onx_state_model::Cell;

fn setup_arbiter() -> (
    PaymentChannelArbiter,
    SecretKey,
    SecretKey,
    FullAddress,
    FullAddress,
) {
    let sk_a = SecretKey::from_seed(&[1u8; 32]).unwrap();
    let sk_b = SecretKey::from_seed(&[2u8; 32]).unwrap();
    let pk_a = sk_a.public_key();
    let pk_b = sk_b.public_key();

    let party_a = FullAddress::new(WorkchainIdent::BASIC, AccountId::from_bytes([0x01; 32]));
    let party_b = FullAddress::new(WorkchainIdent::BASIC, AccountId::from_bytes([0x02; 32]));

    let channel_id = Uint256([0x77; 32]);
    let deposit = Uint128::from(1000u128);
    let challenge_window = Uint64::from(50u64);

    let arbiter = PaymentChannelArbiter::new(
        channel_id,
        party_a,
        party_b,
        pk_a,
        pk_b,
        deposit,
        challenge_window,
    );

    (arbiter, sk_a, sk_b, party_a, party_b)
}

#[test]
fn test_cooperative_settlement_success() {
    let (mut arbiter, sk_a, sk_b, _, _) = setup_arbiter();

    let state = ChannelState {
        channel_id: arbiter.channel_id,
        sequence: Uint64::from(1u64),
        balance_party_a: Uint128::from(600u128),
        balance_party_b: Uint128::from(400u128),
        condition_hash: None,
        expiry_lt: None,
    };

    let state_hash = state.hash();
    let sig_a = sk_a.sign(&TX_BODY_V1, &state_hash.0);
    let sig_b = sk_b.sign(&TX_BODY_V1, &state_hash.0);

    assert_eq!(
        arbiter.cooperative_settle(state.clone(), sig_a, sig_b),
        Ok(())
    );
    assert!(arbiter.is_settled);
    assert_eq!(arbiter.latest_state, state);
}

#[test]
fn test_uncooperative_dispute_and_finalization() {
    let (mut arbiter, sk_a, sk_b, _, _) = setup_arbiter();

    let state1 = ChannelState {
        channel_id: arbiter.channel_id,
        sequence: Uint64::from(1u64),
        balance_party_a: Uint128::from(800u128),
        balance_party_b: Uint128::from(200u128),
        condition_hash: None,
        expiry_lt: None,
    };
    let hash1 = state1.hash();
    let sig_a1 = sk_a.sign(&TX_BODY_V1, &hash1.0);
    let sig_b1 = sk_b.sign(&TX_BODY_V1, &hash1.0);

    assert_eq!(
        arbiter.submit_uncooperative_state(state1, sig_a1, sig_b1, Uint64::from(100u64)),
        Ok(())
    );

    // Finalize before challenge window expires should fail
    assert_eq!(
        arbiter.finalize_uncooperative_settlement(Uint64::from(120u64)),
        Err(ChannelError::ChallengePeriodActive)
    );

    // Finalize after challenge window expires (100 + 50 = 150) should succeed
    assert_eq!(
        arbiter.finalize_uncooperative_settlement(Uint64::from(151u64)),
        Ok(())
    );
    assert!(arbiter.is_settled);
}

#[test]
fn test_merkle_proof_verification() {
    let (arbiter, _, _, _, _) = setup_arbiter();

    let valid_cell = Cell::new(vec![0x01, 0x02], vec![]).unwrap();
    let pruned_cell = Cell::new_with_special(vec![0x00], vec![], true).unwrap();

    assert_eq!(arbiter.verify_merkle_proof(&valid_cell), Ok(()));
    assert_eq!(
        arbiter.verify_merkle_proof(&pruned_cell),
        Err(ChannelError::InvalidProof)
    );
}
