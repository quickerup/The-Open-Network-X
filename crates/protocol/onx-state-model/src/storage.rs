//! Durable, crash-recoverable state storage.
//!
//! The storage layout exposes four logical column families as directories. A
//! write-ahead journal makes a block commitment all-or-nothing across them.
use crate::{AccountState, BagOfCells, Cell, ShardStateTree, StateModelError};
use onx_data_structures::{AccountId, BlockHeader, ShardIdent};
use std::{
    collections::HashSet,
    fmt, fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};

pub const COLUMN_FAMILIES: [&str; 4] = ["cells", "accounts", "block_headers", "shard_states"];
const CELL_REFS_LEN: usize = 8;
#[derive(Debug)]
pub enum StorageError {
    Io(io::Error),
    State(StateModelError),
    Corrupt(String),
    RootMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
}
impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "storage I/O error: {e}"),
            Self::State(e) => write!(f, "state error: {e}"),
            Self::Corrupt(e) => write!(f, "corrupt persisted state: {e}"),
            Self::RootMismatch { .. } => {
                write!(f, "block state root does not match supplied state")
            }
        }
    }
}
impl std::error::Error for StorageError {}
impl From<io::Error> for StorageError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<StateModelError> for StorageError {
    fn from(e: StateModelError) -> Self {
        Self::State(e)
    }
}
#[derive(Debug, Clone)]
pub struct BlockCommitment {
    pub header: BlockHeader,
    pub state: ShardStateTree,
    pub cell_bocs: Vec<BagOfCells>,
}
pub struct StateStorage {
    root: PathBuf,
    write_lock: Mutex<()>,
}
#[derive(Default)]
struct Batch {
    puts: Vec<(PathBuf, Vec<u8>)>,
    removes: Vec<PathBuf>,
}
impl StateStorage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let root = path.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        for cf in COLUMN_FAMILIES {
            fs::create_dir_all(root.join(cf))?;
        }
        let s = Self {
            root,
            write_lock: Mutex::new(()),
        };
        s.recover()?;
        Ok(s)
    }
    pub fn put_cell(&self, cell: &Cell) -> Result<[u8; 32], StorageError> {
        let _g = self.write_lock.lock().expect("storage lock poisoned");
        let mut b = Batch::default();
        self.put_cells(std::iter::once(cell), &mut b)?;
        self.commit(b)?;
        Ok(cell.hash())
    }
    pub fn get_cell(&self, hash: &[u8; 32]) -> Result<Option<Cell>, StorageError> {
        self.read("cells", hash)?
            .map(|v| Self::decode_cell_record(hash, &v).map(|(_, c)| c))
            .transpose()
    }
    pub fn cell_refcount(&self, hash: &[u8; 32]) -> Result<Option<u64>, StorageError> {
        self.read("cells", hash)?
            .map(|v| Self::decode_cell_record(hash, &v).map(|(n, _)| n))
            .transpose()
    }
    pub fn put_boc(&self, boc: &BagOfCells) -> Result<(), StorageError> {
        boc.verify_dag()?;
        let _g = self.write_lock.lock().expect("storage lock poisoned");
        let mut b = Batch::default();
        self.put_cells(boc.cells().values(), &mut b)?;
        self.commit(b)
    }
    pub fn retain_cell_root(&self, root: [u8; 32]) -> Result<(), StorageError> {
        self.change_root(root, true)
    }
    pub fn release_cell_root(&self, root: [u8; 32]) -> Result<(), StorageError> {
        self.change_root(root, false)
    }
    pub fn get_account(
        &self,
        shard: ShardIdent,
        account: AccountId,
    ) -> Result<Option<AccountState>, StorageError> {
        self.read("accounts", &account_key(shard, account))?
            .map(|v| decode_account(&v))
            .transpose()
    }
    pub fn load_shard_state(&self, shard: ShardIdent) -> Result<ShardStateTree, StorageError> {
        let mut t = ShardStateTree::new();
        let prefix = hex(&shard.to_bytes());
        for e in fs::read_dir(self.root.join("accounts"))? {
            let e = e?;
            let name = e.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(&prefix) {
                let key = unhex(&name)?;
                if key.len() != 44 {
                    return Err(StorageError::Corrupt("invalid account key".into()));
                }
                t.insert(
                    AccountId::from_bytes(key[12..].try_into().unwrap()),
                    decode_account(&fs::read(e.path())?)?,
                );
            }
        }
        Ok(t)
    }
    pub fn shard_state_root(&self, shard: ShardIdent) -> Result<Option<[u8; 32]>, StorageError> {
        self.read("shard_states", &shard.to_bytes())?
            .map(|v| {
                v.as_slice()
                    .try_into()
                    .map_err(|_| StorageError::Corrupt("invalid shard root length".into()))
            })
            .transpose()
    }
    pub fn get_block_header(&self, hash: &[u8; 32]) -> Result<Option<BlockHeader>, StorageError> {
        self.read("block_headers", hash)?
            .map(|v| BlockHeader::from_bytes(&v).map_err(|e| StorageError::Corrupt(e.to_string())))
            .transpose()
    }
    pub fn commit_block(&self, c: BlockCommitment) -> Result<(), StorageError> {
        let actual = c.state.state_root_hash();
        let expected = c.header.state_root_hash.0;
        if actual != expected {
            return Err(StorageError::RootMismatch { expected, actual });
        }
        for boc in &c.cell_bocs {
            boc.verify_dag()?;
        }
        let proof = c.state.generate_proof(AccountId::from_bytes([0; 32]))?;
        let _g = self.write_lock.lock().expect("storage lock poisoned");
        let mut b = Batch::default();
        for boc in &c.cell_bocs {
            self.put_cells(boc.cells().values(), &mut b)?;
        }
        self.put_cells(proof.proof_boc.cells().values(), &mut b)?;
        let shard = c.header.shard;
        let old_root = self.shard_state_root(shard)?;
        let old = self.load_shard_state(shard)?.accounts().clone();
        for a in old.keys() {
            b.removes
                .push(self.file("accounts", &account_key(shard, *a)));
        }
        for (a, s) in c.state.accounts() {
            b.puts
                .push((self.file("accounts", &account_key(shard, *a)), s.to_bytes()));
        }
        let mut seen = HashSet::new();
        self.adjust_cell(proof.root_hash, true, &mut seen, &mut b)?;
        if let Some(root) = old_root {
            let mut seen = HashSet::new();
            self.adjust_cell(root, false, &mut seen, &mut b)?;
        }
        b.puts.push((
            self.file("block_headers", &header_hash(&c.header)),
            c.header.to_bytes().to_vec(),
        ));
        b.puts.push((
            self.file("shard_states", &shard.to_bytes()),
            actual.to_vec(),
        ));
        self.commit(b)
    }
    fn change_root(&self, root: [u8; 32], up: bool) -> Result<(), StorageError> {
        let _g = self.write_lock.lock().expect("storage lock poisoned");
        let mut b = Batch::default();
        self.adjust_cell(root, up, &mut HashSet::new(), &mut b)?;
        self.commit(b)
    }
    fn put_cells<'a>(
        &self,
        cells: impl IntoIterator<Item = &'a Cell>,
        b: &mut Batch,
    ) -> Result<(), StorageError> {
        for cell in cells {
            let h = cell.hash();
            if let Some(v) = self.read("cells", &h)? {
                let (_, old) = Self::decode_cell_record(&h, &v)?;
                if old != *cell {
                    return Err(StorageError::Corrupt(
                        "cell hash maps to different bytes".into(),
                    ));
                }
            } else {
                b.puts
                    .push((self.file("cells", &h), encode_cell_record(0, cell)));
            }
        }
        Ok(())
    }
    fn adjust_cell(
        &self,
        h: [u8; 32],
        up: bool,
        seen: &mut HashSet<[u8; 32]>,
        b: &mut Batch,
    ) -> Result<(), StorageError> {
        if !seen.insert(h) {
            return Ok(());
        }
        let cell_path = self.file("cells", &h);
        let v = b
            .puts
            .iter()
            .rev()
            .find(|(path, _)| *path == cell_path)
            .map(|(_, value)| value.clone())
            .or(self.read("cells", &h)?)
            .ok_or_else(|| StorageError::Corrupt("referenced cell is absent".into()))?;
        let (n, c) = Self::decode_cell_record(&h, &v)?;
        if up {
            b.puts.push((
                self.file("cells", &h),
                encode_cell_record(
                    n.checked_add(1)
                        .ok_or_else(|| StorageError::Corrupt("cell refcount overflow".into()))?,
                    &c,
                ),
            ));
            for child in c.cell_refs() {
                self.adjust_cell(*child, true, seen, b)?;
            }
        } else if n == 0 {
            return Err(StorageError::Corrupt("cell refcount underflow".into()));
        } else if n == 1 {
            b.removes.push(self.file("cells", &h));
            for child in c.cell_refs() {
                self.adjust_cell(*child, false, seen, b)?;
            }
        } else {
            b.puts
                .push((self.file("cells", &h), encode_cell_record(n - 1, &c)));
        }
        Ok(())
    }
    fn read(&self, cf: &str, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        match fs::read(self.file(cf, key)) {
            Ok(v) => Ok(Some(v)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    fn file(&self, cf: &str, key: &[u8]) -> PathBuf {
        self.root.join(cf).join(hex(key))
    }
    fn commit(&self, b: Batch) -> Result<(), StorageError> {
        let journal = self.root.join(".commit-journal");
        let mut data = Vec::new();
        for p in b.removes {
            data.extend_from_slice(b"D ");
            data.extend_from_slice(
                p.strip_prefix(&self.root)
                    .unwrap()
                    .to_string_lossy()
                    .as_bytes(),
            );
            data.push(b'\n');
        }
        for (p, v) in b.puts {
            data.extend_from_slice(b"P ");
            data.extend_from_slice(
                p.strip_prefix(&self.root)
                    .unwrap()
                    .to_string_lossy()
                    .as_bytes(),
            );
            data.push(b' ');
            data.extend_from_slice(hex(&v).as_bytes());
            data.push(b'\n');
        }
        fs::write(&journal, data)?;
        fs::File::open(&journal)?.sync_all()?;
        self.recover()?;
        Ok(())
    }
    fn recover(&self) -> Result<(), StorageError> {
        let j = self.root.join(".commit-journal");
        let Ok(text) = fs::read_to_string(&j) else {
            return Ok(());
        };
        for line in text.lines() {
            let mut p = line.splitn(3, ' ');
            match (p.next(), p.next(), p.next()) {
                (Some("D"), Some(path), _) => {
                    let _ = fs::remove_file(self.root.join(path));
                }
                (Some("P"), Some(path), Some(value)) => {
                    let dst = self.root.join(path);
                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    let tmp = dst.with_extension("tmp");
                    fs::write(&tmp, unhex(value)?)?;
                    fs::rename(tmp, dst)?;
                }
                _ => return Err(StorageError::Corrupt("invalid commit journal".into())),
            }
        }
        fs::remove_file(j)?;
        Ok(())
    }
    fn decode_cell_record(h: &[u8; 32], v: &[u8]) -> Result<(u64, Cell), StorageError> {
        if v.len() < CELL_REFS_LEN {
            return Err(StorageError::Corrupt("truncated cell record".into()));
        }
        let n = u64::from_be_bytes(v[..8].try_into().unwrap());
        let (c, used) = Cell::from_bytes(&v[8..])?;
        if used + 8 != v.len() || c.hash() != *h {
            return Err(StorageError::Corrupt("invalid cell record".into()));
        }
        Ok((n, c))
    }
}
fn encode_cell_record(n: u64, c: &Cell) -> Vec<u8> {
    let mut v = n.to_be_bytes().to_vec();
    v.extend_from_slice(&c.to_bytes());
    v
}
fn account_key(s: ShardIdent, a: AccountId) -> [u8; 44] {
    let mut k = [0; 44];
    k[..12].copy_from_slice(&s.to_bytes());
    k[12..].copy_from_slice(&a.to_bytes());
    k
}
fn decode_account(v: &[u8]) -> Result<AccountState, StorageError> {
    let (s, n) = AccountState::from_bytes(v)?;
    if n != v.len() {
        return Err(StorageError::Corrupt("trailing account bytes".into()));
    }
    Ok(s)
}
fn header_hash(h: &BlockHeader) -> [u8; 32] {
    onx_primitives::domain_hash(&BlockHeader::DOMAIN_TAG, &h.to_bytes())
}
fn hex(v: &[u8]) -> String {
    v.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(v: &str) -> Result<Vec<u8>, StorageError> {
    if !v.len().is_multiple_of(2) {
        return Err(StorageError::Corrupt(
            "invalid hexadecimal storage key".into(),
        ));
    }
    v.as_bytes()
        .chunks(2)
        .map(|p| {
            u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16)
                .map_err(|_| StorageError::Corrupt("invalid hexadecimal storage data".into()))
        })
        .collect()
}
