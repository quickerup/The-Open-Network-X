use crate::error::StateModelError;
use onx_primitives::{domain_hash, DomainTag, Uint256};
use std::collections::HashSet;

/// Maximum payload data byte length in a standard Cell per `docs/specification/state-model.md` §3.3.
pub const CELL_MAX_DATA_BYTES: usize = 128;

/// Maximum child reference count in a standard Cell per `docs/specification/state-model.md` §3.3.
pub const CELL_MAX_REFS: usize = 4;

/// Domain separation tag for Cell representation hashing (`ONX_CELL_HASH_V1`).
pub const CELL_HASH_DOMAIN_TAG: DomainTag = DomainTag::from_ascii("ONX_CELL_HASH_V1");

/// 32-byte domain-separated SHA-256 Cell representation hash.
pub type CellHash = Uint256;

/// Standard TVM Cell representation per `docs/specification/state-model.md` §3.3 and §4.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub is_special: bool,
    pub data: Vec<u8>,
    pub refs: Vec<CellHash>,
}

impl Cell {
    /// Constructs a new `Cell` with validation on data byte length (<= 128) and ref count (<= 4).
    pub fn new(
        is_special: bool,
        data: Vec<u8>,
        refs: Vec<CellHash>,
    ) -> Result<Self, StateModelError> {
        if data.len() > CELL_MAX_DATA_BYTES {
            return Err(StateModelError::InvalidCellDataLength(data.len()));
        }
        if refs.len() > CELL_MAX_REFS {
            return Err(StateModelError::InvalidCellRefCount(refs.len()));
        }
        Ok(Self {
            is_special,
            data,
            refs,
        })
    }

    /// Computes descriptor byte d1 (ref_count | is_special_flag).
    pub fn d1(&self) -> u8 {
        let ref_count = self.refs.len() as u8;
        let special_flag = if self.is_special { 8 } else { 0 };
        ref_count | special_flag
    }

    /// Computes descriptor byte d2 (data_byte_length).
    pub fn d2(&self) -> u8 {
        self.data.len() as u8
    }

    /// Calculates domain-separated SHA-256 Cell hash per `docs/specification/state-model.md` §4.3.
    pub fn cell_hash(&self) -> CellHash {
        let mut buf = Vec::with_capacity(2 + self.data.len() + self.refs.len() * 32);
        buf.push(self.d1());
        buf.push(self.d2());
        buf.extend_from_slice(&self.data);
        for ref_hash in &self.refs {
            buf.extend_from_slice(&ref_hash.encode());
        }
        Uint256(domain_hash(&CELL_HASH_DOMAIN_TAG, &buf))
    }

    /// Serializes cell to binary representation per §4.2.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(2 + self.data.len() + self.refs.len() * 32);
        buf.push(self.d1());
        buf.push(self.d2());
        buf.extend_from_slice(&self.data);
        for ref_hash in &self.refs {
            buf.extend_from_slice(&ref_hash.encode());
        }
        buf
    }

    /// Deserializes a cell from binary payload per §4.2.
    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), StateModelError> {
        if bytes.len() < 2 {
            return Err(StateModelError::InvalidCellDataLength(bytes.len()));
        }
        let d1 = bytes[0];
        let d2 = bytes[1] as usize;

        let ref_count = (d1 & 0x07) as usize;
        let is_special = (d1 & 0x08) != 0;

        if ref_count > CELL_MAX_REFS {
            return Err(StateModelError::InvalidCellRefCount(ref_count));
        }
        if d2 > CELL_MAX_DATA_BYTES {
            return Err(StateModelError::InvalidCellDataLength(d2));
        }

        let total_expected = 2 + d2 + ref_count * 32;
        if bytes.len() < total_expected {
            return Err(StateModelError::InvalidCellDataLength(bytes.len()));
        }

        let data = bytes[2..2 + d2].to_vec();
        let mut refs = Vec::with_capacity(ref_count);
        let mut offset = 2 + d2;

        for _ in 0..ref_count {
            let ref_bytes: [u8; 32] = bytes[offset..offset + 32].try_into().unwrap();
            refs.push(Uint256::decode_exact(&ref_bytes)?);
            offset += 32;
        }

        Ok((
            Cell {
                is_special,
                data,
                refs,
            },
            total_expected,
        ))
    }
}

/// Bag-of-Cells (BoC) sequence representing a rooted DAG per §3.3 and §4.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoC {
    pub cells: Vec<Cell>,
}

impl BoC {
    /// Constructs and validates a Bag-of-Cells sequence.
    pub fn new(cells: Vec<Cell>) -> Result<Self, StateModelError> {
        let boc = Self { cells };
        boc.validate_dag()?;
        Ok(boc)
    }

    /// Computes the State Root Hash (hash of the root cell at index 0).
    pub fn root_hash(&self) -> Result<CellHash, StateModelError> {
        self.cells
            .first()
            .map(|cell| cell.cell_hash())
            .ok_or_else(|| {
                StateModelError::InvalidStateTransition("Empty Bag-of-Cells".to_string())
            })
    }

    /// Validates that the cell sequence forms an acyclic directed graph (DAG).
    pub fn validate_dag(&self) -> Result<(), StateModelError> {
        let hashes: Vec<CellHash> = self.cells.iter().map(|c| c.cell_hash()).collect();

        // Check for self-referential base payload hash cycles
        for cell in &self.cells {
            let base_payload_hash =
                Cell::new(cell.is_special, cell.data.clone(), vec![])?.cell_hash();
            if cell.refs.contains(&base_payload_hash) {
                return Err(StateModelError::CyclicCellReference);
            }
        }

        let mut visited = HashSet::new();
        let mut in_stack = HashSet::new();

        for i in 0..self.cells.len() {
            let hash = hashes[i];
            if !visited.contains(&hash) {
                self.dfs_check_cycle(i, &hashes, &mut visited, &mut in_stack)?;
            }
        }

        Ok(())
    }

    fn dfs_check_cycle(
        &self,
        cell_idx: usize,
        hashes: &[CellHash],
        visited: &mut HashSet<CellHash>,
        in_stack: &mut HashSet<CellHash>,
    ) -> Result<(), StateModelError> {
        let hash = hashes[cell_idx];
        visited.insert(hash);
        in_stack.insert(hash);

        for ref_hash in &self.cells[cell_idx].refs {
            if in_stack.contains(ref_hash) {
                return Err(StateModelError::CyclicCellReference);
            }
            if let Some(child_idx) = hashes.iter().position(|h| h == ref_hash) {
                if !visited.contains(ref_hash) {
                    self.dfs_check_cycle(child_idx, hashes, visited, in_stack)?;
                }
            }
        }

        in_stack.remove(&hash);
        Ok(())
    }

    /// Serializes BoC to binary stream.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&(self.cells.len() as u32).to_be_bytes());
        for cell in &self.cells {
            buf.extend_from_slice(&cell.to_bytes());
        }
        buf
    }

    /// Deserializes BoC from binary stream and validates DAG acyclicity.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, StateModelError> {
        if bytes.len() < 4 {
            return Err(StateModelError::InvalidStateTransition(
                "Truncated BoC length".to_string(),
            ));
        }
        let cell_count = u32::from_be_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let mut cells = Vec::with_capacity(cell_count);
        let mut offset = 4;

        for _ in 0..cell_count {
            let (cell, len) = Cell::from_bytes(&bytes[offset..])?;
            cells.push(cell);
            offset += len;
        }

        if offset != bytes.len() {
            return Err(StateModelError::InvalidStateTransition(format!(
                "Trailing bytes in BoC: expected {}, got {}",
                offset,
                bytes.len()
            )));
        }

        Self::new(cells)
    }
}
