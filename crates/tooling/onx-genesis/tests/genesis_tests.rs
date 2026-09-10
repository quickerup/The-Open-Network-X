use onx_genesis::{generate_genesis, GenesisConfig};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn emits_canonical_genesis_and_bootstrap_files() {
    let temp = std::env::temp_dir().join(format!(
        "onx-genesis-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    let _ = fs::remove_dir_all(&temp);

    let cfg = GenesisConfig {
        balances: vec![
            onx_genesis::Balance {
                address: "onx:alice".to_string(),
                amount: 1_000_000,
            },
            onx_genesis::Balance {
                address: "onx:bob".to_string(),
                amount: 2_000_000,
            },
        ],
        validators: vec![onx_genesis::Validator {
            public_key: "validator-01".to_string(),
            stake: 1_000,
        }],
        workchains: vec![onx_genesis::Workchain {
            id: 0,
            name: "masterchain".to_string(),
            shard_prefix: "0x00".to_string(),
            enabled: true,
        }],
    };

    generate_genesis(&cfg, temp.clone()).unwrap();
    assert!(temp.join("genesis.boc").exists());
    assert!(temp.join("shard-header-0.boc").exists());
    assert!(temp.join("node-0.toml").exists());
    assert!(temp.join("node-1.toml").exists());
    assert!(temp.join("node-2.toml").exists());
    assert!(temp.join("node-3.toml").exists());

    let generated = fs::read_to_string(temp.join("genesis.boc")).unwrap();
    assert!(generated.contains("masterchain_genesis#0"));
    assert!(generated.contains("initial_balances="));
    assert!(generated.contains("validator_keys="));

    let _ = fs::remove_dir_all(temp);
}
