use onx_consensus::{run_election, CandidateValidatorSpec, ElectionConfig};
use onx_primitives::{PublicKey, Uint64};
use std::collections::BTreeMap;

pub fn run_elector_contract(candidates: Vec<CandidateValidatorSpec>) -> Result<Vec<ValidatorSetEntryStub>, String> {
    let result = run_election(candidates, ElectionConfig::default()).map_err(|err| err.to_string())?;
    Ok(result
        .validators
        .into_iter()
        .map(|v| ValidatorSetEntryStub {
            validator_id: v.validator_id,
            public_key_hex: hex::encode(v.public_key.encode()),
            actual_stake: v.actual_stake.0,
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorSetEntryStub {
    pub validator_id: u32,
    pub public_key_hex: String,
    pub actual_stake: u64,
}
