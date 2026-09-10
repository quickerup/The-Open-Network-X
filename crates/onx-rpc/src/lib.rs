use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use onx_state_model::{AccountState, BagOfCells, MerkleProof};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    pub bind: String,
    pub cors_allowed_origin: String,
    pub max_rps: u64,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8080".to_string(),
            cors_allowed_origin: "*".to_string(),
            max_rps: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAccountStateRequest {
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub raw_boc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestBlockRequest {
    pub shard: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimateFeeRequest {
    pub raw_boc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub ok: bool,
    pub result: Value,
}

#[derive(Debug, Clone)]
pub struct RpcGateway {
    pub config: RpcConfig,
    pub storage: Arc<RwLock<HashMap<String, AccountState>>>,
    pub throttle: Arc<RwLock<HashMap<String, u64>>>,
}

impl RpcGateway {
    pub fn new(config: RpcConfig) -> Self {
        Self {
            config,
            storage: Arc::new(RwLock::new(HashMap::new())),
            throttle: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn serve(self) -> Result<(), String> {
        let bind = self.config.bind.clone();
        let addr: SocketAddr = bind
            .parse::<SocketAddr>()
            .map_err(|err| err.to_string())?;
        let app = Router::new()
            .route("/rpc", post(rpc_json))
            .route("/lite", post(lite_binary))
            .route("/health", get(health))
            .layer(CorsLayer::permissive())
            .with_state(self);
        let listener = TcpListener::bind(addr).await.map_err(|err| err.to_string())?;
        let server = axum::serve(listener, app.into_make_service());
        server.await.map_err(|err| err.to_string())
    }

    pub fn get_account_state(&self, address: &str) -> Result<Value, String> {
        let map = self.storage.read().unwrap();
        let key = address.to_string();
        if let Some(state) = map.get(&key) {
            Ok(json!({
                "balance": 0,
                "address": address,
                "account_state": format!("{:?}", state),
            }))
        } else {
            Ok(json!({
                "balance": 0,
                "address": address,
                "account_state": "uninitialized",
            }))
        }
    }

    pub fn send_message(&self, raw_boc: &str) -> Result<Value, String> {
        if raw_boc.trim().is_empty() {
            return Err("empty raw_boc payload".to_string());
        }
        if self.throttle.read().unwrap().get(raw_boc).copied().unwrap_or(0) >= self.config.max_rps {
            return Err("rate limit exceeded".to_string());
        }
        Ok(json!({
            "accepted": true,
            "message_hash": "sha256:deterministic-synthetic",
            "fee": 0,
        }))
    }

    pub fn get_latest_block(&self) -> Result<Value, String> {
        Ok(json!({
            "block": 0,
            "workchain": 0,
            "shard": "0x8000_0000_0000_0000",
            "root_hash": "0x00",
        }))
    }

    pub fn estimate_fee(&self, raw_boc: &str) -> Result<Value, String> {
        let _ = raw_boc;
        Ok(json!({
            "fee": 0,
            "gas_used": 0,
            "accepted": true,
        }))
    }
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "ok": true })))
}

async fn rpc_json(
    State(state): State<RpcGateway>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let method = payload.get("method").and_then(|v| v.as_str()).unwrap_or_default();
    let params = payload.get("params").cloned().unwrap_or(json!({}));

    let response = match method {
        "getAccountState" => {
            let address = params.get("address").and_then(|v| v.as_str()).unwrap_or_default();
            state.get_account_state(address)
        }
        "sendMessage" => {
            let raw_boc = params.get("raw_boc").and_then(|v| v.as_str()).unwrap_or_default();
            state.send_message(raw_boc)
        }
        "getLatestBlock" => {
            state.get_latest_block()
        }
        "estimateFee" => {
            let raw_boc = params.get("raw_boc").and_then(|v| v.as_str()).unwrap_or_default();
            state.estimate_fee(raw_boc)
        }
        _ => Ok(json!({ "error": "unknown_method" })),
    };

    match response {
        Ok(result) => (StatusCode::OK, Json(RpcResponse { ok: true, result })).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(RpcResponse { ok: false, result: json!({"error": err}) })).into_response(),
    }
}

async fn lite_binary(State(_state): State<RpcGateway>) -> impl IntoResponse {
    let proof = MerkleProof {
        magic_bytes: 0x4D505246,
        target_key: [0u8; 32],
        root_hash: [0u8; 32],
        proof_boc: BagOfCells::from_root(onx_state_model::Cell::new(vec![], vec![]).unwrap()).unwrap(),
    };
    let bytes = proof.to_bytes();
    (StatusCode::OK, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_gateway_shape_compiles() {
        let cfg = RpcConfig::default();
        let gateway = RpcGateway::new(cfg);
        assert!(gateway.get_latest_block().is_ok());
    }
}
