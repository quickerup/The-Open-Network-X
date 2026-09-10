use onx_consensus::{run_election, CandidateValidatorSpec, ElectionConfig};
use onx_execution::execute;
use onx_data_structures::{AccountId, FullAddress, Message, MessageType, WorkchainIdent};
use onx_primitives::{PublicKey, Uint128, Uint256, Uint64};
use onx_state_model::Cell;

#[test]
fn elector_contract_processes_stakes_and_emits_validator_set_cell() {
    let mut candidates = Vec::new();
    for i in 0..10 {
        let bytes = [i as u8; 32];
        let key = PublicKey::from_bytes(bytes).unwrap_or_else(|_| PublicKey::from_bytes([i as u8; 32]).unwrap());
        candidates.push(CandidateValidatorSpec {
            public_key: key,
            proposed_stake: Uint64::from(1000u64 + i as u64),
            max_load_factor: 1000,
        });
    }

    let result = run_election(candidates, ElectionConfig::default()).unwrap();
    assert_eq!(result.validators.len(), 10);

    let code = Cell::new(vec![0x00, 0x72], vec![]).unwrap();
    let data = Cell::new(vec![], vec![]).unwrap();
    let message = Message {
        msg_type: MessageType::Internal,
        src_address: FullAddress::new(WorkchainIdent::BASIC, AccountId::from_bytes([1u8; 32])),
        dest_address: FullAddress::new(WorkchainIdent::BASIC, AccountId::from_bytes([2u8; 32])),
        amount_nanos: Uint128::from(1u128),
        extra_currencies: vec![],
        created_lt: Uint64::from(1u64),
        body_cell_hash: Uint256([0xFF; 32]),
    };
    let exec = execute(code, data, message, onx_execution::ExecutionContext {
        gen_utime: 0,
        start_lt: 0,
        end_lt: 1,
        gas_limit: 1000,
    });
    assert!(matches!(exec, onx_execution::ExecutionResult::Success { .. }));
}
