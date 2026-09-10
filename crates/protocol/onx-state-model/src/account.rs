use crate::error::StateModelError;
use onx_primitives::{Uint128, Uint32, Uint64, Uint8};

/// Canonical account lifecycle states per docs/specification/state-model.md §3.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AccountType {
    Uninitialized = 0x00,
    Active = 0x01,
    Frozen = 0x02,
    Destroyed = 0x03,
}

impl AccountType {
    pub fn from_u8(value: u8) -> Result<Self, StateModelError> {
        match value {
            0x00 => Ok(Self::Uninitialized),
            0x01 => Ok(Self::Active),
            0x02 => Ok(Self::Frozen),
            0x03 => Ok(Self::Destroyed),
            other => Err(StateModelError::InvalidStateType(other)),
        }
    }

    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

/// Storage resource consumption statistics for an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageStat {
    pub cell_count: u32,
    pub byte_count: u64,
}

/// Canonical Account State record per docs/specification/state-model.md §3.2 and §4.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountState {
    Uninitialized,
    Active {
        balance_nanos: u128,
        last_trans_lt: u64,
        code_hash: [u8; 32],
        data_hash: [u8; 32],
        storage_stat: StorageStat,
    },
    Frozen {
        balance_nanos: u128,
        last_trans_lt: u64,
        storage_hash: [u8; 32],
    },
    Destroyed,
}

impl AccountState {
    pub fn account_type(&self) -> AccountType {
        match self {
            Self::Uninitialized => AccountType::Uninitialized,
            Self::Active { .. } => AccountType::Active,
            Self::Frozen { .. } => AccountType::Frozen,
            Self::Destroyed => AccountType::Destroyed,
        }
    }

    /// Returns the account balance in nanocoins, or 0 if uninitialized or destroyed.
    pub fn balance_nanos(&self) -> u128 {
        match self {
            Self::Active { balance_nanos, .. } | Self::Frozen { balance_nanos, .. } => {
                *balance_nanos
            }
            Self::Uninitialized | Self::Destroyed => 0,
        }
    }

    /// Serializes an active account state record according to docs/specification/state-model.md §4.1.
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::Uninitialized => vec![AccountType::Uninitialized.to_u8()],
            Self::Active {
                balance_nanos,
                last_trans_lt,
                code_hash,
                data_hash,
                storage_stat,
            } => {
                let mut bytes = Vec::with_capacity(105);
                bytes.push(AccountType::Active.to_u8());
                bytes.extend_from_slice(&Uint128(*balance_nanos).encode());
                bytes.extend_from_slice(&Uint64(*last_trans_lt).encode());
                bytes.extend_from_slice(code_hash);
                bytes.extend_from_slice(data_hash);
                bytes.extend_from_slice(&Uint32(storage_stat.cell_count).encode());
                bytes.extend_from_slice(&Uint64(storage_stat.byte_count).encode());
                bytes
            }
            Self::Frozen {
                balance_nanos,
                last_trans_lt,
                storage_hash,
            } => {
                let mut bytes = Vec::with_capacity(57);
                bytes.push(AccountType::Frozen.to_u8());
                bytes.extend_from_slice(&Uint128(*balance_nanos).encode());
                bytes.extend_from_slice(&Uint64(*last_trans_lt).encode());
                bytes.extend_from_slice(storage_hash);
                bytes
            }
            Self::Destroyed => vec![AccountType::Destroyed.to_u8()],
        }
    }

    /// Deserializes an account state record from binary bytes.
    pub fn from_bytes(slice: &[u8]) -> Result<(Self, usize), StateModelError> {
        if slice.is_empty() {
            return Err(StateModelError::DeserializationError(
                "Empty byte slice for AccountState".to_string(),
            ));
        }

        let mut cursor = slice;
        let state_type_val = Uint8::read(&mut cursor)
            .map_err(|e| StateModelError::DeserializationError(e.to_string()))?;
        let state_type = AccountType::from_u8(state_type_val.0)?;
        let mut offset = Uint8::BYTE_LEN;

        match state_type {
            AccountType::Uninitialized => Ok((Self::Uninitialized, offset)),
            AccountType::Destroyed => Ok((Self::Destroyed, offset)),
            AccountType::Frozen => {
                let balance_val = Uint128::read(&mut cursor)
                    .map_err(|e| StateModelError::DeserializationError(e.to_string()))?;
                offset += Uint128::BYTE_LEN;

                let lt_val = Uint64::read(&mut cursor)
                    .map_err(|e| StateModelError::DeserializationError(e.to_string()))?;
                offset += Uint64::BYTE_LEN;

                if cursor.len() < 32 {
                    return Err(StateModelError::DeserializationError(
                        "Truncated Frozen AccountState storage hash".to_string(),
                    ));
                }

                let mut storage_hash = [0u8; 32];
                storage_hash.copy_from_slice(&cursor[..32]);
                offset += 32;

                Ok((
                    Self::Frozen {
                        balance_nanos: balance_val.0,
                        last_trans_lt: lt_val.0,
                        storage_hash,
                    },
                    offset,
                ))
            }
            AccountType::Active => {
                let balance_val = Uint128::read(&mut cursor)
                    .map_err(|e| StateModelError::DeserializationError(e.to_string()))?;
                offset += Uint128::BYTE_LEN;

                let lt_val = Uint64::read(&mut cursor)
                    .map_err(|e| StateModelError::DeserializationError(e.to_string()))?;
                offset += Uint64::BYTE_LEN;

                if cursor.len() < 64 {
                    return Err(StateModelError::DeserializationError(
                        "Truncated Active AccountState hashes".to_string(),
                    ));
                }

                let mut code_hash = [0u8; 32];
                code_hash.copy_from_slice(&cursor[..32]);
                cursor = &cursor[32..];
                offset += 32;

                let mut data_hash = [0u8; 32];
                data_hash.copy_from_slice(&cursor[..32]);
                cursor = &cursor[32..];
                offset += 32;

                let cell_count_val = Uint32::read(&mut cursor)
                    .map_err(|e| StateModelError::DeserializationError(e.to_string()))?;
                offset += Uint32::BYTE_LEN;

                let byte_count_val = Uint64::read(&mut cursor)
                    .map_err(|e| StateModelError::DeserializationError(e.to_string()))?;
                offset += Uint64::BYTE_LEN;

                Ok((
                    Self::Active {
                        balance_nanos: balance_val.0,
                        last_trans_lt: lt_val.0,
                        code_hash,
                        data_hash,
                        storage_stat: StorageStat {
                            cell_count: cell_count_val.0,
                            byte_count: byte_count_val.0,
                        },
                    },
                    offset,
                ))
            }
        }
    }

    /// Validates a proposed state transition from `self` to `next` with transaction logical time and balance checks.
    pub fn validate_transition(
        &self,
        next: &AccountState,
        new_lt: u64,
    ) -> Result<(), StateModelError> {
        match (self, next) {
            (Self::Destroyed, _) => Err(StateModelError::InvalidStateTransition(
                "Cannot perform transition on a Destroyed account".to_string(),
            )),
            (Self::Uninitialized, Self::Active { last_trans_lt, .. }) => {
                if *last_trans_lt != new_lt {
                    return Err(StateModelError::LogicalTimeRegression {
                        current: *last_trans_lt,
                        next: new_lt,
                    });
                }
                Ok(())
            }
            (Self::Uninitialized, Self::Uninitialized) => Ok(()),
            (Self::Uninitialized, _) => Err(StateModelError::InvalidStateTransition(
                "Uninitialized account can only transition to Active or remain Uninitialized"
                    .to_string(),
            )),
            (
                Self::Active {
                    last_trans_lt: cur_lt,
                    ..
                },
                Self::Active {
                    last_trans_lt: next_lt,
                    ..
                },
            )
            | (
                Self::Active {
                    last_trans_lt: cur_lt,
                    ..
                },
                Self::Frozen {
                    last_trans_lt: next_lt,
                    ..
                },
            ) => {
                if new_lt <= *cur_lt || *next_lt != new_lt {
                    return Err(StateModelError::LogicalTimeRegression {
                        current: *cur_lt,
                        next: new_lt,
                    });
                }
                Ok(())
            }
            (Self::Active { .. }, Self::Destroyed) => Ok(()),
            (
                Self::Frozen {
                    last_trans_lt: cur_lt,
                    ..
                },
                Self::Active {
                    last_trans_lt: next_lt,
                    ..
                },
            ) => {
                if new_lt <= *cur_lt || *next_lt != new_lt {
                    return Err(StateModelError::LogicalTimeRegression {
                        current: *cur_lt,
                        next: new_lt,
                    });
                }
                Ok(())
            }
            (
                Self::Frozen {
                    last_trans_lt: cur_lt,
                    ..
                },
                Self::Frozen {
                    last_trans_lt: next_lt,
                    ..
                },
            ) => {
                if new_lt <= *cur_lt || *next_lt != new_lt {
                    return Err(StateModelError::LogicalTimeRegression {
                        current: *cur_lt,
                        next: new_lt,
                    });
                }
                Ok(())
            }
            (Self::Frozen { .. }, Self::Destroyed) => Ok(()),
            _ => Err(StateModelError::InvalidStateTransition(format!(
                "Invalid state transition from {:?} to {:?}",
                self.account_type(),
                next.account_type()
            ))),
        }
    }

    /// Validates a proposed state transition from `self` to `next` with logical time,
    /// lifecycle rules, and balance delta checks. Returns `StateModelError::BalanceUnderflow`
    /// if `balance_delta` causes the resulting account balance to fall below zero.
    pub fn validate_transition_with_delta(
        &self,
        next: &AccountState,
        new_lt: u64,
        balance_delta: i128,
    ) -> Result<(), StateModelError> {
        self.validate_transition(next, new_lt)?;

        let cur_balance = self.balance_nanos();
        if balance_delta < 0 && balance_delta.unsigned_abs() > cur_balance {
            return Err(StateModelError::BalanceUnderflow);
        }

        let expected_next_balance = if balance_delta >= 0 {
            cur_balance.saturating_add(balance_delta as u128)
        } else {
            cur_balance - balance_delta.unsigned_abs()
        };

        if let Self::Active { balance_nanos, .. } | Self::Frozen { balance_nanos, .. } = next {
            if *balance_nanos != expected_next_balance {
                return Err(StateModelError::InvalidStateTransition(format!(
                    "Expected next state balance {}, found {}",
                    expected_next_balance, balance_nanos
                )));
            }
        }

        Ok(())
    }
}
