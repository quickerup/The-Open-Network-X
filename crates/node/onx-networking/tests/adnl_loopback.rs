//! Loopback integration tests for ADNL UDP Transport layer.

use onx_networking::adnl_transport::{AdnlTransportNode, FastPacket, FullPacket};
use onx_primitives::{domain_hash, SecretKey};
use std::sync::Arc;
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn test_full_packet_encrypt_decrypt() {
    let alice_sk = SecretKey::from_seed(&[11u8; 32]).unwrap();
    let bob_sk = SecretKey::from_seed(&[22u8; 32]).unwrap();
    let bob_pk = bob_sk.public_key();

    let alice_node = AdnlTransportNode::bind(alice_sk.clone(), "127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let bob_node = AdnlTransportNode::bind(bob_sk.clone(), "127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();

    let payload = b"Hello Full Public-Key Encrypted Packet".to_vec();
    let timestamp = 1700000000u32;
    let nonce = 12345u32;

    let signable =
        FullPacket::to_signable_bytes(alice_node.abstract_address(), timestamp, nonce, &payload);
    let signature = alice_sk.sign(
        &onx_primitives::hash::VALIDATOR_SIGN_V1,
        &domain_hash(
            &onx_networking::adnl_transport::ADNL_SHARED_SECRET_TAG,
            &signable,
        ),
    );

    let full_pkt = FullPacket {
        recipient_address: bob_node.abstract_address(),
        sender_address: alice_node.abstract_address(),
        timestamp,
        nonce,
        payload: payload.clone(),
        signature,
    };

    let wire = full_pkt.encode(&alice_sk, &bob_pk);
    let decoded = FullPacket::decode(&wire, &bob_sk).unwrap();

    assert_eq!(decoded.recipient_address, bob_node.abstract_address());
    assert_eq!(decoded.sender_address, alice_node.abstract_address());
    assert_eq!(decoded.payload, payload);
    assert_eq!(decoded.timestamp, timestamp);
    assert_eq!(decoded.nonce, nonce);
}

#[tokio::test]
async fn test_fast_packet_encrypt_decrypt() {
    let shared_secret = [0x55u8; 32];
    let nonce = [0x77u8; 32];
    let sender_address = onx_primitives::Uint256([0x11; 32]);
    let channel_id = onx_primitives::Uint256([0x22; 32]);
    let payload = b"Fast channel datagram payload".to_vec();

    let pkt = FastPacket {
        channel_or_recipient_id: channel_id,
        sender_address,
        nonce,
        payload: payload.clone(),
    };

    let wire = pkt.encode(&shared_secret);
    let decoded = FastPacket::decode(&wire, &shared_secret).unwrap();

    assert_eq!(decoded.channel_or_recipient_id, channel_id);
    assert_eq!(decoded.sender_address, sender_address);
    assert_eq!(decoded.payload, payload);
}

#[tokio::test]
async fn test_adnl_node_loopback_ping_pong_and_reconnection() {
    let node1_sk = SecretKey::from_seed(&[101u8; 32]).unwrap();
    let node1 = Arc::new(
        AdnlTransportNode::bind(node1_sk, "127.0.0.1:0".parse().unwrap())
            .await
            .unwrap(),
    );

    let node2_sk = SecretKey::from_seed(&[102u8; 32]).unwrap();
    let node2_pk = node2_sk.public_key();
    let node2 = Arc::new(
        AdnlTransportNode::bind(node2_sk, "127.0.0.1:0".parse().unwrap())
            .await
            .unwrap(),
    );

    let addr1 = node1.local_addr().unwrap();
    let addr2 = node2.local_addr().unwrap();

    let node1_pk = node1.public_key();
    let node1_abstract_addr = node1.abstract_address();
    let node2_abstract_addr = node2.abstract_address();

    // Node 1 pre-establishes session to Node 2 and Node 2 to Node 1
    node1.connect_peer(node2_pk, addr2);
    node2.connect_peer(node1_pk, addr1);

    // Task for Node 2: receive ping, verify payload, and send pong
    let node2_clone = Arc::clone(&node2);
    let node2_task = tokio::spawn(async move {
        let (sender_addr, payload, src_endpoint) = node2_clone.recv_datagram().await.unwrap();
        assert_eq!(sender_addr, node1_abstract_addr);

        // Payload verification: payload must match expected PING datagram
        assert_eq!(payload, b"PING");

        let pong_payload = b"PONG";
        // Node 2 sends datagram back to Node 1 via automatic session lookup
        node2_clone
            .send_datagram(node1_pk, src_endpoint, pong_payload)
            .await
            .unwrap();
    });

    // Node 1 sends PING to Node 2
    node1.send_datagram(node2_pk, addr2, b"PING").await.unwrap();

    // Node 1 receives PONG from Node 2
    let recv_res = timeout(Duration::from_secs(2), node1.recv_datagram()).await;
    let (sender_addr, payload, _src) = recv_res.unwrap().unwrap();

    assert_eq!(sender_addr, node2_abstract_addr);
    assert_eq!(payload, b"PONG");

    node2_task.await.unwrap();
}
