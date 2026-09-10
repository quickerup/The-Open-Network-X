#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigParameters {
    pub gas_per_cell: u64,
    pub consensus_timeout_ms: u64,
    pub validator_cap: usize,
}

impl Default for ConfigParameters {
    fn default() -> Self {
        Self {
            gas_per_cell: 1_000,
            consensus_timeout_ms: 1000,
            validator_cap: 100,
        }
    }
}
