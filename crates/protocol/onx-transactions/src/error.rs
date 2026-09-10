//! Error types for transaction and message admission, output-queue
//! ordering, and double-delivery prevention.

use onx_data_structures::MessageType;
use std::fmt;

/// Errors arising from transaction/message admission, output-queue
/// ordering, or double-delivery prevention per
/// `docs/specification/transactions.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionsError {
    /// A message was presented to an admission handler for the wrong `msg_type`.
    WrongMessageType {
        expected: MessageType,
        actual: MessageType,
    },
    /// An External Inbound message carries non-zero value (§5 rule 2).
    NonZeroExternalValue,
    /// `extra_currencies` pairs are not in strictly ascending `currency_id` order (§5 rule 4).
    UnsortedExtraCurrencies,
    /// `extra_currencies` contains a duplicate `currency_id` (§5 rule 4).
    DuplicateCurrencyId { currency_id: u32 },
    /// Tentative execution of an External Inbound message failed signature verification (§5 rule 3).
    TentativeSignatureInvalid,
    /// Tentative execution exceeded `MAX_TENTATIVE_GAS` (§5 rule 3).
    TentativeGasLimitExceeded { used: u64, limit: u64 },
    /// An Internal message's source account is not Active (§3.1 rule 1).
    InternalSourceNotActive,
    /// An inbound message's representation hash is already present in `processed_msg_hashes` (§5 rule 5).
    DuplicateDelivery,
    /// A delivered sequence violated strict per-(sender, recipient) FIFO ordering (§5 rule 6).
    FifoOrderViolation { previous_lt: u64, next_lt: u64 },
    /// Enqueuing into an output-queue lane would regress or repeat that lane's logical time (§3.3 rule 2).
    LogicalTimeRegression { previous_lt: u64, next_lt: u64 },
    /// A routing request crossed workchains or used shards at different depths.
    IncompatibleRoute,
    /// A routing request's message endpoint does not belong to the supplied shard.
    RouteEndpointMismatch,
    /// A proposed transit hop is not a one-bit hypercube neighbor.
    InvalidHypercubeHop,
    /// A message cannot pay the deterministic forwarding fees for its route.
    InsufficientTransitFee { available: u128, required: u128 },
    /// Cross-workchain message expiry calculation overflowed logical time.
    ExpiryOverflow,
}

impl fmt::Display for TransactionsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongMessageType { expected, actual } => write!(
                f,
                "message admitted via wrong handler: expected {:?}, got {:?}",
                expected, actual
            ),
            Self::NonZeroExternalValue => {
                write!(f, "External Inbound message carries non-zero value")
            }
            Self::UnsortedExtraCurrencies => write!(
                f,
                "extra_currencies pairs are not in strictly ascending currency_id order"
            ),
            Self::DuplicateCurrencyId { currency_id } => write!(
                f,
                "extra_currencies contains duplicate currency_id {}",
                currency_id
            ),
            Self::TentativeSignatureInvalid => write!(
                f,
                "External Inbound message failed tentative signature verification"
            ),
            Self::TentativeGasLimitExceeded { used, limit } => write!(
                f,
                "tentative execution gas {} exceeds MAX_TENTATIVE_GAS {}",
                used, limit
            ),
            Self::InternalSourceNotActive => {
                write!(f, "Internal message source account is not Active")
            }
            Self::DuplicateDelivery => write!(
                f,
                "message representation hash already present in processed_msg_hashes"
            ),
            Self::FifoOrderViolation {
                previous_lt,
                next_lt,
            } => write!(
                f,
                "FIFO order violation: lt {} did not strictly follow lt {}",
                next_lt, previous_lt
            ),
            Self::LogicalTimeRegression {
                previous_lt,
                next_lt,
            } => write!(
                f,
                "logical time regression in output-queue lane: lt {} <= previous lt {}",
                next_lt, previous_lt
            ),
            Self::IncompatibleRoute => write!(
                f,
                "hypercube route requires equal-depth shards in one workchain"
            ),
            Self::RouteEndpointMismatch => {
                write!(f, "message endpoint does not belong to its routing shard")
            }
            Self::InvalidHypercubeHop => {
                write!(f, "route contains a non-neighboring hypercube hop")
            }
            Self::InsufficientTransitFee {
                available,
                required,
            } => write!(
                f,
                "message value {} is insufficient for transit fee {}",
                available, required
            ),
            Self::ExpiryOverflow => {
                write!(f, "cross-workchain message expiry overflows logical time")
            }
        }
    }
}

impl std::error::Error for TransactionsError {}
