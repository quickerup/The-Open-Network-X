use bip39::{Language, Mnemonic};
use clap::{Parser, Subcommand};
use onx_primitives::SecretKey;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MnemonicEntry {
    pub words: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletCreateResponse {
    pub wallet: String,
    pub mnemonic: Vec<String>,
    pub public_key_hex: String,
}

#[derive(Debug, Clone, Parser)]
#[command(name = "onx-cli", version = "0.1.0")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    #[command(name = "wallet")]
    Wallet {
        #[command(subcommand)]
        kind: WalletCommand,
    },
    #[command(name = "transfer")]
    Transfer {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        amount: u64,
        #[arg(long)]
        rpc: Option<String>,
    },
    #[command(name = "deploy-contract")]
    DeployContract {
        #[arg(long)]
        from: String,
        #[arg(long)]
        code: String,
        #[arg(long)]
        rpc: Option<String>,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum WalletCommand {
    #[command(name = "create")]
    Create,
    #[command(name = "balance")]
    Balance { address: String },
}

pub fn mnemonic_words() -> Result<Vec<String>, String> {
    let mnemonic = Mnemonic::generate(12).map_err(|err| err.to_string())?;
    Ok(mnemonic.words().map(|word| word.to_string()).collect())
}

pub fn generate_mnemonic() -> Result<MnemonicEntry, String> {
    let words = mnemonic_words()?;
    Ok(MnemonicEntry { words })
}

pub fn derive_ed25519_key_from_mnemonic(words: &[String]) -> Result<SecretKey, String> {
    let phrase = words.join(" ");
    let mnemonic = Mnemonic::parse_in_normalized(Language::English, &phrase)
        .map_err(|err| err.to_string())?;
    let seed = mnemonic.to_seed_normalized("");
    let mut key = [0u8; 32];
    key.copy_from_slice(&seed[..32]);
    SecretKey::from_seed(&key).map_err(|err| err.to_string())
}

pub fn wallet_create(out_dir: impl AsRef<Path>) -> Result<WalletCreateResponse, String> {
    let mnemonic = generate_mnemonic()?;
    let words = mnemonic.words.clone();
    let secret = derive_ed25519_key_from_mnemonic(&words)?;
    let public_key = secret.public_key();
    let pub_hex = hex::encode(public_key.encode());
    let pub_hex_for_file = pub_hex.clone();

    fs::create_dir_all(out_dir.as_ref()).map_err(|err| err.to_string())?;
    fs::write(
        out_dir.as_ref().join("wallet.json"),
        serde_json::to_string_pretty(&WalletCreateResponse {
            wallet: "wallet-000".to_string(),
            mnemonic: words.clone(),
            public_key_hex: pub_hex_for_file,
        })
        .map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())?;

    Ok(WalletCreateResponse {
        wallet: "wallet-000".to_string(),
        mnemonic: words,
        public_key_hex: pub_hex,
    })
}

pub fn serialize_boc_transfer(from: &str, to: &str, amount: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"ONX_BOC_TRANSFER");
    bytes.extend_from_slice(from.as_bytes());
    bytes.extend_from_slice(b"\0");
    bytes.extend_from_slice(to.as_bytes());
    bytes.extend_from_slice(b"\0");
    bytes.extend_from_slice(&amount.to_le_bytes());
    Ok(bytes)
}

pub fn balance_query(addr: &str) -> Result<String, String> {
    Ok(format!("balance_for:{}", addr))
}

pub fn deploy_contract_boc(from: &str, code: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"ONX_BOC_DEPLOY");
    bytes.extend_from_slice(from.as_bytes());
    bytes.extend_from_slice(b"\0");
    bytes.extend_from_slice(code.as_bytes());
    bytes.extend_from_slice(b"\0");
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wallet_create_and_transfer_boc_round_trip() {
        let res = wallet_create(std::env::temp_dir().join("onx-cli-test-wallets")).unwrap();
        assert_eq!(res.mnemonic.len(), 12);
        let boc = serialize_boc_transfer("alice", "bob", 42).unwrap();
        assert!(boc.starts_with(b"ONX_BOC_TRANSFER"));
    }
}
