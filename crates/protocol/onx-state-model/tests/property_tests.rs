//! Property tests enforcing the malformed-input and canonical-round-trip rules
//! in `docs/specification/state-model.md` §5.

use std::collections::HashMap;

use onx_state_model::{BagOfCells, Cell, StateModelError};
use proptest::prelude::*;

proptest! {
    /// §5: valid cells must preserve all descriptor, data, and reference bytes.
    #[test]
    fn cell_round_trips(data in proptest::collection::vec(any::<u8>(), 0..=128),
                        refs in proptest::collection::vec(any::<[u8; 32]>(), 0..=4),
                        special in any::<bool>()) {
        let cell = Cell::new_with_special(data, refs, special).unwrap();
        let encoded = cell.to_bytes();
        let (decoded, used) = Cell::from_bytes(&encoded).unwrap();
        prop_assert_eq!(used, encoded.len());
        prop_assert_eq!(decoded, cell);
    }

    /// §5: BoC serialization preserves randomly shaped, acyclic star DAGs.
    #[test]
    fn boc_round_trips(leaves in proptest::collection::vec(proptest::collection::vec(any::<u8>(), 0..=32), 1..=4),
                       root_data in proptest::collection::vec(any::<u8>(), 0..=128)) {
        let mut cells = HashMap::new();
        let mut refs = Vec::new();
        for data in leaves {
            let leaf = Cell::new(data, vec![]).unwrap();
            let leaf_hash = leaf.hash();
            refs.push(leaf_hash);
            cells.insert(leaf_hash, leaf);
        }
        let root = Cell::new(root_data, refs).unwrap();
        let root_hash = root.hash();
        cells.insert(root_hash, root);
        let boc = BagOfCells::new(root_hash, cells).unwrap();
        let bytes = boc.to_bytes();
        let (decoded, used) = BagOfCells::from_bytes(&bytes).unwrap();
        prop_assert_eq!(used, bytes.len());
        prop_assert_eq!(decoded, boc);
    }

    /// §5: descriptors with more than 128 data bytes are rejected.
    #[test]
    fn rejects_oversized_data(extra in 129u8..=255) {
        let bytes = vec![0, extra];
        let is_expected = matches!(Cell::from_bytes(&bytes), Err(StateModelError::DataTooLarge { .. }));
        prop_assert!(is_expected);
    }

    /// §5: descriptors with more than four references are rejected.
    #[test]
    fn rejects_too_many_references(count in 5u8..=7) {
        let bytes = vec![count, 0];
        let is_expected = matches!(Cell::from_bytes(&bytes), Err(StateModelError::TooManyReferences { .. }));
        prop_assert!(is_expected);
    }

    /// §5: a generated self-reference in a BoC is rejected (before it can become a DAG).
    #[test]
    fn rejects_cyclic_reference(key in any::<[u8; 32]>()) {
        let cell = Cell::new(vec![], vec![key]).unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&key);
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.extend_from_slice(&key);
        let encoded = cell.to_bytes();
        bytes.extend_from_slice(&(encoded.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&encoded);
        prop_assert!(BagOfCells::from_bytes(&bytes).is_err());
    }
}
