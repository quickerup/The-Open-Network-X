//! ONX Virtual Machine Execution Engine and Interpreter.
//!
//! Implements `docs/specification/execution.md` (ADR-0007) and
//! `docs/specification/tvm-instruction-set.md` (ADR-0017).

pub mod continuation;
pub mod dictionary;
pub mod interpreter;
pub mod types;

pub use continuation::{Continuation, ControlRegisters};
pub use dictionary::{Dictionary, DictionaryError};
pub use interpreter::Interpreter;
pub use types::{Builder, ExceptionKind, ExecutionContext, ExecutionResult, Slice, StackValue};

use onx_data_structures::Message;
use onx_state_model::Cell;

/// Executes contract code against cell data, inbound message, and execution context.
pub fn execute(
    code: Cell,
    data: Cell,
    message: Message,
    context: ExecutionContext,
) -> ExecutionResult {
    let mut interpreter = Interpreter::new(code, data, message, context);
    interpreter.run()
}
