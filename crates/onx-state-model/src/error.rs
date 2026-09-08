use onx_data_structures::DataStructureError;
use onx_primitives::PrimitiveError;

#[derive(Debug, PartialEq, Eq)]
pub enum StateModelError {
    InvalidStateTransition(String),
    LogicalTimeRegression { prior: u64, current: u64 },
    BalanceUnderflow,
    InvalidCellDataLength(usize),
    InvalidCellRefCount(usize),
    CyclicCellReference,
    StateRootMismatch,
    MalformedMerkleProof(String),
    Primitive(PrimitiveError),
    DataStructure(DataStructureError),
}

impl From<PrimitiveError> for StateModelError {
    fn from(err: PrimitiveError) -> Self {
        StateModelError::Primitive(err)
    }
}

impl From<DataStructureError> for StateModelError {
    fn from(err: DataStructureError) -> Self {
        StateModelError::DataStructure(err)
    }
}

impl std::fmt::Display for StateModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateModelError::InvalidStateTransition(msg) => {
                write!(f, "Invalid state transition: {}", msg)
            }
            StateModelError::LogicalTimeRegression { prior, current } => {
                write!(
                    f,
                    "Logical time regression: current {} <= prior {}",
                    current, prior
                )
            }
            StateModelError::BalanceUnderflow => write!(f, "Balance underflow"),
            StateModelError::InvalidCellDataLength(len) => {
                write!(f, "Invalid cell data length: {} (max 128)", len)
            }
            StateModelError::InvalidCellRefCount(refs) => {
                write!(f, "Invalid cell ref count: {} (max 4)", refs)
            }
            StateModelError::CyclicCellReference => {
                write!(f, "Cyclic cell reference detected in BoC graph")
            }
            StateModelError::StateRootMismatch => write!(f, "State root hash mismatch"),
            StateModelError::MalformedMerkleProof(msg) => {
                write!(f, "Malformed Merkle proof: {}", msg)
            }
            StateModelError::Primitive(err) => write!(f, "Primitive error: {:?}", err),
            StateModelError::DataStructure(err) => write!(f, "Data structure error: {:?}", err),
        }
    }
}

impl std::error::Error for StateModelError {}
