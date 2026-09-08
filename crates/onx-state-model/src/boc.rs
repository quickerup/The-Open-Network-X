use crate::{cell::Cell, error::StateModelError};
use onx_primitives::integers::{Uint16, Uint32};
use std::collections::HashMap;

/// Magic bytes prefix for Bag-of-Cells ("BOC1" = 0x424F4331).
pub const BOC_MAGIC: [u8; 4] = *b"BOC1";

/// A Bag-of-Cells (BoC) structure holding an acyclic directed graph of cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BagOfCells {
    pub cells: Vec<Cell>,
    pub root_indexes: Vec<usize>,
}

impl BagOfCells {
    /// Creates a new BagOfCells from cells and root indexes after verifying DAG property (no cycles).
    pub fn new(cells: Vec<Cell>, root_indexes: Vec<usize>) -> Result<Self, StateModelError> {
        if cells.is_empty() {
            return Err(StateModelError::MalformedBagOfCells(
                "cell list cannot be empty".to_string(),
            ));
        }
        if root_indexes.is_empty() {
            return Err(StateModelError::MalformedBagOfCells(
                "root index list cannot be empty".to_string(),
            ));
        }
        for &idx in &root_indexes {
            if idx >= cells.len() {
                return Err(StateModelError::MalformedBagOfCells(format!(
                    "root index {idx} out of bounds (cell count {})",
                    cells.len()
                )));
            }
        }

        let boc = Self {
            cells,
            root_indexes,
        };
        boc.validate_dag()?;
        Ok(boc)
    }

    /// Validates that the cell graph contains no directed cycles (DAG invariant §3.3 & §5.5).
    pub fn validate_dag(&self) -> Result<(), StateModelError> {
        let mut hash_to_index: HashMap<[u8; 32], usize> = HashMap::with_capacity(self.cells.len());
        for (i, cell) in self.cells.iter().enumerate() {
            hash_to_index.insert(cell.hash(), i);
        }
        Self::check_dag_cycles(&self.cells, &hash_to_index)
    }

    /// Internal DFS cycle detector over a cell slice and hash-to-index mapping.
    pub fn check_dag_cycles(
        cells: &[Cell],
        hash_to_index: &HashMap<[u8; 32], usize>,
    ) -> Result<(), StateModelError> {
        let mut visited = vec![0u8; cells.len()]; // 0: unvisited, 1: visiting, 2: visited

        fn dfs(
            idx: usize,
            cells: &[Cell],
            hash_to_index: &HashMap<[u8; 32], usize>,
            visited: &mut [u8],
        ) -> Result<(), StateModelError> {
            if visited[idx] == 1 {
                return Err(StateModelError::CyclicCellReference);
            }
            if visited[idx] == 2 {
                return Ok(());
            }

            visited[idx] = 1;
            let cell = &cells[idx];
            for ref_hash in &cell.refs {
                if let Some(&child_idx) = hash_to_index.get(ref_hash) {
                    dfs(child_idx, cells, hash_to_index, visited)?;
                }
            }
            visited[idx] = 2;
            Ok(())
        }

        for i in 0..cells.len() {
            if visited[i] == 0 {
                dfs(i, cells, hash_to_index, &mut visited)?;
            }
        }

        Ok(())
    }

    /// Returns the primary root cell hash (first root).
    pub fn state_root_hash(&self) -> Result<[u8; 32], StateModelError> {
        let root_idx = *self.root_indexes.first().ok_or_else(|| {
            StateModelError::MalformedBagOfCells("no root cells in BoC".to_string())
        })?;
        Ok(self.cells[root_idx].hash())
    }

    /// Serializes the Bag-of-Cells to binary format.
    /// Binary format:
    /// - magic: 4 bytes ("BOC1")
    /// - cell_count: uint32 (4 bytes)
    /// - root_count: uint32 (4 bytes)
    /// - root_indexes: [uint32; root_count]
    /// - cells: sequence of cell binary payloads with 2-byte header length per cell
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&BOC_MAGIC);
        buf.extend_from_slice(&Uint32::from(self.cells.len() as u32).encode());
        buf.extend_from_slice(&Uint32::from(self.root_indexes.len() as u32).encode());

        for &root_idx in &self.root_indexes {
            buf.extend_from_slice(&Uint32::from(root_idx as u32).encode());
        }

        for cell in &self.cells {
            let payload = cell.to_binary_payload();
            buf.extend_from_slice(&Uint16::from(payload.len() as u16).encode());
            buf.extend_from_slice(&payload);
        }

        buf
    }

    /// Deserializes a Bag-of-Cells from binary format and verifies DAG invariants.
    pub fn from_bytes(slice: &[u8]) -> Result<Self, StateModelError> {
        if slice.len() < 12 {
            return Err(StateModelError::MalformedBagOfCells(
                "slice too short for BoC header".to_string(),
            ));
        }

        if slice[0..4] != BOC_MAGIC {
            return Err(StateModelError::MalformedBagOfCells(
                "invalid BoC magic bytes".to_string(),
            ));
        }

        let cell_count = Uint32::decode_exact(&slice[4..8])
            .map_err(|e| StateModelError::MalformedBagOfCells(e.to_string()))?
            .0 as usize;

        let root_count = Uint32::decode_exact(&slice[8..12])
            .map_err(|e| StateModelError::MalformedBagOfCells(e.to_string()))?
            .0 as usize;

        let header_len = 12 + root_count * 4;
        if slice.len() < header_len {
            return Err(StateModelError::MalformedBagOfCells(
                "slice too short for BoC root indexes".to_string(),
            ));
        }

        let mut root_indexes = Vec::with_capacity(root_count);
        let mut offset = 12;
        for _ in 0..root_count {
            let idx = Uint32::decode_exact(&slice[offset..offset + 4])
                .map_err(|e| StateModelError::MalformedBagOfCells(e.to_string()))?
                .0 as usize;
            if idx >= cell_count {
                return Err(StateModelError::MalformedBagOfCells(format!(
                    "root index {idx} out of range for cell count {cell_count}"
                )));
            }
            root_indexes.push(idx);
            offset += 4;
        }

        let mut cells = Vec::with_capacity(cell_count);
        for _ in 0..cell_count {
            if slice.len() < offset + 2 {
                return Err(StateModelError::MalformedBagOfCells(
                    "unexpected EOF reading cell payload length".to_string(),
                ));
            }
            let payload_len = Uint16::decode_exact(&slice[offset..offset + 2])
                .map_err(|e| StateModelError::MalformedBagOfCells(e.to_string()))?
                .0 as usize;
            offset += 2;

            if slice.len() < offset + payload_len {
                return Err(StateModelError::MalformedBagOfCells(
                    "unexpected EOF reading cell payload".to_string(),
                ));
            }

            let cell = Cell::from_binary_payload(&slice[offset..offset + payload_len])?;
            cells.push(cell);
            offset += payload_len;
        }

        if offset != slice.len() {
            return Err(StateModelError::MalformedBagOfCells(format!(
                "trailing bytes in BoC: consumed {offset}, total {}",
                slice.len()
            )));
        }

        Self::new(cells, root_indexes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boc_round_trip() {
        let child = Cell::new(vec![10, 20], vec![], false).unwrap();
        let child_hash = child.hash();

        let root = Cell::new(vec![1, 2, 3], vec![child_hash], false).unwrap();

        let boc = BagOfCells::new(vec![root, child], vec![0]).unwrap();
        let bytes = boc.to_bytes();

        let decoded = BagOfCells::from_bytes(&bytes).unwrap();
        assert_eq!(boc, decoded);
    }

    #[test]
    fn test_cycle_detection_rejection() {
        let hash1 = [1u8; 32];
        let hash2 = [2u8; 32];

        let cell1 = Cell::new(vec![1], vec![hash2], false).unwrap();
        let cell2 = Cell::new(vec![2], vec![hash1], false).unwrap();

        let mut mock_map = HashMap::new();
        mock_map.insert(hash1, 0);
        mock_map.insert(hash2, 1);

        let res = BagOfCells::check_dag_cycles(&[cell1, cell2], &mock_map);
        assert!(matches!(res, Err(StateModelError::CyclicCellReference)));
    }
}
