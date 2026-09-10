#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageRootConfig {
    pub workchain_id: i32,
    pub shard_prefix: String,
    pub gas_rate: u64,
}

impl Default for StorageRootConfig {
    fn default() -> Self {
        Self {
            workchain_id: 0,
            shard_prefix: "0x8000_0000_0000_0000".to_string(),
            gas_rate: 1_000,
        }
    }
}
