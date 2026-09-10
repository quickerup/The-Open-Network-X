//! Multi-validator Catchain state-machine integration tests.
use onx_consensus::{
    proposal_signing_bytes, vote_signing_bytes, ConsensusEngine, ConsensusProposal, ConsensusStep,
    ConsensusVote, RoundTimeouts, ValidatorSetEntry, VotePhase,
};
use onx_data_structures::{ShardIdent, WorkchainIdent};
use onx_primitives::{hash::VALIDATOR_SIGN_V1, SecretKey, Uint256, Uint64};

fn validators(count: u8) -> (Vec<ValidatorSetEntry>, Vec<SecretKey>) {
    let keys: Vec<_> = (1..=count)
        .map(|id| SecretKey::from_seed(&[id; 32]).unwrap())
        .collect();
    let vals = keys
        .iter()
        .enumerate()
        .map(|(id, key)| ValidatorSetEntry {
            validator_id: id as u32,
            public_key: key.public_key(),
            actual_stake: Uint64::from(100),
        })
        .collect();
    (vals, keys)
}

fn finalize(
    engine: &mut ConsensusEngine,
    keys: &[SecretKey],
    shard: ShardIdent,
    hash: Uint256,
    online: &[usize],
) {
    let leader = engine.leader() as usize;
    let proposal = ConsensusProposal {
        height: 42,
        round: engine.round(),
        block_hash: hash,
        proposer_id: leader as u32,
        signature: keys[leader].sign(
            &VALIDATOR_SIGN_V1,
            &proposal_signing_bytes(&shard, 42, engine.round(), &hash),
        ),
    };
    engine.receive_proposal(proposal).unwrap();
    for phase in [VotePhase::PreVote, VotePhase::PreCommit, VotePhase::Commit] {
        for &id in online {
            let vote = ConsensusVote {
                height: 42,
                round: engine.round(),
                phase,
                block_hash: hash,
                validator_id: id as u32,
                signature: keys[id].sign(
                    &VALIDATOR_SIGN_V1,
                    &vote_signing_bytes(&shard, 42, engine.round(), phase, &hash),
                ),
            };
            engine.receive_vote(vote).unwrap();
        }
    }
}

#[test]
fn four_nodes_finalize_despite_one_offline_validator() {
    let shard = ShardIdent::root(WorkchainIdent::BASIC);
    let (vals, keys) = validators(4);
    let mut engine = ConsensusEngine::new(shard, 42, vals, 0, RoundTimeouts::default()).unwrap();
    let hash = Uint256([7; 32]);
    finalize(&mut engine, &keys, shard, hash, &[0, 1, 2]);
    assert_eq!(engine.step(), ConsensusStep::Finalized);
    assert_eq!(engine.finalized().unwrap().block_hash, hash);
}

#[test]
fn seven_nodes_change_view_after_byzantine_leader_and_finalize() {
    let shard = ShardIdent::root(WorkchainIdent::BASIC);
    let (vals, keys) = validators(7);
    let mut engine = ConsensusEngine::new(
        shard,
        42,
        vals,
        0,
        RoundTimeouts {
            proposal: 2,
            ..RoundTimeouts::default()
        },
    )
    .unwrap();
    // Round zero's leader is offline; all honest nodes deterministically enter round one.
    assert!(engine.on_timeout(2));
    assert_eq!(engine.round(), 1);
    assert_eq!(engine.step(), ConsensusStep::Proposal);
    let hash = Uint256([9; 32]);
    // Five of seven equal-stake validators are the exact 2/3 quorum; validator 0 is the
    // failed leader and validator 6 does not participate.
    finalize(&mut engine, &keys, shard, hash, &[1, 2, 3, 4, 5]);
    assert_eq!(engine.step(), ConsensusStep::Finalized);
    assert_eq!(engine.finalized().unwrap().round, 1);
}
