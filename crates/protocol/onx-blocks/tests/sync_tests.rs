use onx_blocks::{
    BlockSyncEngine, MasterchainBlockExtra, RecomputedRoots, ShardEntry, SyncCandidate, SyncError,
};
use onx_data_structures::{BlockHeader, ShardIdent, WorkchainIdent, BLOCK_HEADER_MAGIC};
use onx_primitives::{Uint16, Uint256, Uint32, Uint64};

fn header(shard: ShardIdent, seq_no: u32) -> BlockHeader {
    BlockHeader {
        magic_constructor: Uint32(BLOCK_HEADER_MAGIC),
        shard,
        seq_no: Uint32(seq_no),
        flags: Uint16::ZERO,
        gen_utime: Uint32(100 + seq_no),
        start_lt: Uint64(u64::from(seq_no) * 100),
        end_lt: Uint64(u64::from(seq_no) * 100 + 99),
        prev_key_block: Uint32::ZERO,
        prev_ref_hash: Uint256::ZERO,
        prev_ref_hash_2: Uint256::ZERO,
        master_ref_hash: Uint256::ZERO,
        state_root_hash: Uint256([1; 32]),
        in_msg_root_hash: Uint256([2; 32]),
        out_msg_root_hash: Uint256([3; 32]),
    }
}

fn roots(header: &BlockHeader) -> RecomputedRoots {
    RecomputedRoots {
        state_root_hash: header.state_root_hash,
        in_msg_root_hash: header.in_msg_root_hash,
        out_msg_root_hash: header.out_msg_root_hash,
    }
}

#[test]
fn imports_canonical_shard_block_against_held_masterchain_proof() {
    let master_shard = ShardIdent::root(WorkchainIdent::MASTERCHAIN);
    let shard = ShardIdent::root(WorkchainIdent::BASIC);
    let master_genesis = header(master_shard, 0);
    let shard_genesis = header(shard, 0);
    let mut engine = BlockSyncEngine::default();
    engine
        .insert_trusted(
            master_genesis.clone(),
            Some(MasterchainBlockExtra::default()),
        )
        .unwrap();
    engine.insert_trusted(shard_genesis.clone(), None).unwrap();

    let mut shard_one = header(shard, 1);
    shard_one.prev_ref_hash = shard_genesis.block_hash();
    shard_one.master_ref_hash = master_genesis.block_hash();
    let mut master_one = header(master_shard, 1);
    master_one.prev_ref_hash = master_genesis.block_hash();
    let extra = MasterchainBlockExtra {
        shard_entries: vec![ShardEntry {
            shard,
            block_hash: shard_one.block_hash(),
            seq_no: Uint32(1),
        }],
    };
    let master_one_hash = master_one.block_hash();

    engine
        .import_batch(&[
            SyncCandidate {
                header: master_one.clone(),
                recomputed_roots: roots(&master_one),
                masterchain_extra: Some(extra),
                canonicality_proof: None,
            },
            SyncCandidate {
                header: shard_one.clone(),
                recomputed_roots: roots(&shard_one),
                masterchain_extra: None,
                canonicality_proof: Some(master_one_hash),
            },
        ])
        .unwrap();
    assert_eq!(engine.header(&shard_one.block_hash()), Some(&shard_one));
}

#[test]
fn rejects_invalid_canonicality_proof_without_partially_importing_batch() {
    let master_shard = ShardIdent::root(WorkchainIdent::MASTERCHAIN);
    let shard = ShardIdent::root(WorkchainIdent::BASIC);
    let master_genesis = header(master_shard, 0);
    let shard_genesis = header(shard, 0);
    let mut engine = BlockSyncEngine::default();
    engine
        .insert_trusted(
            master_genesis.clone(),
            Some(MasterchainBlockExtra::default()),
        )
        .unwrap();
    engine.insert_trusted(shard_genesis.clone(), None).unwrap();

    let mut shard_one = header(shard, 1);
    shard_one.prev_ref_hash = shard_genesis.block_hash();
    shard_one.master_ref_hash = master_genesis.block_hash();
    let mut master_one = header(master_shard, 1);
    master_one.prev_ref_hash = master_genesis.block_hash();
    let master_one_hash = master_one.block_hash();
    let result = engine.import_batch(&[
        SyncCandidate {
            header: master_one.clone(),
            recomputed_roots: roots(&master_one),
            masterchain_extra: Some(MasterchainBlockExtra::default()),
            canonicality_proof: None,
        },
        SyncCandidate {
            header: shard_one.clone(),
            recomputed_roots: roots(&shard_one),
            masterchain_extra: None,
            canonicality_proof: Some(master_one_hash),
        },
    ]);
    assert_eq!(
        result,
        Err(SyncError::InvalidCanonicalityProof(master_one_hash))
    );
    assert!(engine.header(&master_one_hash).is_none());
    assert!(engine.header(&shard_one.block_hash()).is_none());
}

#[test]
fn rejects_shard_header_with_modified_state_root() {
    let master_shard = ShardIdent::root(WorkchainIdent::MASTERCHAIN);
    let shard = ShardIdent::root(WorkchainIdent::BASIC);
    let master_genesis = header(master_shard, 0);
    let shard_genesis = header(shard, 0);
    let mut engine = BlockSyncEngine::default();
    engine
        .insert_trusted(
            master_genesis.clone(),
            Some(MasterchainBlockExtra::default()),
        )
        .unwrap();
    engine.insert_trusted(shard_genesis.clone(), None).unwrap();

    let mut shard_one = header(shard, 1);
    shard_one.prev_ref_hash = shard_genesis.block_hash();
    shard_one.master_ref_hash = master_genesis.block_hash();
    let mut master_one = header(master_shard, 1);
    master_one.prev_ref_hash = master_genesis.block_hash();
    let master_one_hash = master_one.block_hash();
    let proof = MasterchainBlockExtra {
        shard_entries: vec![ShardEntry {
            shard,
            block_hash: shard_one.block_hash(),
            seq_no: Uint32(1),
        }],
    };
    let mut wrong_roots = roots(&shard_one);
    wrong_roots.state_root_hash = Uint256([9; 32]);
    let result = engine.import_batch(&[
        SyncCandidate {
            header: master_one.clone(),
            recomputed_roots: roots(&master_one),
            masterchain_extra: Some(proof),
            canonicality_proof: None,
        },
        SyncCandidate {
            header: shard_one,
            recomputed_roots: wrong_roots,
            masterchain_extra: None,
            canonicality_proof: Some(master_one_hash),
        },
    ]);
    assert_eq!(
        result,
        Err(SyncError::Block(onx_blocks::BlocksError::StateRootMismatch))
    );
}

#[test]
fn imports_descendant_first_masterchain_response() {
    let master_shard = ShardIdent::root(WorkchainIdent::MASTERCHAIN);
    let master_genesis = header(master_shard, 0);
    let mut engine = BlockSyncEngine::default();
    engine
        .insert_trusted(
            master_genesis.clone(),
            Some(MasterchainBlockExtra::default()),
        )
        .unwrap();
    let mut first = header(master_shard, 1);
    first.prev_ref_hash = master_genesis.block_hash();
    let mut second = header(master_shard, 2);
    second.prev_ref_hash = first.block_hash();

    engine
        .import_reverse_batch(&[
            SyncCandidate {
                header: second.clone(),
                recomputed_roots: roots(&second),
                masterchain_extra: Some(MasterchainBlockExtra::default()),
                canonicality_proof: None,
            },
            SyncCandidate {
                header: first.clone(),
                recomputed_roots: roots(&first),
                masterchain_extra: Some(MasterchainBlockExtra::default()),
                canonicality_proof: None,
            },
        ])
        .unwrap();
    assert_eq!(engine.header(&second.block_hash()), Some(&second));
}
