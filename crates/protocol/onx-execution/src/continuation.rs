//! TVM control-register continuations.
//!
//! A continuation is deliberately kept outside the operand stack: it identifies
//! an instruction position and is installed in one of the control registers.
use onx_state_model::Cell;

/// Resumption point for execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Continuation {
    pub code: Cell,
    pub pc_bits: usize,
}

impl Continuation {
    pub fn new(code: Cell, pc_bits: usize) -> Self {
        Self { code, pc_bits }
    }
}

/// The control registers used by this interpreter.
///
/// `c0` is the normal subroutine return, `c1` is an alternate return, and
/// `c2` is entered when an instruction raises an exception.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlRegisters {
    pub c0: Option<Continuation>,
    pub c1: Option<Continuation>,
    pub c2: Option<Continuation>,
}

impl ControlRegisters {
    pub fn set_c0(&mut self, continuation: Continuation) {
        self.c0 = Some(continuation);
    }
    pub fn set_c1(&mut self, continuation: Continuation) {
        self.c1 = Some(continuation);
    }
    pub fn set_c2(&mut self, continuation: Continuation) {
        self.c2 = Some(continuation);
    }
    pub fn take_c0(&mut self) -> Option<Continuation> {
        self.c0.take()
    }
    pub fn take_c1(&mut self) -> Option<Continuation> {
        self.c1.take()
    }
    pub fn c2(&self) -> Option<Continuation> {
        self.c2.clone()
    }

    /// Takes the normal (`c0`) or alternative (`c1`) return continuation.
    pub fn take_return(&mut self, alternative: bool) -> Option<Continuation> {
        if alternative {
            self.take_c1()
        } else {
            self.take_c0()
        }
    }
}
