use onx_consensus::{ConsensusEngine, RoundTimeouts, ValidatorSetEntry};
use onx_data_structures::{ShardIdent, WorkchainIdent};
use onx_execution::ExecutionContext;
use onx_networking::{
    AdnlTransportNode, DhtContact, DhtDaemon, DhtRpc, DhtRpcResponse, DhtTransport,
    NetworkError, RldpConfig, RldpSender,
};
use onx_primitives::{SecretKey, Uint64, Uint256};
use onx_state_model::StateStorage;
use onx_telemetry::{serve_metrics, TelemetryConfig, TelemetryHandle};
use std::future::Future;
use std::net::SocketAddr;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use tokio::time::{interval, sleep};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    FullNode,
    ValidatorNode,
    LiteServerNode,
}

impl NodeRole {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "full" | "full-node" | "fullnode" => Ok(Self::FullNode),
            "validator" | "validator-node" | "validatornode" => Ok(Self::ValidatorNode),
            "lite" | "lite-server" | "liteserver" | "lite-server-node" => Ok(Self::LiteServerNode),
            _ => Err(format!("unknown node role: {s}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnxdConfig {
    pub role: NodeRole,
    pub storage_path: String,
    pub network_enabled: bool,
    pub network_bind: String,
    pub peers: Vec<String>,
    pub shutdown_after_ms: Option<u64>,
}

impl Default for OnxdConfig {
    fn default() -> Self {
        Self {
            role: NodeRole::FullNode,
            storage_path: "./onx-data".to_string(),
            network_enabled: true,
            network_bind: "127.0.0.1:0".to_string(),
            peers: Vec::new(),
            shutdown_after_ms: None,
        }
    }
}

pub fn load_config(path: impl AsRef<Path>) -> Result<OnxdConfig, String> {
    let raw = fs::read_to_string(path.as_ref())
        .map_err(|err| format!("failed to read config file {}: {err}", path.as_ref().display()))?;

    let mut role = NodeRole::FullNode;
    let mut storage_path = "./onx-data".to_string();
    let mut network_enabled = true;
    let mut network_bind = "127.0.0.1:0".to_string();
    let mut peers = Vec::new();
    let mut shutdown_after_ms = None;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            match key {
                "role" => role = NodeRole::from_str(value)? ,
                "storage_path" => storage_path = value.to_string(),
                "network_enabled" => network_enabled = matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"),
                "network_bind" => network_bind = value.to_string(),
                "peers" => {
                    peers = value.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
                }
                "shutdown_after_ms" => {
                    shutdown_after_ms = Some(value.parse::<u64>().map_err(|_| format!("invalid shutdown_after_ms value: {value}"))?);
                }
                _ => {}
            }
        }
    }

    Ok(OnxdConfig {
        role,
        storage_path,
        network_enabled,
        network_bind,
        peers,
        shutdown_after_ms,
    })
}

pub fn parse_cli_args(args: &[String]) -> Result<OnxdConfig, String> {
    let mut config = OnxdConfig::default();
    let mut config_path = None;

    for idx in 1..args.len() {
        match args[idx].as_str() {
            "--config" => {
                if idx + 1 >= args.len() {
                    return Err("--config requires a path".to_string());
                }
                config_path = Some(args[idx + 1].clone());
            }
            _ => {}
        }
    }

    if let Some(path) = config_path {
        config = load_config(path)?;
    }

    let mut idx = 1;
    while idx < args.len() {
        match args[idx].as_str() {
            "--role" => {
                idx += 1;
                if idx >= args.len() {
                    return Err("--role requires a value".to_string());
                }
                config.role = NodeRole::from_str(&args[idx])?;
            }
            "--storage-path" => {
                idx += 1;
                if idx >= args.len() {
                    return Err("--storage-path requires a value".to_string());
                }
                config.storage_path = args[idx].clone();
            }
            "--network-bind" => {
                idx += 1;
                if idx >= args.len() {
                    return Err("--network-bind requires a value".to_string());
                }
                config.network_bind = args[idx].clone();
            }
            "--peer" => {
                idx += 1;
                if idx >= args.len() {
                    return Err("--peer requires a value".to_string());
                }
                config.peers.push(args[idx].clone());
            }
            "--help" | "-h" => {
                return Err("usage: onxd --config onxd.toml [--role full|validator|lite]".to_string());
            }
            _ => {}
        }
        idx += 1;
    }
    Ok(config)
}

#[derive(Clone)]
struct NullDhtTransport;

impl DhtTransport for NullDhtTransport {
    fn call<'a>(
        &'a self,
        _recipient: DhtContact,
        _request: DhtRpc,
    ) -> Pin<Box<dyn Future<Output = Result<DhtRpcResponse, NetworkError>> + Send + 'a>> {
        Box::pin(async { Ok(DhtRpcResponse::Pong) })
    }
}

pub async fn run_daemon(config: OnxdConfig) -> Result<(), String> {
    fs::create_dir_all(&config.storage_path)
        .map_err(|err| format!("failed to initialize storage path {}: {err}", config.storage_path))?;

    let storage = StateStorage::open(&config.storage_path)
        .map_err(|err| format!("failed to initialize state storage: {err}"))?;
    let _ = storage;

    let metrics = TelemetryHandle::new().map_err(|err| err.to_string())?;
    metrics.set_block_height(0);
    metrics.set_connected_peers(0);
    metrics.set_tx_pool_size(0);

    let metrics_cfg = TelemetryConfig::default();
    let metrics_task = tokio::spawn(async move {
        let _ = serve_metrics(metrics_cfg).await;
    });

    let start = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let _ = start;

    let runtime_shutdown = if let Some(ms) = config.shutdown_after_ms {
        Some(sleep(Duration::from_millis(ms)))
    } else {
        None
    };

    let mut network_loop = None;
    if config.network_enabled {
        let bind = config.network_bind.clone();
        let role = config.role;
        let peers = config.peers.clone();
        let _ = (bind.clone(), role, peers);

        let addr = bind
            .parse::<SocketAddr>()
            .map_err(|err| format!("invalid network bind address: {err}"))?;
        let seed = [1u8; 32];
        let secret_key = SecretKey::from_seed(&seed)
            .map_err(|err| format!("failed to build deterministic secret key: {err}"))?;
        let public_key = secret_key.public_key();
        let adnl = AdnlTransportNode::bind(secret_key, addr)
            .await
            .map_err(|err| format!("failed to bind ADNL transport: {err}"))?;
        let _public = public_key;
        let dht = DhtDaemon::new(public_key, Arc::new(NullDhtTransport));
        let _ = dht;

        let shard = ShardIdent::root(WorkchainIdent::BASIC);
        let exec_context = ExecutionContext {
            gen_utime: 0,
            start_lt: 0,
            end_lt: 1,
            gas_limit: 1_000_000,
        };
        let _ = exec_context;

        let validator_entries = vec![ValidatorSetEntry {
            validator_id: 0,
            public_key,
            actual_stake: Uint64::from(1),
        }];
        let engine = ConsensusEngine::new(
            shard,
            0,
            validator_entries,
            0,
            RoundTimeouts::default(),
        )
        .map_err(|err| format!("failed to initialize consensus engine: {err}"))?;
        let _ = engine;

        let _rldp = RldpSender::new(
            Uint256([0u8; 32]),
            &[0u8; 1],
            RldpConfig::default(),
        )
        .map_err(|err| format!("failed to initialize RLDP sender: {err}"))?;

        let handle = tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(10));
            let _ = &adnl;
            let _ = &dht;
=======
        let _ = (bind, role, peers);

        let handle = tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(10));
>>>>>>> origin/main
            loop {
                ticker.tick().await;
                let _ = "network-loop";
            }
        });
        network_loop = Some(handle);
    }

    let shutdown = tokio::signal::ctrl_c();
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|err| format!("failed to install SIGTERM watcher: {err}"))?;

    let outcome = tokio::select! {
        _ = shutdown => {
            if let Some(handle) = network_loop {
                handle.abort();
            }
            Ok(())
        }
        _ = sigterm.recv() => {
            if let Some(handle) = network_loop {
                handle.abort();
            }
            Ok(())
        }
        _ = async {
            if let Some(timer) = runtime_shutdown {
                timer.await;
            } else {
                std::future::pending::<()>().await;
            }
        } => {
            if let Some(handle) = network_loop {
                handle.abort();
            }
            Ok(())
        }
    };

    metrics_task.abort();
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parsing_and_role_mapping_round_trip() {
        let tmp = std::env::temp_dir().join(format!("onxd-config-{}.toml", std::process::id()));
        fs::write(&tmp, "role = \"validator\"\nstorage_path = \"./state\"\nnetwork_enabled = true\nnetwork_bind = \"127.0.0.1:9001\"\npeers = \"p1,p2\"\n").unwrap();
        let config = load_config(&tmp).unwrap();
        assert_eq!(config.role, NodeRole::ValidatorNode);
        assert_eq!(config.storage_path, "./state");
        assert_eq!(config.network_bind, "127.0.0.1:9001");
        assert_eq!(config.peers, vec!["p1", "p2"]);
        let _ = fs::remove_file(tmp);
    }

    #[test]
    fn cli_parse_accepts_role_and_config_file() {
        let cfg = parse_cli_args(&[
            "onxd".to_string(),
            "--role".to_string(),
            "lite".to_string(),
            "--config".to_string(),
            "does-not-exist.toml".to_string(),
        ]);
        assert!(cfg.is_err());

        let parsed = parse_cli_args(&[
            "onxd".to_string(),
            "--role".to_string(),
            "full".to_string(),
        ]).unwrap();
        assert_eq!(parsed.role, NodeRole::FullNode);
    }
}
