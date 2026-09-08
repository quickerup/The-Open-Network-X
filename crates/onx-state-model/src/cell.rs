use crate::error::StateModelError;
use onx_primitives::{domain_hash, DomainTag};

/// Domain separation tag for Cell hashing per docs/specification/state-model.md §4.3.
pub const ONX_CELL_HASH_V1_TAG: DomainTag = DomainTag::from_ascii("ONX_CELL_HASH_V1");

/// Maximum data payload length in bytes for a standard Cell per §3.3.
pub const MAX_CELL_DATA_LEN: usize = 128;

/// Maximum number of child references for a standard Cell per §3.3.
pub const MAX_CELL_REFS: usize = 4;

/// Standard Cell primitive per docs/specification/state-model.md §3.3 and §4.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub is_special: bool,
    pub data: Vec<u8>,
    pub refs: Vec<[u8; 32]>,
}

impl Cell {
    /// Creates a new cell after validating capacity limits (max 128 data bytes, max 4 refs).
    pub fn new(
        data: Vec<u8>,
        refs: Vec<[u8; 32]>,
        is_special: bool,
    ) -> Result<Self, StateModelError> {
        if data.len() > MAX_CELL_DATA_LEN {
            return Err(StateModelError::MalformedCell(format!(
                "data length {} exceeds maximum allowed {}",
                data.len(),
                MAX_CELL_DATA_LEN
            )));
        }
        if refs.len() > MAX_CELL_REFS {
            return Err(StateModelError::MalformedCell(format!(
                "child cell references count {} exceeds maximum allowed {}",
                refs.len(),
                MAX_CELL_REFS
            )));
        }
        Ok(Self {
            is_special,
            data,
            refs,
        })
    }

    /// Computes descriptor byte 1 (d1 = ref_count | (is_special_flag << 3)).
    pub fn d1(&self) -> u8 {
        let ref_count = (self.refs.len() & 0x07) as u8;
        let special_flag = if self.is_special { 0x08 } else { 0x00 };
        ref_count | special_flag
    }

    /// Computes descriptor byte 2 (d2 = data_length).
    pub fn d2(&self) -> u8 {
        self.data.len() as u8
    }

    /// Serializes the cell into binary payload for hashing or BoC storage (§4.2).
    pub fn to_binary_payload(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(2 + self.data.len() + self.refs.len() * 32);
        buf.push(self.d1());
        buf.push(self.d2());
        buf.extend_from_slice(&self.data);
        for ref_hash in &self.refs {
            buf.extend_from_slice(ref_hash);
        }
        buf
    }

    /// Computes the domain-separated SHA-256 Cell hash per §4.3.
    pub fn hash(&self) -> [u8; 32] {
        domain_hash(&ONX_CELL_HASH_V1_TAG, &self.to_binary_payload())
    }

    /// Deserializes a cell from binary payload.
    pub fn from_binary_payload(slice: &[u8]) -> Result<Self, StateModelError> {
        if slice.len() < 2 {
            return Err(StateModelError::MalformedCell(
                "slice too short for cell descriptor bytes".to_string(),
            ));
        }
        let d1 = slice[0];
        let d2 = slice[1] as usize;

        let ref_count = (d1 & 0x07) as usize;
        let is_special = (d1 & 0x08) != 0;

        if ref_count > MAX_CELL_REFS {
            return Err(StateModelError::MalformedCell(format!(
                "cell descriptor specifies ref_count {ref_count} > {MAX_CELL_REFS}"
            )));
        }
        if d2 > MAX_CELL_DATA_LEN {
            return Err(StateModelError::MalformedCell(format!(
                "cell descriptor specifies data length {d2} > {MAX_CELL_DATA_LEN}"
            )));
        }

        let expected_len = 2 + d2 + ref_count * 32;
        if slice.len() != expected_len {
            return Err(StateModelError::MalformedCell(format!(
                "cell payload length mismatch: expected {expected_len}, got {}",
                slice.len()
            )));
        }

        let data = slice[2..2 + d2].to_vec();
        let mut refs = Vec::with_capacity(ref_count);
        let mut offset = 2 + d2;
        for _ in 0..ref_count {
            let mut ref_hash = [0u8; 32];
            ref_hash.copy_from_slice(&slice[offset..offset + 32]);
            refs.push(ref_hash);
            offset += 32;
        }

        Ok(Self {
            is_special,
            data,
            refs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_creation_and_hashing() {
        let cell = Cell::new(vec![0x01, 0x02, 0x03], vec![], false).unwrap();
        assert_eq!(cell.d1(), 0x00);
        assert_eq!(cell.d2(), 3);

        let payload = cell.to_binary_payload();
        assert_eq!(payload, vec![0x00, 0x03, 0x01, 0x02, 0x03]);

        let hash1 = cell.hash();
        let hash2 = cell.hash();
        assert_eq!(hash1, hash2);

        let decoded = Cell::from_binary_payload(&payload).unwrap();
        assert_eq!(cell, decoded);
    }

    #[test]
    fn test_cell_limits_enforcement() {
        // Exceeding data len
        let too_much_data = vec![0u8; 129];
        assert!(Cell::new(too_much_data, vec![], false).is_err());

        // Exceeding refs
        let too_many_refs = vec![[0u8; 32]; 5];
        assert!(Cell::new(vec![1, 2], too_many_refs, false).is_err());
    }
}
