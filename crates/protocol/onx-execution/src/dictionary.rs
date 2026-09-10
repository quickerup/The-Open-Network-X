//! Binary Patricia dictionaries whose nodes are committed by TVM cells.
use onx_state_model::Cell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictionaryError {
    InvalidKeyLength,
    InvalidKey,
    Cell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    Leaf {
        suffix: Vec<bool>,
        value: Cell,
    },
    Branch {
        prefix: Vec<bool>,
        zero: Box<Node>,
        one: Box<Node>,
    },
}

/// A fixed-key-width binary Patricia dictionary.  Values are cells, and every
/// mutation rebuilds the affected immutable node cells, making `root_cell` a
/// deterministic commitment to the dictionary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dictionary {
    key_bits: usize,
    root: Option<Box<Node>>,
    root_cell: Option<Cell>,
}

impl Dictionary {
    pub fn new(key_bits: usize) -> Result<Self, DictionaryError> {
        if !(1..=1023).contains(&key_bits) {
            return Err(DictionaryError::InvalidKeyLength);
        }
        Ok(Self {
            key_bits,
            root: None,
            root_cell: None,
        })
    }
    pub fn key_bits(&self) -> usize {
        self.key_bits
    }
    pub fn root_cell(&self) -> Option<&Cell> {
        self.root_cell.as_ref()
    }
    pub fn get(&self, key: &[u8]) -> Result<Option<&Cell>, DictionaryError> {
        let bits = key_bits(key, self.key_bits)?;
        Ok(self.root.as_deref().and_then(|node| get(node, &bits)))
    }
    /// TVM `DictGet` operation.
    pub fn dict_get(&self, key: &[u8]) -> Result<Option<&Cell>, DictionaryError> {
        self.get(key)
    }
    pub fn set(&mut self, key: &[u8], value: Cell) -> Result<Option<Cell>, DictionaryError> {
        let bits = key_bits(key, self.key_bits)?;
        let mut old = None;
        self.root = Some(Box::new(insert(self.root.take(), &bits, value, &mut old)));
        self.root_cell = Some(serialize(self.root.as_ref().expect("root just installed"))?);
        Ok(old)
    }
    /// TVM `DictSet` operation.
    pub fn dict_set(&mut self, key: &[u8], value: Cell) -> Result<Option<Cell>, DictionaryError> {
        self.set(key, value)
    }
    pub fn delete(&mut self, key: &[u8]) -> Result<Option<Cell>, DictionaryError> {
        let bits = key_bits(key, self.key_bits)?;
        let mut old = None;
        self.root = remove(self.root.take(), &bits, &mut old);
        self.root_cell = self.root.as_ref().map(|node| serialize(node)).transpose()?;
        Ok(old)
    }
    /// TVM `DictDel` operation.
    pub fn dict_del(&mut self, key: &[u8]) -> Result<Option<Cell>, DictionaryError> {
        self.delete(key)
    }
}

fn key_bits(key: &[u8], width: usize) -> Result<Vec<bool>, DictionaryError> {
    if key.len() * 8 < width || key.len() * 8 >= width + 8 {
        return Err(DictionaryError::InvalidKey);
    }
    Ok((0..width)
        .map(|i| key[i / 8] & (1 << (7 - i % 8)) != 0)
        .collect())
}
fn common(a: &[bool], b: &[bool]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}
fn get<'a>(node: &'a Node, bits: &[bool]) -> Option<&'a Cell> {
    match node {
        Node::Leaf { suffix, value } => (suffix == bits).then_some(value),
        Node::Branch { prefix, zero, one }
            if bits.starts_with(prefix) && bits.len() > prefix.len() =>
        {
            get(
                if bits[prefix.len()] { one } else { zero },
                &bits[prefix.len() + 1..],
            )
        }
        _ => None,
    }
}
fn insert(node: Option<Box<Node>>, bits: &[bool], value: Cell, old: &mut Option<Cell>) -> Node {
    match node {
        None => Node::Leaf {
            suffix: bits.to_vec(),
            value,
        },
        Some(node) => match *node {
            Node::Leaf {
                suffix,
                value: prior,
            } => {
                let n = common(&suffix, bits);
                if n == suffix.len() && n == bits.len() {
                    *old = Some(prior);
                    return Node::Leaf { suffix, value };
                }
                let old_bit = suffix[n];
                let old_leaf = Node::Leaf {
                    suffix: suffix[n + 1..].to_vec(),
                    value: prior,
                };
                let new_leaf = Node::Leaf {
                    suffix: bits[n + 1..].to_vec(),
                    value,
                };
                if old_bit {
                    Node::Branch {
                        prefix: suffix[..n].to_vec(),
                        zero: Box::new(new_leaf),
                        one: Box::new(old_leaf),
                    }
                } else {
                    Node::Branch {
                        prefix: suffix[..n].to_vec(),
                        zero: Box::new(old_leaf),
                        one: Box::new(new_leaf),
                    }
                }
            }
            Node::Branch { prefix, zero, one } => {
                let n = common(&prefix, bits);
                if n < prefix.len() {
                    let old_bit = prefix[n];
                    let old = Node::Branch {
                        prefix: prefix[n + 1..].to_vec(),
                        zero,
                        one,
                    };
                    let new = Node::Leaf {
                        suffix: bits[n + 1..].to_vec(),
                        value,
                    };
                    if old_bit {
                        Node::Branch {
                            prefix: prefix[..n].to_vec(),
                            zero: Box::new(new),
                            one: Box::new(old),
                        }
                    } else {
                        Node::Branch {
                            prefix: prefix[..n].to_vec(),
                            zero: Box::new(old),
                            one: Box::new(new),
                        }
                    }
                } else {
                    let index = prefix.len();
                    if bits[index] {
                        Node::Branch {
                            prefix,
                            zero,
                            one: Box::new(insert(Some(one), &bits[index + 1..], value, old)),
                        }
                    } else {
                        Node::Branch {
                            prefix,
                            zero: Box::new(insert(Some(zero), &bits[index + 1..], value, old)),
                            one,
                        }
                    }
                }
            }
        },
    }
}
fn remove(node: Option<Box<Node>>, bits: &[bool], old: &mut Option<Cell>) -> Option<Box<Node>> {
    let node = node?;
    match *node {
        Node::Leaf { suffix, value } => {
            if suffix == bits {
                *old = Some(value);
                None
            } else {
                Some(Box::new(Node::Leaf { suffix, value }))
            }
        }
        Node::Branch { prefix, zero, one } => {
            if !bits.starts_with(&prefix) || bits.len() <= prefix.len() {
                return Some(Box::new(Node::Branch { prefix, zero, one }));
            }
            let i = prefix.len();
            if bits[i] {
                match remove(Some(one), &bits[i + 1..], old) {
                    Some(child) => Some(Box::new(Node::Branch {
                        prefix,
                        zero,
                        one: child,
                    })),
                    None => Some(Box::new(prepend(prefix, false, *zero))),
                }
            } else {
                match remove(Some(zero), &bits[i + 1..], old) {
                    Some(child) => Some(Box::new(Node::Branch {
                        prefix,
                        zero: child,
                        one,
                    })),
                    None => Some(Box::new(prepend(prefix, true, *one))),
                }
            }
        }
    }
}
fn prepend(mut prefix: Vec<bool>, edge: bool, node: Node) -> Node {
    match node {
        Node::Leaf { mut suffix, value } => {
            prefix.push(edge);
            prefix.append(&mut suffix);
            Node::Leaf {
                suffix: prefix,
                value,
            }
        }
        Node::Branch {
            prefix: mut child_prefix,
            zero,
            one,
        } => {
            prefix.push(edge);
            prefix.append(&mut child_prefix);
            Node::Branch { prefix, zero, one }
        }
    }
}

// Node cells use tags 0xD0 (leaf) and 0xD1 (branch), a u16 label length and
// packed label bits. Long labels are chained through 0xD2 cells, so all valid
// 1023-bit keys remain representable within the 128-byte cell limit.
fn serialize(node: &Node) -> Result<Cell, DictionaryError> {
    match node {
        Node::Leaf { suffix, value } => node_cell(0xD0, suffix, vec![value.hash()]),
        Node::Branch { prefix, zero, one } => node_cell(
            0xD1,
            prefix,
            vec![serialize(zero)?.hash(), serialize(one)?.hash()],
        ),
    }
}
fn node_cell(tag: u8, bits: &[bool], mut refs: Vec<[u8; 32]>) -> Result<Cell, DictionaryError> {
    let first = bits.len().min(1000);
    let mut data = vec![tag];
    data.extend_from_slice(&(bits.len() as u16).to_be_bytes());
    data.extend(pack(&bits[..first]));
    if first < bits.len() {
        refs.insert(0, label_tail(&bits[first..])?.hash());
    }
    Cell::new(data, refs).map_err(|_| DictionaryError::Cell)
}
fn label_tail(bits: &[bool]) -> Result<Cell, DictionaryError> {
    let first = bits.len().min(1000);
    let mut refs = Vec::new();
    if first < bits.len() {
        refs.push(label_tail(&bits[first..])?.hash());
    }
    Cell::new([vec![0xD2], pack(&bits[..first])].concat(), refs).map_err(|_| DictionaryError::Cell)
}
fn pack(bits: &[bool]) -> Vec<u8> {
    let mut out = vec![0; bits.len().div_ceil(8)];
    for (i, bit) in bits.iter().enumerate() {
        if *bit {
            out[i / 8] |= 1 << (7 - i % 8);
        }
    }
    out
}
