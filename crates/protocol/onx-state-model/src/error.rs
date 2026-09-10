use std::fmt;

/// Errors arising in state model calculations, encodings, transitions, or proof verifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateModelError {
    /// Descriptor or layout overflow / mismatch.
    InvalidDescriptor,
    /// Cell data byte count exceeds 128 bytes.
    DataTooLarge { length: usize, max: usize },
    /// Cell child reference count exceeds 4.
    TooManyReferences { count: usize, max: usize },
    /// Truncated or unexpected byte slice during deserialization.
    DeserializationError(String),
    /// Trailing unused bytes after deserialization.
    TrailingBytes { remaining: usize },
    /// Invalid state type code.
    InvalidStateType(u8),
    /// Account state transition violation (e.g. invalid lifecycle progression or operating on destroyed account).
    InvalidStateTransition(String),
    /// Logical time regression (new last_trans_lt <= old last_trans_lt).
    LogicalTimeRegression { current: u64, next: u64 },
    /// Balance underflow.
    BalanceUnderflow,
    /// Cycle detected in Bag-of-Cells graph.
    CyclicCellReference,
    /// State root hash mismatch during block state verification.
    StateRootMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
    /// Merkle proof verification failure.
    InvalidMerkleProof(String),
}

impl fmt::Display for StateModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDescriptor => write!(f, "Invalid cell descriptor bytes"),
            Self::DataTooLarge { length, max } => {
                write!(
                    f,
                    "Cell data length {} exceeds maximum allowed {}",
                    length, max
                )
            }
            Self::TooManyReferences { count, max } => {
                write!(
                    f,
                    "Cell reference count {} exceeds maximum allowed {}",
                    count, max
                )
            }
            Self::DeserializationError(msg) => write!(f, "Deserialization error: {}", msg),
            Self::TrailingBytes { remaining } => {
                write!(
                    f,
                    "Trailing bytes remaining after deserialization: {}",
                    remaining
                )
            }
            Self::InvalidStateType(code) => {
                write!(f, "Invalid account state type u8: {:#04x}", code)
            }
            Self::InvalidStateTransition(msg) => write!(f, "Invalid state transition: {}", msg),
            Self::LogicalTimeRegression { current, next } => write!(
                f,
                "Logical time regression: new lt {} <= current lt {}",
                next, current
            ),
            Self::BalanceUnderflow => write!(f, "Balance underflow: balance cannot be negative"),
            Self::CyclicCellReference => {
                write!(f, "Cyclic cell reference detected in Bag-of-Cells")
            }
            Self::StateRootMismatch { expected, actual } => write!(
                f,
                "State root mismatch: expected {:?}, actual {:?}",
                expected, actual
            ),
            Self::InvalidMerkleProof(msg) => write!(f, "Invalid Merkle proof: {}", msg),
        }
    }
}

impl std::error::Error for StateModelError {}
