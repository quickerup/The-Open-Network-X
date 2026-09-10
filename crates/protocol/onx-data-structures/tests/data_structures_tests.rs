//! Tests for canonical data structures per docs/specification/data-structures.md §6.

use onx_data_structures::{
    AccountId, BlockHeader, DataStructureError, FullAddress, Message, MessageType, ShardIdent,
    WorkchainIdent, BLOCK_HEADER_MAGIC, MERGE_RESULT_FLAG,
};
use onx_primitives::{Uint128, Uint16, Uint256, Uint32, Uint64};

#[test]
fn test_workchain_and_address_round_trip() {
    let master = WorkchainIdent::MASTERCHAIN;
    assert!(master.is_masterchain());
    assert_eq!(master.to_bytes(), [0xFF, 0xFF, 0xFF, 0xFF]);

    let basic = WorkchainIdent::BASIC;
    assert!(!basic.is_masterchain());
    assert_eq!(basic.to_bytes(), [0x00, 0x00, 0x00, 0x00]);

    let custom_wc = WorkchainIdent::new(1234);
    assert_eq!(WorkchainIdent::from_bytes(custom_wc.to_bytes()), custom_wc);

    let acct_id = AccountId::from_bytes([0x42; 32]);
    let addr = FullAddress::new(basic, acct_id);
    let bytes = addr.to_bytes();
    assert_eq!(bytes.len(), 36);
    assert_eq!(&bytes[0..4], &[0x00, 0x00, 0x00, 0x00]);
    assert_eq!(&bytes[4..36], &[0x42; 32]);

    let decoded = FullAddress::from_bytes(&bytes).unwrap();
    assert_eq!(decoded, addr);
}

#[test]
fn test_shard_ident_encoding_and_containment() {
    let wc = WorkchainIdent::BASIC;

    // Root shard: L=0, prefix_ident = 0x8000_0000_0000_0000
    let root_shard = ShardIdent::root(wc);
    assert_eq!(root_shard.prefix_len().unwrap(), 0);
    assert_eq!(root_shard.shard_prefix_ident.0, 0x8000_0000_0000_0000);

    // Prefix '0' (L=1): marker bit at position 62 -> 0x4000_0000_0000_0000
    let shard_0 = ShardIdent::from_prefix_bits(wc, 0x0000_0000_0000_0000, 1).unwrap();
    assert_eq!(shard_0.prefix_len().unwrap(), 1);
    assert_eq!(shard_0.shard_prefix_ident.0, 0x4000_0000_0000_0000);

    // Prefix '1' (L=1): marker bit at position 62, bit 63 is 1 -> 0xC000_0000_0000_0000
    let shard_1 = ShardIdent::from_prefix_bits(wc, 0x8000_0000_0000_0000, 1).unwrap();
    assert_eq!(shard_1.prefix_len().unwrap(), 1);
    assert_eq!(shard_1.shard_prefix_ident.0, 0xC000_0000_0000_0000);

    // Test account containment
    let acct_0 = AccountId::from_bytes([0x00; 32]); // MSB is 0
    let mut acct_1_bytes = [0x00; 32];
    acct_1_bytes[0] = 0x80; // MSB is 1
    let acct_1 = AccountId::from_bytes(acct_1_bytes);

    assert!(root_shard.contains_account(&acct_0).unwrap());
    assert!(root_shard.contains_account(&acct_1).unwrap());

    assert!(shard_0.contains_account(&acct_0).unwrap());
    assert!(!shard_0.contains_account(&acct_1).unwrap());

    assert!(!shard_1.contains_account(&acct_0).unwrap());
    assert!(shard_1.contains_account(&acct_1).unwrap());

    // Round trip
    let bytes = shard_0.to_bytes();
    let decoded = ShardIdent::from_bytes(&bytes).unwrap();
    assert_eq!(decoded, shard_0);
}

#[test]
fn test_shard_ident_validation_rejects_invalid() {
    let wc = WorkchainIdent::BASIC;

    // Missing marker bit (0)
    let invalid_shard = ShardIdent {
        workchain_id: wc,
        shard_prefix_ident: Uint64::from(0),
    };
    assert_eq!(
        invalid_shard.validate(),
        Err(DataStructureError::InvalidShardMarker)
    );

    // Prefix length > 60
    // If marker bit is at position 2 (trailing_zeros = 2), prefix_len = 63 - 2 = 61 (> 60)
    let invalid_len_shard = ShardIdent {
        workchain_id: wc,
        shard_prefix_ident: Uint64::from(1u64 << 2),
    };
    assert_eq!(
        invalid_len_shard.validate(),
        Err(DataStructureError::ShardPrefixLengthExceeded { length: 61 })
    );
}

#[test]
fn test_message_round_trip_and_validation() {
    let src = FullAddress::new(WorkchainIdent::BASIC, AccountId::from_bytes([0x01; 32]));
    let dest = FullAddress::new(
        WorkchainIdent::MASTERCHAIN,
        AccountId::from_bytes([0x02; 32]),
    );

    let msg = Message {
        msg_type: MessageType::Internal,
        src_address: src,
        dest_address: dest,
        amount_nanos: Uint128::from(1_000_000_000u128),
        extra_currencies: vec![
            (Uint32::from(7u32), Uint128::from(42u128)),
            (Uint32::from(9u32), Uint128::from(84u128)),
        ],
        created_lt: Uint64::from(1001u64),
        body_cell_hash: Uint256([0xAA; 32]),
    };

    let bytes = msg.to_bytes();
    assert_eq!(bytes.len(), Message::FIXED_BYTE_LENGTH + 40);
    let decoded = Message::from_bytes(&bytes).unwrap();
    assert_eq!(decoded, msg);
    assert_eq!(decoded.message_hash(), msg.message_hash());

    // Malformed message type tag
    let mut bad_bytes = bytes;
    bad_bytes[0] = 0xFF;
    assert!(matches!(
        Message::from_bytes(&bad_bytes),
        Err(DataStructureError::InvalidMessageType { tag: 0xFF })
    ));
}

#[test]
fn test_block_header_serialization_and_hashing() {
    let shard = ShardIdent::root(WorkchainIdent::MASTERCHAIN);

    let header = BlockHeader {
        magic_constructor: Uint32::from(BLOCK_HEADER_MAGIC),
        shard,
        seq_no: Uint32::from(100u32),
        flags: Uint16::from(0u16),
        gen_utime: Uint32::from(1700000000u32),
        start_lt: Uint64::from(1000u64),
        end_lt: Uint64::from(2000u64),
        prev_key_block: Uint32::from(90u32),
        prev_ref_hash: Uint256([0x11; 32]),
        prev_ref_hash_2: Uint256([0x00; 32]),
        master_ref_hash: Uint256([0x00; 32]),
        state_root_hash: Uint256([0x22; 32]),
        in_msg_root_hash: Uint256([0x33; 32]),
        out_msg_root_hash: Uint256([0x44; 32]),
    };

    let bytes = header.to_bytes();
    assert_eq!(bytes.len(), BlockHeader::BYTE_LENGTH);

    let decoded = BlockHeader::from_bytes(&bytes).unwrap();
    assert_eq!(decoded, header);

    let hash = header.block_hash();
    assert_ne!(hash.0, [0u8; 32]);

    // Rejects invalid magic constructor
    let mut bad_magic_bytes = bytes;
    bad_magic_bytes[0..4].copy_from_slice(&0xDEADBEEFu32.to_be_bytes());
    assert!(matches!(
        BlockHeader::from_bytes(&bad_magic_bytes),
        Err(DataStructureError::HeaderMagicMismatch { magic: 0xDEADBEEF })
    ));

    // Test Merge Result Block Header (ADR-0016)
    let merge_header = BlockHeader {
        magic_constructor: Uint32::from(BLOCK_HEADER_MAGIC),
        shard,
        seq_no: Uint32::from(101u32),
        flags: Uint16::from(MERGE_RESULT_FLAG),
        gen_utime: Uint32::from(1700000010u32),
        start_lt: Uint64::from(2000u64),
        end_lt: Uint64::from(3000u64),
        prev_key_block: Uint32::from(90u32),
        prev_ref_hash: Uint256([0x11; 32]),
        prev_ref_hash_2: Uint256([0x55; 32]),
        master_ref_hash: Uint256([0x00; 32]),
        state_root_hash: Uint256([0x22; 32]),
        in_msg_root_hash: Uint256([0x33; 32]),
        out_msg_root_hash: Uint256([0x44; 32]),
    };
    let merge_bytes = merge_header.to_bytes();
    let decoded_merge = BlockHeader::from_bytes(&merge_bytes).unwrap();
    assert_eq!(decoded_merge, merge_header);

    // Negative test: prev_ref_hash_2 non-zero without MERGE_RESULT_FLAG
    let mut bad_merge_bytes = merge_bytes;
    bad_merge_bytes[20..22].copy_from_slice(&0u16.to_be_bytes()); // Clear flags
    assert_eq!(
        BlockHeader::from_bytes(&bad_merge_bytes),
        Err(DataStructureError::MergeParentReferenceInconsistency)
    );

    // Negative test: prev_ref_hash_2 zero with MERGE_RESULT_FLAG set
    let mut bad_merge_bytes_2 = merge_bytes;
    bad_merge_bytes_2[78..110].copy_from_slice(&[0u8; 32]); // Zero prev_ref_hash_2 (offset 78..110)
    assert_eq!(
        BlockHeader::from_bytes(&bad_merge_bytes_2),
        Err(DataStructureError::MergeParentReferenceInconsistency)
    );
}

#[test]
fn test_truncated_and_trailing_bytes_rejection() {
    let src = FullAddress::new(WorkchainIdent::BASIC, AccountId::from_bytes([0x01; 32]));
    let dest = FullAddress::new(WorkchainIdent::BASIC, AccountId::from_bytes([0x02; 32]));
    let msg = Message {
        msg_type: MessageType::Internal,
        src_address: src,
        dest_address: dest,
        amount_nanos: Uint128::from(100u128),
        extra_currencies: vec![],
        created_lt: Uint64::from(1u64),
        body_cell_hash: Uint256([0x55; 32]),
    };
    let bytes = msg.to_bytes();

    // Truncated input
    assert!(matches!(
        Message::from_bytes(&bytes[..100]),
        Err(DataStructureError::TruncatedInput {
            expected: Message::FIXED_BYTE_LENGTH,
            got: 100
        })
    ));

    // Trailing bytes
    let mut trailing = bytes.to_vec();
    trailing.push(0x00);
    assert!(matches!(
        Message::from_bytes(&trailing),
        Err(DataStructureError::TrailingBytes { remaining: 1 })
    ));
}

#[test]
fn test_address_shard_mismatch_validation() {
    let wc = WorkchainIdent::BASIC;

    // Shard 0: prefix '0' (MSB 0)
    let shard_0 = ShardIdent::from_prefix_bits(wc, 0x0000_0000_0000_0000, 1).unwrap();
    // Shard 1: prefix '1' (MSB 1)
    let shard_1 = ShardIdent::from_prefix_bits(wc, 0x8000_0000_0000_0000, 1).unwrap();

    let acct_0 = AccountId::from_bytes([0x00; 32]); // MSB is 0
    let mut acct_1_bytes = [0x00; 32];
    acct_1_bytes[0] = 0x80; // MSB is 1
    let acct_1 = AccountId::from_bytes(acct_1_bytes);

    // 1. ShardIdent::validate_account_id
    assert_eq!(shard_0.validate_account_id(&acct_0), Ok(()));
    assert_eq!(
        shard_0.validate_account_id(&acct_1),
        Err(DataStructureError::AddressShardMismatch)
    );
    assert_eq!(shard_1.validate_account_id(&acct_1), Ok(()));
    assert_eq!(
        shard_1.validate_account_id(&acct_0),
        Err(DataStructureError::AddressShardMismatch)
    );

    // 2. FullAddress::validate_against_shard
    let addr_0 = FullAddress::new(wc, acct_0);
    let addr_1 = FullAddress::new(wc, acct_1);
    let addr_wc_mismatch = FullAddress::new(WorkchainIdent::MASTERCHAIN, acct_0);

    assert_eq!(addr_0.validate_against_shard(&shard_0), Ok(()));
    assert_eq!(
        addr_1.validate_against_shard(&shard_0),
        Err(DataStructureError::AddressShardMismatch)
    );
    assert_eq!(
        addr_wc_mismatch.validate_against_shard(&shard_0),
        Err(DataStructureError::AddressShardMismatch)
    );

    // 3. Message::validate_against_shard
    let msg_internal = Message {
        msg_type: MessageType::Internal,
        src_address: addr_0,
        dest_address: addr_1,
        amount_nanos: Uint128::from(500u128),
        extra_currencies: vec![],
        created_lt: Uint64::from(100u64),
        body_cell_hash: Uint256([0x12; 32]),
    };

    // Internal msg src is in shard_0, dest is in shard_1. Both shard_0 and shard_1 accept it.
    assert_eq!(msg_internal.validate_against_shard(&shard_0), Ok(()));
    assert_eq!(msg_internal.validate_against_shard(&shard_1), Ok(()));

    // Internal msg where neither src nor dest matches shard (e.g. Masterchain shard)
    let master_shard = ShardIdent::root(WorkchainIdent::MASTERCHAIN);
    assert_eq!(
        msg_internal.validate_against_shard(&master_shard),
        Err(DataStructureError::AddressShardMismatch)
    );

    // External Inbound: requires dest_address to match
    let msg_ext_in = Message {
        msg_type: MessageType::ExternalInbound,
        src_address: addr_0,
        dest_address: addr_1,
        amount_nanos: Uint128::from(0u128),
        extra_currencies: vec![],
        created_lt: Uint64::from(101u64),
        body_cell_hash: Uint256([0x34; 32]),
    };
    assert_eq!(msg_ext_in.validate_against_shard(&shard_1), Ok(()));
    assert_eq!(
        msg_ext_in.validate_against_shard(&shard_0),
        Err(DataStructureError::AddressShardMismatch)
    );

    // External Outbound: requires src_address to match
    let msg_ext_out = Message {
        msg_type: MessageType::ExternalOutbound,
        src_address: addr_0,
        dest_address: addr_1,
        amount_nanos: Uint128::from(0u128),
        extra_currencies: vec![],
        created_lt: Uint64::from(102u64),
        body_cell_hash: Uint256([0x56; 32]),
    };
    assert_eq!(msg_ext_out.validate_against_shard(&shard_0), Ok(()));
    assert_eq!(
        msg_ext_out.validate_against_shard(&shard_1),
        Err(DataStructureError::AddressShardMismatch)
    );
}
