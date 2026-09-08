use crate::error::StateModelError;
use onx_primitives::{
    integers::{Uint128, Uint32, Uint64},
    PrimitiveError,
};

/// Canonical account lifecycle states per docs/specification/state-model.md §3.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AccountStatus {
    Uninitialized = 0x00,
    Active = 0x01,
    Frozen = 0x02,
    Destroyed = 0x03,
}

impl AccountStatus {
    pub fn from_u8(value: u8) -> Result<Self, StateModelError> {
        match value {
            0x00 => Ok(Self::Uninitialized),
            0x01 => Ok(Self::Active),
            0x02 => Ok(Self::Frozen),
            0x03 => Ok(Self::Destroyed),
            _ => Err(StateModelError::SerializationError(format!(
                "invalid account status tag: {value:#04x}"
            ))),
        }
    }
}

/// Active account state record layout per docs/specification/state-model.md §4.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountStateRecord {
    pub balance_nanos: u128,
    pub last_trans_lt: u64,
    pub code_hash: [u8; 32],
    pub data_hash: [u8; 32],
    pub cell_count: u32,
    pub byte_count: u64,
}

impl AccountStateRecord {
    /// Serializes the account state record into canonical binary form (101 bytes).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(101);
        buf.push(AccountStatus::Active as u8); // state_type = 0x01
        buf.extend_from_slice(&Uint128::from(self.balance_nanos).encode());
        buf.extend_from_slice(&Uint64::from(self.last_trans_lt).encode());
        buf.extend_from_slice(&self.code_hash);
        buf.extend_from_slice(&self.data_hash);
        buf.extend_from_slice(&Uint32::from(self.cell_count).encode());
        buf.extend_from_slice(&Uint64::from(self.byte_count).encode());
        buf
    }

    /// Deserializes an Active account state record from bytes.
    pub fn from_bytes(slice: &[u8]) -> Result<Self, StateModelError> {
        if slice.len() != 101 {
            return Err(StateModelError::SerializationError(format!(
                "account state record length mismatch: expected 101 bytes, got {}",
                slice.len()
            )));
        }

        let status = AccountStatus::from_u8(slice[0])?;
        if status != AccountStatus::Active {
            return Err(StateModelError::SerializationError(format!(
                "expected active state status 0x01, got {:#04x}",
                slice[0]
            )));
        }

        let balance_nanos = Uint128::decode_exact(&slice[1..17])
            .map_err(|e: PrimitiveError| StateModelError::SerializationError(e.to_string()))?
            .0;

        let last_trans_lt = Uint64::decode_exact(&slice[17..25])
            .map_err(|e: PrimitiveError| StateModelError::SerializationError(e.to_string()))?
            .0;

        let mut code_hash = [0u8; 32];
        code_hash.copy_from_slice(&slice[25..57]);

        let mut data_hash = [0u8; 32];
        data_hash.copy_from_slice(&slice[57..89]);

        let cell_count = Uint32::decode_exact(&slice[89..93])
            .map_err(|e: PrimitiveError| StateModelError::SerializationError(e.to_string()))?
            .0;

        let byte_count = Uint64::decode_exact(&slice[93..101])
            .map_err(|e: PrimitiveError| StateModelError::SerializationError(e.to_string()))?
            .0;

        Ok(Self {
            balance_nanos,
            last_trans_lt,
            code_hash,
            data_hash,
            cell_count,
            byte_count,
        })
    }
}

/// Full account state representation covering all lifecycle states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountState {
    Uninitialized,
    Active(AccountStateRecord),
    Frozen { storage_hash: [u8; 32] },
    Destroyed,
}

impl AccountState {
    pub fn status(&self) -> AccountStatus {
        match self {
            Self::Uninitialized => AccountStatus::Uninitialized,
            Self::Active(_) => AccountStatus::Active,
            Self::Frozen { .. } => AccountStatus::Frozen,
            Self::Destroyed => AccountStatus::Destroyed,
        }
    }

    /// Validates a state transition from `self` to `next` given transaction parameters.
    pub fn transition_to(
        &self,
        next_record: AccountStateRecord,
    ) -> Result<AccountState, StateModelError> {
        match self {
            Self::Destroyed => Err(StateModelError::InvalidStateTransition(
                "cannot perform transactions on a destroyed account".to_string(),
            )),
            Self::Frozen { .. } => Err(StateModelError::InvalidStateTransition(
                "cannot execute state transition directly on a frozen account without unfreezing"
                    .to_string(),
            )),
            Self::Uninitialized => {
                // Must start with valid initial balance/lt
                Ok(AccountState::Active(next_record))
            }
            Self::Active(current) => {
                if next_record.last_trans_lt <= current.last_trans_lt {
                    return Err(StateModelError::LogicalTimeRegression {
                        prior: current.last_trans_lt,
                        attempted: next_record.last_trans_lt,
                    });
                }
                Ok(AccountState::Active(next_record))
            }
        }
    }

    /// Deducts fees or funds from balance, enforcing no balance underflow.
    pub fn deduct_balance(&mut self, amount: u128) -> Result<(), StateModelError> {
        match self {
            Self::Active(ref mut record) => {
                if record.balance_nanos < amount {
                    return Err(StateModelError::BalanceUnderflow {
                        available: record.balance_nanos,
                        required: amount,
                    });
                }
                record.balance_nanos -= amount;
                Ok(())
            }
            _ => Err(StateModelError::InvalidStateTransition(
                "cannot deduct balance from non-active account".to_string(),
            )),
        }
    }

    /// Adds funds to balance.
    pub fn credit_balance(&mut self, amount: u128) -> Result<(), StateModelError> {
        match self {
            Self::Active(ref mut record) => {
                record.balance_nanos = record.balance_nanos.saturating_add(amount);
                Ok(())
            }
            Self::Uninitialized => {
                // Balance received on uninitialized account retains balance when activated
                Ok(())
            }
            _ => Err(StateModelError::InvalidStateTransition(
                "cannot credit balance to frozen/destroyed account directly".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_state_record_round_trip() {
        let rec = AccountStateRecord {
            balance_nanos: 1_000_000_000,
            last_trans_lt: 1005,
            code_hash: [0x11; 32],
            data_hash: [0x22; 32],
            cell_count: 5,
            byte_count: 250,
        };

        let bytes = rec.to_bytes();
        assert_eq!(bytes.len(), 101);
        assert_eq!(bytes[0], 0x01);

        let decoded = AccountStateRecord::from_bytes(&bytes).unwrap();
        assert_eq!(rec, decoded);
    }

    #[test]
    fn test_logical_time_regression_rejection() {
        let initial = AccountStateRecord {
            balance_nanos: 500,
            last_trans_lt: 100,
            code_hash: [0x01; 32],
            data_hash: [0x02; 32],
            cell_count: 1,
            byte_count: 10,
        };

        let active_state = AccountState::Active(initial.clone());

        let mut regressed = initial.clone();
        regressed.last_trans_lt = 100; // Same LT

        let res = active_state.transition_to(regressed);
        assert!(matches!(
            res,
            Err(StateModelError::LogicalTimeRegression {
                prior: 100,
                attempted: 100
            })
        ));
    }

    #[test]
    fn test_balance_underflow() {
        let mut active_state = AccountState::Active(AccountStateRecord {
            balance_nanos: 50,
            last_trans_lt: 10,
            code_hash: [0; 32],
            data_hash: [0; 32],
            cell_count: 1,
            byte_count: 10,
        });

        assert!(matches!(
            active_state.deduct_balance(100),
            Err(StateModelError::BalanceUnderflow {
                available: 50,
                required: 100
            })
        ));
    }
}
