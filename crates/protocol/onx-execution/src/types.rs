use onx_data_structures::Message;
use onx_state_model::{Cell, StateModelError};
use std::fmt;

/// Closed exception set per docs/specification/execution.md §3.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExceptionKind {
    /// Gas limit reached.
    OutOfGas,
    /// Unsigned/signed arithmetic or conversion overflow or division by zero.
    IntegerOverflow,
    /// Access to pruned Merkle-proof cell content (CTOS).
    AbsentNode,
    /// Structural cell violation or bytecode decode/stack violation.
    MalformedCell,
    /// Operand stack type mismatch or signature length mismatch.
    TypeMismatch,
}

impl fmt::Display for ExceptionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfGas => write!(f, "OutOfGas"),
            Self::IntegerOverflow => write!(f, "IntegerOverflow"),
            Self::AbsentNode => write!(f, "AbsentNode"),
            Self::MalformedCell => write!(f, "MalformedCell"),
            Self::TypeMismatch => write!(f, "TypeMismatch"),
        }
    }
}

impl std::error::Error for ExceptionKind {}

impl From<StateModelError> for ExceptionKind {
    fn from(_err: StateModelError) -> Self {
        ExceptionKind::MalformedCell
    }
}

/// Execution environment context per docs/specification/execution.md §3.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionContext {
    pub gen_utime: u32,
    pub start_lt: u64,
    pub end_lt: u64,
    pub gas_limit: u64,
}

/// A read cursor over a Cell's data bytes and child cell references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slice {
    pub cell: Cell,
    pub bit_offset: usize,
    pub ref_offset: usize,
    pub child_cells: Vec<Cell>,
}

impl Slice {
    pub fn new(cell: Cell) -> Self {
        Self {
            cell,
            bit_offset: 0,
            ref_offset: 0,
            child_cells: Vec::new(),
        }
    }

    pub fn new_with_children(cell: Cell, child_cells: Vec<Cell>) -> Self {
        Self {
            cell,
            bit_offset: 0,
            ref_offset: 0,
            child_cells,
        }
    }

    pub fn remaining_bits(&self) -> usize {
        let total_bits = self.cell.data_bytes().len() * 8;
        total_bits.saturating_sub(self.bit_offset)
    }

    pub fn remaining_refs(&self) -> usize {
        self.cell.cell_refs().len().saturating_sub(self.ref_offset)
    }

    /// Reads up to 256 bits as a 32-byte big-endian slice.
    pub fn read_bits(&mut self, width_bits: usize) -> Result<[u8; 32], ExceptionKind> {
        if self.remaining_bits() < width_bits || width_bits > 256 {
            return Err(ExceptionKind::MalformedCell);
        }

        let mut res = [0u8; 32];
        let data = self.cell.data_bytes();

        for i in 0..width_bits {
            let src_bit_idx = self.bit_offset + i;
            let src_byte_idx = src_bit_idx / 8;
            let src_bit_in_byte = 7 - (src_bit_idx % 8);
            let bit_val = (data[src_byte_idx] >> src_bit_in_byte) & 1;

            let dest_bit_idx = (256 - width_bits) + i;
            let dest_byte_idx = dest_bit_idx / 8;
            let dest_bit_in_byte = 7 - (dest_bit_idx % 8);

            if bit_val == 1 {
                res[dest_byte_idx] |= 1 << dest_bit_in_byte;
            }
        }

        self.bit_offset += width_bits;
        Ok(res)
    }
}

/// A write accumulator for constructing new Cells.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Builder {
    pub data_bytes: Vec<u8>,
    pub current_bit_len: usize,
    pub references: Vec<Cell>,
}

impl Builder {
    /// Appends the lower `width_bits` from a 32-byte big-endian value.
    pub fn append_bits(
        &mut self,
        val_bytes: &[u8; 32],
        width_bits: usize,
    ) -> Result<(), ExceptionKind> {
        if width_bits > 256 {
            return Err(ExceptionKind::MalformedCell);
        }
        let target_total_bits = self.current_bit_len + width_bits;
        if target_total_bits > 128 * 8 {
            return Err(ExceptionKind::MalformedCell);
        }

        for i in 0..width_bits {
            let src_bit_idx = (256 - width_bits) + i;
            let src_byte_idx = src_bit_idx / 8;
            let src_bit_in_byte = 7 - (src_bit_idx % 8);
            let bit_val = (val_bytes[src_byte_idx] >> src_bit_in_byte) & 1;

            let dest_bit_idx = self.current_bit_len + i;
            let dest_byte_idx = dest_bit_idx / 8;
            let dest_bit_in_byte = 7 - (dest_bit_idx % 8);

            if dest_byte_idx >= self.data_bytes.len() {
                self.data_bytes.push(0);
            }

            if bit_val == 1 {
                self.data_bytes[dest_byte_idx] |= 1 << dest_bit_in_byte;
            }
        }

        self.current_bit_len = target_total_bits;
        Ok(())
    }
}

/// Stack values over the five TVM kinds per docs/specification/tvm-instruction-set.md §3.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackValue {
    /// 256-bit big-endian integer representation.
    Integer([u8; 32]),
    Bytes(Vec<u8>),
    Cell(Cell),
    Slice(Slice),
    Builder(Builder),
}

impl StackValue {
    pub fn from_i128(val: i128) -> Self {
        let mut bytes = [0u8; 32];
        if val < 0 {
            bytes[..16].fill(0xFF);
        }
        bytes[16..32].copy_from_slice(&val.to_be_bytes());
        StackValue::Integer(bytes)
    }

    pub fn to_i128(&self) -> Result<i128, ExceptionKind> {
        match self {
            StackValue::Integer(bytes) => {
                let mut arr = [0u8; 16];
                arr.copy_from_slice(&bytes[16..32]);
                Ok(i128::from_be_bytes(arr))
            }
            _ => Err(ExceptionKind::TypeMismatch),
        }
    }
}

/// Successful or exceptional execution result per docs/specification/execution.md §3.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionResult {
    Success {
        new_data: Cell,
        out_messages: Vec<Message>,
        gas_used: u64,
    },
    Exception {
        kind: ExceptionKind,
        gas_used: u64,
    },
}
