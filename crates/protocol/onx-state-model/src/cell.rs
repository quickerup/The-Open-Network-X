use crate::error::StateModelError;
use onx_primitives::{domain_hash, DomainTag, Uint16};

/// Domain separation tag for Cell representation hashing per docs/specification/state-model.md §4.3.
pub const ONX_CELL_HASH_V1_TAG: DomainTag = DomainTag::from_ascii("ONX_CELL_HASH_V1");

pub const MAX_CELL_DATA_BYTES: usize = 128;
pub const MAX_CELL_REFS: usize = 4;

/// A canonical Cell structure containing up to 128 data bytes and up to 4 references to child cell hashes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Cell {
    data_bytes: Vec<u8>,
    cell_refs: Vec<[u8; 32]>,
    is_special: bool,
}

impl Cell {
    /// Creates a new standard cell with given data bytes and child cell hashes.
    pub fn new(data_bytes: Vec<u8>, cell_refs: Vec<[u8; 32]>) -> Result<Self, StateModelError> {
        Self::new_with_special(data_bytes, cell_refs, false)
    }

    /// Creates a new cell specifying if special flag is set.
    pub fn new_with_special(
        data_bytes: Vec<u8>,
        cell_refs: Vec<[u8; 32]>,
        is_special: bool,
    ) -> Result<Self, StateModelError> {
        if data_bytes.len() > MAX_CELL_DATA_BYTES {
            return Err(StateModelError::DataTooLarge {
                length: data_bytes.len(),
                max: MAX_CELL_DATA_BYTES,
            });
        }
        if cell_refs.len() > MAX_CELL_REFS {
            return Err(StateModelError::TooManyReferences {
                count: cell_refs.len(),
                max: MAX_CELL_REFS,
            });
        }
        Ok(Self {
            data_bytes,
            cell_refs,
            is_special,
        })
    }

    pub fn data_bytes(&self) -> &[u8] {
        &self.data_bytes
    }

    pub fn cell_refs(&self) -> &[[u8; 32]] {
        &self.cell_refs
    }

    pub fn is_special(&self) -> bool {
        self.is_special
    }

    /// Computes the descriptor bytes `(d1, d2)` per spec §4.2.
    /// `d1` = `ref_count | (if is_special { 8 } else { 0 })`
    /// `d2` = `data_byte_length`
    pub fn descriptor_bytes(&self) -> (u8, u8) {
        let mut d1 = self.cell_refs.len() as u8;
        if self.is_special {
            d1 |= 0x08;
        }
        let d2 = self.data_bytes.len() as u8;
        (d1, d2)
    }

    /// Computes the 32-byte domain-separated SHA-256 cell representation hash per spec §4.3.
    pub fn hash(&self) -> [u8; 32] {
        let (d1, d2) = self.descriptor_bytes();
        let mut payload = Vec::with_capacity(2 + self.data_bytes.len() + self.cell_refs.len() * 32);
        payload.push(d1);
        payload.push(d2);
        payload.extend_from_slice(&self.data_bytes);
        for ref_hash in &self.cell_refs {
            payload.extend_from_slice(ref_hash);
        }
        domain_hash(&ONX_CELL_HASH_V1_TAG, &payload)
    }

    /// Serializes the cell into binary payload per spec §4.2.
    pub fn to_bytes(&self) -> Vec<u8> {
        let (d1, d2) = self.descriptor_bytes();
        let descriptor_u16 = ((d1 as u16) << 8) | (d2 as u16);
        let mut bytes = Vec::with_capacity(2 + self.data_bytes.len() + self.cell_refs.len() * 32);
        bytes.extend_from_slice(&Uint16(descriptor_u16).encode());
        bytes.extend_from_slice(&self.data_bytes);
        for ref_hash in &self.cell_refs {
            bytes.extend_from_slice(ref_hash);
        }
        bytes
    }

    /// Deserializes a Cell from binary slice.
    pub fn from_bytes(slice: &[u8]) -> Result<(Self, usize), StateModelError> {
        if slice.len() < 2 {
            return Err(StateModelError::DeserializationError(
                "Slice too short for cell descriptor".to_string(),
            ));
        }

        let mut cursor = slice;
        let descriptor_val = Uint16::read(&mut cursor)
            .map_err(|e| StateModelError::DeserializationError(e.to_string()))?;
        let descriptor_u16 = descriptor_val.0;
        let mut offset = Uint16::BYTE_LEN;

        let d1 = (descriptor_u16 >> 8) as u8;
        let d2 = (descriptor_u16 & 0xFF) as u8;

        let ref_count = (d1 & 0x07) as usize;
        let is_special = (d1 & 0x08) != 0;
        let data_len = d2 as usize;

        if ref_count > MAX_CELL_REFS {
            return Err(StateModelError::TooManyReferences {
                count: ref_count,
                max: MAX_CELL_REFS,
            });
        }
        if data_len > MAX_CELL_DATA_BYTES {
            return Err(StateModelError::DataTooLarge {
                length: data_len,
                max: MAX_CELL_DATA_BYTES,
            });
        }

        let required_len = offset + data_len + ref_count * 32;
        if slice.len() < required_len {
            return Err(StateModelError::DeserializationError(format!(
                "Truncated Cell payload: expected {} bytes, got {}",
                required_len,
                slice.len()
            )));
        }

        let data_bytes = slice[offset..offset + data_len].to_vec();
        offset += data_len;

        let mut cell_refs = Vec::with_capacity(ref_count);
        for _ in 0..ref_count {
            let mut ref_hash = [0u8; 32];
            ref_hash.copy_from_slice(&slice[offset..offset + 32]);
            cell_refs.push(ref_hash);
            offset += 32;
        }

        let cell = Self::new_with_special(data_bytes, cell_refs, is_special)?;
        Ok((cell, offset))
    }
}
