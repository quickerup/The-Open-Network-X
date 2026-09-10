//! Integration coverage for RLDP's ADNL-sized datagrams on a lossy channel.

use onx_networking::rldp::{RldpConfig, RldpDataFrame, RldpReceiver, RldpSender};
use onx_primitives::Uint256;
use sha2::{Digest, Sha256};

#[test]
fn reassembles_one_megabyte_after_twenty_percent_packet_loss() {
    // A non-repeating payload catches both ordering and zero-padding mistakes.
    let payload: Vec<u8> = (0..(1024 * 1024 + 137))
        .map(|i| ((i * 31 + i / 251) % 251) as u8)
        .collect();
    let expected_hash: [u8; 32] = Sha256::digest(&payload).into();
    let config = RldpConfig {
        packets_per_round: 192,
        max_rounds: 80,
        ..RldpConfig::default()
    };
    let mut sender = RldpSender::new(Uint256([9; 32]), &payload, config).unwrap();
    let mut receiver = None;
    let mut delivered = None;
    let mut packet_number = 0usize;

    for _ in 0..config.max_rounds {
        // The deterministic predicate drops exactly 20% of ADNL-sized UDP
        // datagrams, including both first-pass symbols and retry symbols.
        for wire in sender.next_round() {
            packet_number += 1;
            if packet_number.is_multiple_of(5) {
                continue;
            }
            let frame = RldpDataFrame::decode(&wire).unwrap();
            let state = receiver.get_or_insert_with(|| RldpReceiver::from_frame(&frame).unwrap());
            if let Some(message) = state.ingest(frame).unwrap() {
                delivered = Some(message);
            }
            sender.apply_ack(&state.ack()).unwrap();
        }
        if sender.is_complete() {
            break;
        }
    }

    let delivered = delivered.expect("all chunks must survive erasure coding and retry rounds");
    assert!(sender.is_complete());
    assert_eq!(Sha256::digest(&delivered).as_slice(), expected_hash);
    assert_eq!(delivered, payload);
}
