//! Message admission rules per `docs/specification/transactions.md` §3.1 and §5.

use crate::error::TransactionsError;
use onx_data_structures::{Message, MessageType};
use onx_primitives::{Uint128, Uint32};
use onx_state_model::AccountType;

/// Maximum gas an External Inbound message's tentative execution may
/// consume before block inclusion, per §3.1 rule 2 and ADR-0005.
pub const MAX_TENTATIVE_GAS: u64 = 10_000;

/// Result of tentatively executing an External Inbound message candidate
/// against the target account's current state, prior to block inclusion,
/// per §3.1 rule 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TentativeExecutionOutcome {
    /// Whether the message's signature verified against the target account.
    pub signature_valid: bool,
    /// Gas consumed evaluating the message under the tentative-execution cap.
    pub gas_used: u64,
}

/// Validates that `extra_currencies` is strictly sorted in ascending order
/// by `currency_id` with no duplicates, per `data-structures.md` §4.3 and
/// `transactions.md` §5 rule 4.
pub fn validate_extra_currencies_sorted(
    extra_currencies: &[(Uint32, Uint128)],
) -> Result<(), TransactionsError> {
    for pair in extra_currencies.windows(2) {
        let (a, _) = &pair[0];
        let (b, _) = &pair[1];
        if a.0 == b.0 {
            return Err(TransactionsError::DuplicateCurrencyId { currency_id: a.0 });
        }
        if a.0 > b.0 {
            return Err(TransactionsError::UnsortedExtraCurrencies);
        }
    }
    Ok(())
}

/// Validates `message`'s value shape against the rules that apply
/// regardless of message type: `extra_currencies` ordering (§5 rule 4), and
/// the External Inbound zero-value requirement (§5 rule 2).
pub fn validate_message_shape(message: &Message) -> Result<(), TransactionsError> {
    validate_extra_currencies_sorted(&message.extra_currencies)?;
    if message.msg_type == MessageType::ExternalInbound
        && (message.amount_nanos.0 != 0 || !message.extra_currencies.is_empty())
    {
        return Err(TransactionsError::NonZeroExternalValue);
    }
    Ok(())
}

/// Admits an Internal message per §3.1 rule 1: it must carry a valid value
/// shape and have been generated during valid execution of a prior
/// message/transaction on an Active source account.
pub fn admit_internal(
    message: &Message,
    source_account_type: AccountType,
) -> Result<(), TransactionsError> {
    if message.msg_type != MessageType::Internal {
        return Err(TransactionsError::WrongMessageType {
            expected: MessageType::Internal,
            actual: message.msg_type,
        });
    }
    validate_message_shape(message)?;
    if source_account_type != AccountType::Active {
        return Err(TransactionsError::InternalSourceNotActive);
    }
    Ok(())
}

/// Admits an External Inbound message ("from nowhere") per §3.1 rule 2: it
/// must carry zero value and its tentative execution must have succeeded
/// within `MAX_TENTATIVE_GAS`.
pub fn admit_external_inbound(
    message: &Message,
    outcome: TentativeExecutionOutcome,
) -> Result<(), TransactionsError> {
    if message.msg_type != MessageType::ExternalInbound {
        return Err(TransactionsError::WrongMessageType {
            expected: MessageType::ExternalInbound,
            actual: message.msg_type,
        });
    }
    validate_message_shape(message)?;
    if !outcome.signature_valid {
        return Err(TransactionsError::TentativeSignatureInvalid);
    }
    if outcome.gas_used > MAX_TENTATIVE_GAS {
        return Err(TransactionsError::TentativeGasLimitExceeded {
            used: outcome.gas_used,
            limit: MAX_TENTATIVE_GAS,
        });
    }
    Ok(())
}

/// Admits an External Outbound message ("to nowhere") per §3.1 rule 3: it is
/// logged in block output but not routed to any recipient account state, so
/// only the type-independent value shape rules apply.
pub fn admit_external_outbound(message: &Message) -> Result<(), TransactionsError> {
    if message.msg_type != MessageType::ExternalOutbound {
        return Err(TransactionsError::WrongMessageType {
            expected: MessageType::ExternalOutbound,
            actual: message.msg_type,
        });
    }
    validate_message_shape(message)
}
