use core::fmt;

/// Errors produced by the ONX state model operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateModelError {
    /// Attempted invalid state transition or operation on an inactive account.
    InvalidStateTransition(String),
    /// Account balance underflow.
    BalanceUnderflow { available: u128, required: u128 },
    /// Logical time regression (new LT <= prior LT).
    LogicalTimeRegression { prior: u64, attempted: u64 },
    /// Malformed cell representation (e.g. data > 128 bytes, > 4 refs).
    MalformedCell(String),
    /// Cyclic cell reference in Bag-of-Cells graph.
    CyclicCellReference,
    /// Malformed Bag-of-Cells graph or serialization.
    MalformedBagOfCells(String),
    /// State root hash mismatch.
    StateRootMismatch {
        expected: [u8; 32],
        calculated: [u8; 32],
    },
    /// Malformed or invalid Merkle proof.
    InvalidMerkleProof(String),
    /// Buffer underflow or trailing bytes during deserialization.
    SerializationError(String),
}

impl fmt::Display for StateModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStateTransition(msg) => write!(f, "invalid state transition: {msg}"),
            Self::BalanceUnderflow {
                available,
                required,
            } => {
                write!(
                    f,
                    "balance underflow: available {available}, required {required}"
                )
            }
            Self::LogicalTimeRegression { prior, attempted } => {
                write!(
                    f,
                    "logical time regression: prior {prior}, attempted {attempted}"
                )
            }
            Self::MalformedCell(msg) => write!(f, "malformed cell: {msg}"),
            Self::CyclicCellReference => write!(f, "cyclic cell reference in cell graph"),
            Self::MalformedBagOfCells(msg) => write!(f, "malformed bag of cells: {msg}"),
            Self::StateRootMismatch {
                expected,
                calculated,
            } => write!(
                f,
                "state root mismatch: expected {}, calculated {}",
                hex::encode(expected),
                hex::encode(calculated)
            ),
            Self::InvalidMerkleProof(msg) => write!(f, "invalid merkle proof: {msg}"),
            Self::SerializationError(msg) => write!(f, "serialization error: {msg}"),
        }
    }
}

impl std::error::Error for StateModelError {}
