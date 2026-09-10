//! Tests for ONX Consensus and Validator Operation per docs/specification/consensus.md.

use onx_consensus::{
    calculate_actual_stake, calculate_late_reward, is_block_irrevocably_final, BftQuorumTracker,
    BlockSignatureWithDepth, ConsensusError, ValidatorSetEntry, CHALLENGE_WINDOW_BLOCKS,
};
use onx_data_structures::{ShardIdent, WorkchainIdent};
use onx_primitives::{hash::VALIDATOR_SIGN_V1, SecretKey, Uint256, Uint64};

#[test]
fn test_actual_stake_calculation() {
    // s_i = 1000, l_i = 10.0 (1000 scaled), min_selected = 80
    // cap = 10.0 * 80 = 800 -> min(1000, 800) = 800
    let actual = calculate_actual_stake(1000, 1000, 80);
    assert_eq!(actual, 800);

    // uncapped case: s_i = 500 < cap = 800 -> 500
    let uncapped = calculate_actual_stake(500, 1000, 80);
    assert_eq!(uncapped, 500);
}

#[test]
fn test_bft_quorum_voting() {
    let shard = ShardIdent::root(WorkchainIdent::BASIC);
    let block_hash = Uint256([0x88; 32]);
    let total_stake = 300u64;

    let mut tracker = BftQuorumTracker::new(shard, block_hash, total_stake);

    let sk1 = SecretKey::from_seed(&[10u8; 32]).unwrap();
    let sk2 = SecretKey::from_seed(&[20u8; 32]).unwrap();

    let val1 = ValidatorSetEntry {
        validator_id: 1,
        public_key: sk1.public_key(),
        actual_stake: Uint64::from(100u64),
    };
    let val2 = ValidatorSetEntry {
        validator_id: 2,
        public_key: sk2.public_key(),
        actual_stake: Uint64::from(100u64),
    };

    let sig1 = sk1.sign(&VALIDATOR_SIGN_V1, &block_hash.0);
    let sig2 = sk2.sign(&VALIDATOR_SIGN_V1, &block_hash.0);

    let vote1 = BlockSignatureWithDepth {
        validator_id: 1,
        depth: 0,
        signature: sig1,
    };
    let vote2 = BlockSignatureWithDepth {
        validator_id: 2,
        depth: 0,
        signature: sig2,
    };

    // 100 / 300 stake -> no quorum (33.3%)
    tracker.add_vote(&vote1, &val1).unwrap();
    assert!(!tracker.has_quorum());

    // 200 / 300 stake -> quorum reached (66.67%)
    tracker.add_vote(&vote2, &val2).unwrap();
    assert!(tracker.has_quorum());
    assert_eq!(tracker.verify_quorum(), Ok(()));

    // Duplicate vote rejection
    assert_eq!(
        tracker.add_vote(&vote1, &val1),
        Err(ConsensusError::DuplicateValidatorSignature)
    );
}

#[test]
fn test_late_signature_reward_decay() {
    let base_reward = 1_000_000u128;

    assert_eq!(calculate_late_reward(base_reward, 0), base_reward);
    let r1 = calculate_late_reward(base_reward, 1);
    assert_eq!(r1, 900_000); // 90%

    let r16 = calculate_late_reward(base_reward, 16);
    assert!(r16 > 0 && r16 < base_reward);

    assert_eq!(calculate_late_reward(base_reward, 17), 0); // > 16 -> 0
}

#[test]
fn test_absolute_finality_rule() {
    let commit_height = 1_000_000u64;

    assert!(!is_block_irrevocably_final(
        commit_height,
        commit_height + 500,
        false
    ));

    assert!(is_block_irrevocably_final(
        commit_height,
        commit_height + CHALLENGE_WINDOW_BLOCKS,
        false
    ));

    // Challenge present -> not final
    assert!(!is_block_irrevocably_final(
        commit_height,
        commit_height + CHALLENGE_WINDOW_BLOCKS + 10,
        true
    ));
}

#[test]
fn election_selects_stake_weighted_set_and_returns_unlocked_funds() {
    use onx_consensus::{run_election, CandidateValidatorSpec, ElectionConfig};

    let candidates = [(1000, 200), (500, 300), (200, 100), (100, 100)]
        .into_iter()
        .enumerate()
        .map(|(index, (stake, load))| CandidateValidatorSpec {
            public_key: SecretKey::from_seed(&[(index + 1) as u8; 32])
                .unwrap()
                .public_key(),
            proposed_stake: Uint64::from(stake),
            max_load_factor: load,
        })
        .collect();
    let election = run_election(
        candidates,
        ElectionConfig {
            validator_cap: 3,
            max_load_factor_scaled: 1_000,
        },
    )
    .unwrap();

    // The third selected proposal (200) is the cap baseline.  The first
    // candidate is capped to 400; its excess and the unselected stake unlock.
    assert_eq!(
        election
            .validators
            .iter()
            .map(|validator| validator.actual_stake.0)
            .collect::<Vec<_>>(),
        vec![400, 500, 200]
    );
    assert_eq!(
        election
            .refunds
            .iter()
            .map(|refund| refund.amount.0)
            .collect::<Vec<_>>(),
        vec![600, 0, 0, 100]
    );
}

#[test]
fn election_rejects_invalid_or_duplicate_candidates() {
    use onx_consensus::{run_election, CandidateValidatorSpec, ElectionConfig};

    let key = SecretKey::from_seed(&[9; 32]).unwrap().public_key();
    let candidate = CandidateValidatorSpec {
        public_key: key,
        proposed_stake: Uint64::from(10),
        max_load_factor: 100,
    };
    assert_eq!(
        run_election(
            vec![candidate.clone(), candidate],
            ElectionConfig::default()
        ),
        Err(ConsensusError::DuplicateCandidate)
    );
    assert_eq!(
        run_election(
            vec![CandidateValidatorSpec {
                public_key: key,
                proposed_stake: Uint64::from(0),
                max_load_factor: 100,
            }],
            ElectionConfig::default(),
        ),
        Err(ConsensusError::InvalidCandidateStake)
    );
}
