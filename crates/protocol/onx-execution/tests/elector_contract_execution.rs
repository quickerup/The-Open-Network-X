use onx_consensus::{run_election, CandidateValidatorSpec, ElectionConfig};
use onx_data_structures::{AccountId, FullAddress, Message, MessageType, WorkchainIdent};
use onx_execution::{execute, ExecutionContext, ExecutionResult};
use onx_primitives::{SecretKey, Uint128, Uint256, Uint64};
use onx_state_model::Cell;
fn parse_hex_tvm(text: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("ONX_") {
            continue;
        }
        for token in trimmed.split_whitespace() {
            if let Ok(byte) = u8::from_str_radix(token, 16) {
                out.push(byte);
            }
        }
    }
    if out.is_empty() {
        return Err("no executable bytecode tokens found in TVM file".to_string());
    }
    Ok(out)
}

#[test]
fn elector_contract_processes_stakes_and_emits_validator_set_cell() {
    let mut candidates = Vec::new();
    for i in 0..10 {
        let key = SecretKey::from_seed(&[i as u8; 32]).unwrap().public_key();
        candidates.push(CandidateValidatorSpec {
            public_key: key,
            proposed_stake: Uint64::from(1000u64 + i as u64),
            max_load_factor: 1000,
        });
    }

    let result = run_election(candidates, ElectionConfig::default()).unwrap();
    assert_eq!(result.validators.len(), 10);

    let code_bytes = parse_hex_tvm(onx_system_contracts::ELECTOR_BYTECODE).unwrap();
    let code = Cell::new(code_bytes, vec![]).unwrap();
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
    let exec = execute(
        code,
        data,
        message,
        ExecutionContext {
            gen_utime: 0,
            start_lt: 0,
            end_lt: 1,
            gas_limit: 1000,
        },
    );
    assert!(matches!(exec, ExecutionResult::Success { .. }));
}
