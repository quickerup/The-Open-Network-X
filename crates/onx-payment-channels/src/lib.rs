use onx_data_structures::{AccountId, FullAddress, Message, MessageType, WorkchainIdent};
use onx_execution::{ExceptionKind, ExecutionContext, ExecutionResult};
use onx_primitives::{
    domain_hash,
    hash::{DomainTag, TX_BODY_V1},
    PublicKey, Signature, Uint128, Uint256, Uint64,
};
use onx_state_model::Cell;
use std::fmt;

/// Errors arising in payment channel operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelError {
    InvalidSignature,
    NonIncreasingSequence,
    AllocationExceedsDeposit,
    ExpiredCondition,
    MismatchedChannelId,
    InvalidProof,
    ChallengePeriodActive,
    ChallengePeriodExpired,
}

impl fmt::Display for ChannelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSignature => write!(f, "Invalid participant signature"),
            Self::NonIncreasingSequence => write!(f, "Sequence number is not strictly increasing"),
            Self::AllocationExceedsDeposit => write!(f, "Total allocation exceeds locked deposit"),
            Self::ExpiredCondition => write!(f, "Condition or promise has expired"),
            Self::MismatchedChannelId => write!(f, "Mismatched channel identifier"),
            Self::InvalidProof => write!(f, "Invalid Merkle proof or pruned cell"),
            Self::ChallengePeriodActive => {
                write!(f, "Uncooperative challenge period is still active")
            }
            Self::ChallengePeriodExpired => write!(f, "Challenge period has already expired"),
        }
    }
}

impl std::error::Error for ChannelError {}

pub mod daemon;

/// Payment channel state record per docs/specification/payment-channels.md §4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelState {
    pub channel_id: Uint256,
    pub sequence: Uint64,
    pub balance_party_a: Uint128,
    pub balance_party_b: Uint128,
    pub condition_hash: Option<Uint256>,
    pub expiry_lt: Option<Uint64>,
}

impl ChannelState {
    pub const DOMAIN_TAG: DomainTag = DomainTag::from_ascii("ONX_CHANNEL_STATE_V1");

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(128);
        buf.extend_from_slice(&self.channel_id.encode());
        buf.extend_from_slice(&self.sequence.encode());
        buf.extend_from_slice(&self.balance_party_a.encode());
        buf.extend_from_slice(&self.balance_party_b.encode());
        if let Some(ch) = &self.condition_hash {
            buf.push(1);
            buf.extend_from_slice(&ch.encode());
        } else {
            buf.push(0);
        }
        if let Some(exp) = &self.expiry_lt {
            buf.push(1);
            buf.extend_from_slice(&exp.encode());
        } else {
            buf.push(0);
        }
        buf
    }

    pub fn hash(&self) -> Uint256 {
        Uint256(domain_hash(&Self::DOMAIN_TAG, &self.to_bytes()))
    }
}

/// On-chain payment channel arbiter contract model per docs/specification/payment-channels.md §3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentChannelArbiter {
    pub channel_id: Uint256,
    pub party_a: FullAddress,
    pub party_b: FullAddress,
    pub pubkey_a: PublicKey,
    pub pubkey_b: PublicKey,
    pub total_deposit: Uint128,
    pub challenge_window_lt: Uint64,
    pub latest_state: ChannelState,
    pub challenge_start_lt: Option<Uint64>,
    pub is_settled: bool,
}

impl PaymentChannelArbiter {
    pub fn new(
        channel_id: Uint256,
        party_a: FullAddress,
        party_b: FullAddress,
        pubkey_a: PublicKey,
        pubkey_b: PublicKey,
        total_deposit: Uint128,
        challenge_window_lt: Uint64,
    ) -> Self {
        let initial_state = ChannelState {
            channel_id,
            sequence: Uint64::from(0u64),
            balance_party_a: total_deposit,
            balance_party_b: Uint128::from(0u128),
            condition_hash: None,
            expiry_lt: None,
        };
        Self {
            channel_id,
            party_a,
            party_b,
            pubkey_a,
            pubkey_b,
            total_deposit,
            challenge_window_lt,
            latest_state: initial_state,
            challenge_start_lt: None,
            is_settled: false,
        }
    }

    pub fn verify_dual_signatures(
        &self,
        state: &ChannelState,
        sig_a: &Signature,
        sig_b: &Signature,
    ) -> Result<(), ChannelError> {
        let state_hash = state.hash();
        if self
            .pubkey_a
            .verify(&TX_BODY_V1, &state_hash.0, sig_a)
            .is_err()
        {
            return Err(ChannelError::InvalidSignature);
        }
        if self
            .pubkey_b
            .verify(&TX_BODY_V1, &state_hash.0, sig_b)
            .is_err()
        {
            return Err(ChannelError::InvalidSignature);
        }
        Ok(())
    }

    /// Cooperative mutual settlement signed by both parties.
    pub fn cooperative_settle(
        &mut self,
        state: ChannelState,
        sig_a: Signature,
        sig_b: Signature,
    ) -> Result<(), ChannelError> {
        if state.channel_id != self.channel_id {
            return Err(ChannelError::MismatchedChannelId);
        }
        if state
            .balance_party_a
            .0
            .saturating_add(state.balance_party_b.0)
            > self.total_deposit.0
        {
            return Err(ChannelError::AllocationExceedsDeposit);
        }
        self.verify_dual_signatures(&state, &sig_a, &sig_b)?;
        self.latest_state = state;
        self.is_settled = true;
        Ok(())
    }

    /// Uncooperative dispute initiation or state update challenge.
    pub fn submit_uncooperative_state(
        &mut self,
        state: ChannelState,
        sig_a: Signature,
        sig_b: Signature,
        current_lt: Uint64,
    ) -> Result<(), ChannelError> {
        if state.channel_id != self.channel_id {
            return Err(ChannelError::MismatchedChannelId);
        }
        if state.sequence.0 <= self.latest_state.sequence.0 {
            return Err(ChannelError::NonIncreasingSequence);
        }
        if state
            .balance_party_a
            .0
            .saturating_add(state.balance_party_b.0)
            > self.total_deposit.0
        {
            return Err(ChannelError::AllocationExceedsDeposit);
        }
        self.verify_dual_signatures(&state, &sig_a, &sig_b)?;
        self.latest_state = state;
        self.challenge_start_lt = Some(current_lt);
        Ok(())
    }

    /// Finalizes uncooperative settlement after challenge window expiry.
    pub fn finalize_uncooperative_settlement(
        &mut self,
        current_lt: Uint64,
    ) -> Result<(), ChannelError> {
        let start_lt = self
            .challenge_start_lt
            .ok_or(ChannelError::ChallengePeriodActive)?;
        if current_lt.0 < start_lt.0.saturating_add(self.challenge_window_lt.0) {
            return Err(ChannelError::ChallengePeriodActive);
        }
        self.is_settled = true;
        Ok(())
    }

    /// Validates conditional Merkle proof via onx-execution CTOS execution (AbsentNode check).
    pub fn verify_merkle_proof(&self, proof_cell: &Cell) -> Result<(), ChannelError> {
        // Construct code: CTOS (0x45) + RET (0x72)
        let code = Cell::new(vec![0x45, 0x72], vec![]).map_err(|_| ChannelError::InvalidProof)?;
        let dummy_addr = FullAddress::new(WorkchainIdent::BASIC, AccountId::from_bytes([0u8; 32]));
        let dummy_msg = Message {
            msg_type: MessageType::Internal,
            src_address: dummy_addr,
            dest_address: dummy_addr,
            amount_nanos: Uint128::from(0u128),
            extra_currencies: vec![],
            created_lt: Uint64::from(0u64),
            body_cell_hash: Uint256([0u8; 32]),
        };
        let context = ExecutionContext {
            gen_utime: 0,
            start_lt: 0,
            end_lt: 100,
            gas_limit: 1000,
        };

        let empty_cell = Cell::new(vec![], vec![]).map_err(|_| ChannelError::InvalidProof)?;
        let mut interpreter = onx_execution::Interpreter::new(code, empty_cell, dummy_msg, context);
        interpreter
            .stack
            .push(onx_execution::StackValue::Cell(proof_cell.clone()));

        match interpreter.run() {
            ExecutionResult::Success { .. } => Ok(()),
            ExecutionResult::Exception {
                kind: ExceptionKind::AbsentNode,
                ..
            } => Err(ChannelError::InvalidProof),
            ExecutionResult::Exception { .. } => Err(ChannelError::InvalidProof),
        }
    }
}
