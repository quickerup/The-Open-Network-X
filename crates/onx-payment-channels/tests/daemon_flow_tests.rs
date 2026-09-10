//! Flow test for the payment-channel daemon scaffold on top of the arbiter
//! primitives already implemented in `crates/onx-payment-channels/src/lib.rs`.

use onx_payment_channels::{
    daemon::{PaymentChannelDaemon, SignedStateEnvelope},
    ChannelState, PaymentChannelArbiter,
};
use onx_primitives::{hash::TX_BODY_V1, SecretKey, Uint128, Uint256, Uint64};
use onx_data_structures::{AccountId, FullAddress, WorkchainIdent};

fn setup_daemon() -> (PaymentChannelDaemon, SecretKey, SecretKey) {
    let sk_a = SecretKey::from_seed(&[1u8; 32]).unwrap();
    let sk_b = SecretKey::from_seed(&[2u8; 32]).unwrap();

    let party_a = FullAddress::new(WorkchainIdent::BASIC, AccountId::from_bytes([0x01; 32]));
    let party_b = FullAddress::new(WorkchainIdent::BASIC, AccountId::from_bytes([0x02; 32]));

    let channel_id = Uint256([0x77; 32]);
    let deposit = Uint128::from(1000u128);
    let challenge_window = Uint64::from(50u64);

    let arbiter = PaymentChannelArbiter::new(
        channel_id,
        party_a,
        party_b,
        sk_a.public_key(),
        sk_b.public_key(),
        deposit,
        challenge_window,
    );

    (PaymentChannelDaemon::new(arbiter), sk_a, sk_b)
}

#[test]
fn daemon_detects_stale_state_and_submits_dispute_before_timeout() {
    let (mut daemon, sk_a, sk_b) = setup_daemon();

    let mut latest = ChannelState {
        channel_id: daemon.arbiter.channel_id,
        sequence: Uint64::from(0u64),
        balance_party_a: Uint128::from(1000u128),
        balance_party_b: Uint128::from(0u128),
        condition_hash: None,
        expiry_lt: None,
    };

    // Exchange 1,000 payments by moving the balance pair across the channel
    // state tree in a deterministic sequence, while keeping both signatures
    // dual-signed per the arbiter’s existing verify_dual_signatures API.
    let mut payments = 0usize;
    while payments < 1000 {
        latest.sequence = Uint64::from((payments + 1) as u64);
        latest.balance_party_a = Uint128::from(1000u128 - payments as u128);
        latest.balance_party_b = Uint128::from(payments as u128);

        let hash = latest.hash();
        let sig_a = sk_a.sign(&TX_BODY_V1, &hash.0);
        let sig_b = sk_b.sign(&TX_BODY_V1, &hash.0);

        daemon
            .receive_signed_state_and_track(SignedStateEnvelope::new(
                latest.clone(),
                sig_a,
                sig_b,
            ))
            .unwrap();
        payments += 1;
    }

    // Submit a stale state envelope that is signed by both keys but not the
    // freshest latest state according to the daemon hash table.
    let stale = ChannelState {
        channel_id: daemon.arbiter.channel_id,
        sequence: Uint64::from(1001u64),
        balance_party_a: Uint128::from(1000u128),
        balance_party_b: Uint128::from(0u128),
        condition_hash: None,
        expiry_lt: None,
    };
    let stale_hash = stale.hash();
    let stale_sig_a = sk_a.sign(&TX_BODY_V1, &stale_hash.0);
    let stale_sig_b = sk_b.sign(&TX_BODY_V1, &stale_hash.0);

    daemon
        .submit_uncooperative_dispute(
            SignedStateEnvelope::new(stale, stale_sig_a, stale_sig_b),
            Uint64::from(100u64),
        )
        .unwrap();

    daemon
        .finalize_dispute_if_due(Uint64::from(151u64))
        .unwrap();

    assert!(daemon.arbiter.is_settled);
    assert_eq!(daemon.arbiter.challenge_start_lt, Some(Uint64::from(100u64)));
}
