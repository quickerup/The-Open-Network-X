use prometheus::{
    register_counter, register_gauge, register_histogram, Counter, Encoder, Gauge, Histogram,
    TextEncoder,
};
use std::net::SocketAddr;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::runtime::Handle;
use tracing::{span, Level, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryConfig {
    pub metrics_bind: String,
    pub enable_tracing: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            metrics_bind: "127.0.0.1:9100".to_string(),
            enable_tracing: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TelemetryHandle {
    pub block_import_duration_ms: Histogram,
    pub block_height: Gauge,
    pub connected_peer_count: Gauge,
    pub transaction_pool_size: Gauge,
    pub vm_gas_rate: Histogram,
    pub cross_shard_router: Counter,
    pub consensus_rounds: Counter,
}

impl TelemetryHandle {
    pub fn new() -> Result<Self, String> {
        let block_import_duration_ms = register_histogram!(
            "block_import_duration_ms",
            "Histogram of block import duration in milliseconds"
        )
        .map_err(|err| err.to_string())?;
        let block_height = register_gauge!(
            "current_block_height",
            "Latest committed block height tracked by the local node"
        )
        .map_err(|err| err.to_string())?;
        let connected_peer_count = register_gauge!(
            "connected_peer_count",
            "Current number of connected peers"
        )
        .map_err(|err| err.to_string())?;
        let transaction_pool_size = register_gauge!(
            "transaction_pool_size",
            "Current size of the transaction pool"
        )
        .map_err(|err| err.to_string())?;
        let vm_gas_rate = register_histogram!(
            "vm_gas_execution_rate",
            "Distribution of VM gas execution rates"
        )
        .map_err(|err| err.to_string())?;
        let cross_shard_router = register_counter!(
            "cross_shard_message_routing_total",
            "Count of cross-shard route spans and routing events"
        )
        .map_err(|err| err.to_string())?;
        let consensus_rounds = register_counter!(
            "consensus_round_phases_total",
            "Count of consensus round phases observed by the node"
        )
        .map_err(|err| err.to_string())?;

        Ok(Self {
            block_import_duration_ms,
            block_height,
            connected_peer_count,
            transaction_pool_size,
            vm_gas_rate,
            cross_shard_router,
            consensus_rounds,
        })
    }

    pub fn record_block_import_duration(&self, millis: f64) {
        self.block_import_duration_ms.observe(millis);
    }

    pub fn set_block_height(&self, height: i64) {
        self.block_height.set(height as f64);
    }

    pub fn set_connected_peers(&self, count: i64) {
        self.connected_peer_count.set(count as f64);
    }

    pub fn set_tx_pool_size(&self, size: i64) {
        self.transaction_pool_size.set(size as f64);
    }

    pub fn record_gas_execution(&self, gas: f64) {
        self.vm_gas_rate.observe(gas);
    }

    pub fn track_route_span(&self, shard_from: &str, shard_to: &str) -> Span {
        let span = span!(Level::INFO, "cross_shard_route", from=%shard_from, to=%shard_to);
        self.cross_shard_router.inc();
        span
    }

    pub fn track_consensus_phase(&self, phase: &str) -> Span {
        let span = span!(Level::INFO, "consensus_phase", phase=%phase);
        self.consensus_rounds.inc();
        span
    }

    pub fn metrics_text(&self) -> Result<String, String> {
        let mut buffer = Vec::new();
        let encoder = TextEncoder::new();
        let metric_families = prometheus::default_registry().gather();
        encoder.encode(&metric_families, &mut buffer).map_err(|err| err.to_string())?;
        Ok(String::from_utf8_lossy(&buffer).into_owned())
    }
}

pub async fn serve_metrics(config: TelemetryConfig) -> Result<(), String> {
    let addr: SocketAddr = config
        .metrics_bind
        .parse::<SocketAddr>()
        .map_err(|err| err.to_string())?;
    let listener = TcpListener::bind(addr).await.map_err(|err| err.to_string())?;
    let registry = prometheus::default_registry();
    let _ = registry;
    let server = Handle::current();
    let _ = server;
    loop {
        let (mut stream, _) = listener.accept().await.map_err(|err| err.to_string())?;
        let metric_families = prometheus::default_registry().gather();
        let encoder = TextEncoder::new();
        let mut buf = Vec::new();
        encoder.encode(&metric_families, &mut buf).map_err(|err| err.to_string())?;
        let payload = String::from_utf8_lossy(&buf);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.9; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
            payload.len(),
            payload
        );
        let _ = stream.write_all(response.as_bytes()).await;
    }
}
