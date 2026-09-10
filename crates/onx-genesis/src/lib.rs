use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct Balance {
    pub address: String,
    pub amount: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Validator {
    pub public_key: String,
    pub stake: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Workchain {
    pub id: u32,
    pub name: String,
    pub shard_prefix: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenesisConfig {
    pub balances: Vec<Balance>,
    pub validators: Vec<Validator>,
    pub workchains: Vec<Workchain>,
}

impl Default for GenesisConfig {
    fn default() -> Self {
        Self {
            balances: vec![Balance {
                address: "onx:genesis-account".to_string(),
                amount: 5_000_000_000_000_000_000,
            }],
            validators: vec![Validator {
                public_key: "validator-pubkey-00".to_string(),
                stake: 1_000_000,
            }],
            workchains: vec![Workchain {
                id: 0,
                name: "masterchain".to_string(),
                shard_prefix: "0x00".to_string(),
                enabled: true,
            }],
        }
    }
}

pub fn secure_output_dir(output: impl AsRef<Path>) -> Result<PathBuf, String> {
    let out = output.as_ref();
    if out.is_absolute() {
        return Err("onx-genesis failed: --out must be a relative path within the current directory".to_string());
    }

    for comp in out.components() {
        match comp {
            Component::RootDir | Component::Prefix(_) | Component::ParentDir => {
                return Err("onx-genesis failed: --out path must not contain parent, root, or absolute path components".to_string())
            }
            _ => {}
        }
    }

    let root = env::current_dir().map_err(|err| format!("onx-genesis failed: could not determine current directory: {err}"))?;
    let mut out_path = root.clone();
    for comp in out.components() {
        match comp {
            Component::CurDir => {}
            Component::Normal(part) => out_path.push(part),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("onx-genesis failed: --out path must not contain parent, root, or absolute path components".to_string())
            }
        }
    }

    fs::create_dir_all(&out_path)
        .map_err(|err| format!("onx-genesis failed: could not create output directory: {err}"))?;

    let root_canon = fs::canonicalize(&root)
        .map_err(|err| format!("onx-genesis failed: could not canonicalize root: {err}"))?;
    let out_canon = fs::canonicalize(&out_path)
        .map_err(|err| format!("onx-genesis failed: could not canonicalize output directory: {err}"))?;

    if !out_canon.starts_with(&root_canon) {
        return Err("onx-genesis failed: output directory must be within the current directory".to_string());
    }

    Ok(out_canon)
}
pub fn parse_config(path: impl AsRef<Path>) -> Result<GenesisConfig, String> {
    let raw = fs::read_to_string(path.as_ref())
        .map_err(|err| format!("failed to read genesis config {}: {err}", path.as_ref().display()))?;
    let config: GenesisConfig = toml::from_str(&raw).map_err(|err| err.to_string())?;
    Ok(config)
}

pub fn generate_genesis(config: &GenesisConfig, output_dir: PathBuf) -> Result<(), String> {
    fs::create_dir_all(&output_dir)
        .map_err(|err| format!("failed to create output dir {}: {err}", output_dir.display()))?;

    let mut balances = String::new();
    for balance in &config.balances {
        balances.push_str(&format!("{}:{};", balance.address, balance.amount));
    }

    let mut validators = String::new();
    for validator in &config.validators {
        validators.push_str(&format!("{}:{};", validator.public_key, validator.stake));
    }

    let mut workchains = String::new();
    for wc in &config.workchains {
        workchains.push_str(&format!("{}:{}:{}:{};", wc.id, wc.name, wc.shard_prefix, wc.enabled));
    }

    let gateway = format!(
        "ONX_GENESIS_BOC\nmasterchain_genesis#0\nvalidator_keys={validators}\ninitial_balances={balances}\nworkchains={workchains}\n"
    );

    let genesis_boc_path = output_dir.join("genesis.boc");
    fs::write(&genesis_boc_path, &gateway).map_err(|err| err.to_string())?;

    let shard_header = format!(
        "ONX_SHARD_HEADER_BOC\nshard=0x00\nworkchain=0\nparent=masterchain_genesis#0\n"
    );
    fs::write(output_dir.join("shard-header-0.boc"), shard_header).map_err(|err| err.to_string())?;

    for idx in 0..4 {
        let node_cfg = format!(
            "role = \"validator\"\nstorage_path = \"target/onxd-node-{}\"\nnetwork_enabled = true\nnetwork_bind = \"127.0.0.1:{}000\"\npeers = \"127.0.0.1:{}001\"\nbootstrap_genesis = \"{}\"\n",
            idx,
            idx + 1,
            idx + 1,
            genesis_boc_path.display()
        );
        fs::write(output_dir.join(format!("node-{}.toml", idx)), node_cfg).map_err(|err| err.to_string())?;
    }

    Ok(())
}

pub fn write_docs(output_dir: PathBuf) -> Result<(), String> {
    let doc = "# Launch Guide\n\nThis repository ships a deterministic `onx-genesis` bootstrap generator.\n\nUse `onx-genesis --config genesis.toml --out target/onx-genesis` to create `genesis.boc` and four `node-*.toml` files.\n\nThen launch four `onxd` boot nodes from the same generated `genesis.boc` by pointing each node at its generated configuration file, verifying that the first committed masterchain block is `#0` and that a shared shard header bootstrap path is emitted.\n";
    fs::write(output_dir.join("launch_guide.md"), doc).map_err(|err| err.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::secure_output_dir;
    use std::env;
    use std::path::Path;

    #[test]
    fn secure_output_dir_rejects_parent_components() {
        let result = secure_output_dir(Path::new("../../escape"));
        assert!(result.is_err(), "expected traversal path to be rejected");
    }

    #[test]
    fn secure_output_dir_accepts_relative_path_inside_workspace() {
        let cwd = env::current_dir().unwrap();
        let result = secure_output_dir(Path::new("target/onx-genesis-test"));
        assert!(result.is_ok(), "expected safe relative path to be accepted");
        let out = result.unwrap();
        assert!(out.starts_with(cwd));
    }
}
