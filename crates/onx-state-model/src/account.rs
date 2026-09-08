use crate::error::StateModelError;
use onx_primitives::{Uint128, Uint256, Uint32, Uint64};

/// Account lifecycle states per `docs/specification/state-model.md` §3.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AccountStateKind {
    Uninitialized = 0x00,
    Active = 0x01,
    Frozen = 0x02,
    Destroyed = 0x03,
}

impl AccountStateKind {
    pub fn from_u8(value: u8) -> Result<Self, StateModelError> {
        match value {
            0x00 => Ok(AccountStateKind::Uninitialized),
            0x01 => Ok(AccountStateKind::Active),
            0x02 => Ok(AccountStateKind::Frozen),
            0x03 => Ok(AccountStateKind::Destroyed),
            _ => Err(StateModelError::InvalidStateTransition(format!(
                "Invalid account state kind byte: {:#04x}",
                value
            ))),
        }
    }

    /// Validates lifecycle state transitions per §3.1 and §5.1.
    pub fn can_transition_to(self, target: Self) -> bool {
        match (self, target) {
            (Self::Uninitialized, Self::Active) => true,
            (Self::Active, Self::Frozen) => true,
            (Self::Frozen, Self::Active) => true,
            (Self::Active, Self::Destroyed) => true,
            (s, t) if s == t => true,
            _ => false,
        }
    }
}

/// Active account state record per `docs/specification/state-model.md` §3.2 and §4.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountState {
    pub kind: AccountStateKind,
    pub balance_nanos: Uint128,
    pub last_trans_lt: Uint64,
    pub code_hash: Uint256,
    pub data_hash: Uint256,
    pub cell_count: Uint32,
    pub byte_count: Uint64,
}

impl AccountState {
    /// Total binary length of active account state record (101 bytes).
    pub const BINARY_SIZE: usize = 1 + 16 + 8 + 32 + 32 + 4 + 8;

    /// Creates a new active account state.
    pub fn new_active(
        balance_nanos: Uint128,
        last_trans_lt: Uint64,
        code_hash: Uint256,
        data_hash: Uint256,
        cell_count: Uint32,
        byte_count: Uint64,
    ) -> Self {
        Self {
            kind: AccountStateKind::Active,
            balance_nanos,
            last_trans_lt,
            code_hash,
            data_hash,
            cell_count,
            byte_count,
        }
    }

    /// Validates an update to logical time (`last_trans_lt`), enforcing strict monotonicity per §5.2.
    pub fn update_logical_time(&mut self, next_lt: Uint64) -> Result<(), StateModelError> {
        if next_lt.0 <= self.last_trans_lt.0 {
            return Err(StateModelError::LogicalTimeRegression {
                prior: self.last_trans_lt.0,
                current: next_lt.0,
            });
        }
        self.last_trans_lt = next_lt;
        Ok(())
    }

    /// Serializes active account state to canonical binary layout (101 bytes) per §4.1.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(Self::BINARY_SIZE);
        buf.push(self.kind as u8);
        buf.extend_from_slice(&self.balance_nanos.encode());
        buf.extend_from_slice(&self.last_trans_lt.encode());
        buf.extend_from_slice(&self.code_hash.encode());
        buf.extend_from_slice(&self.data_hash.encode());
        buf.extend_from_slice(&self.cell_count.encode());
        buf.extend_from_slice(&self.byte_count.encode());
        buf
    }

    /// Deserializes active account state from canonical binary layout per §4.1.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, StateModelError> {
        if bytes.len() != Self::BINARY_SIZE {
            return Err(StateModelError::InvalidStateTransition(format!(
                "Account state binary layout must be exactly {} bytes, got {}",
                Self::BINARY_SIZE,
                bytes.len()
            )));
        }

        let kind = AccountStateKind::from_u8(bytes[0])?;
        if kind != AccountStateKind::Active {
            return Err(StateModelError::InvalidStateTransition(format!(
                "AccountState binary layout is reserved for Active state (0x01), got {:#04x}",
                bytes[0]
            )));
        }

        let mut offset = 1;

        let balance_nanos = Uint128::decode_exact(&bytes[offset..offset + 16])?;
        offset += 16;

        let last_trans_lt = Uint64::decode_exact(&bytes[offset..offset + 8])?;
        offset += 8;

        let code_hash = Uint256::decode_exact(&bytes[offset..offset + 32])?;
        offset += 32;

        let data_hash = Uint256::decode_exact(&bytes[offset..offset + 32])?;
        offset += 32;

        let cell_count = Uint32::decode_exact(&bytes[offset..offset + 4])?;
        offset += 4;

        let byte_count = Uint64::decode_exact(&bytes[offset..offset + 8])?;

        Ok(Self {
            kind,
            balance_nanos,
            last_trans_lt,
            code_hash,
            data_hash,
            cell_count,
            byte_count,
        })
    }
}
