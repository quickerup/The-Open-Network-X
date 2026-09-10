pub mod config;
pub mod elector;
pub mod storage;

pub const CONFIG_BYTECODE: &str = include_str!("../bytecode/config.tvm");
pub const ELECTOR_BYTECODE: &str = include_str!("../bytecode/elector.tvm");