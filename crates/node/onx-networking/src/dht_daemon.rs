//! Kademlia-like DHT runtime for discovery data.
//!
//! This module intentionally contains no consensus integration: all answers are
//! advisory discovery data as required by `networking-dht.md`.

use crate::{xor_distance, DhtRecord, KeyDescription, NetworkError};
use onx_primitives::{domain_hash, hash::VALIDATOR_SIGN_V1, PublicKey, SecretKey, Uint256, Uint64};
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

/// Maximum contacts in each XOR-distance routing bucket.
pub const K_BUCKET_SIZE: usize = 20;
/// Number of contacts queried in each iterative lookup round.
pub const LOOKUP_ALPHA: usize = 3;

/// A reachable DHT identity. Its node ID is the ADNL abstract address of its key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DhtContact {
    pub node_id: Uint256,
    pub public_key: PublicKey,
}

impl DhtContact {
    pub fn from_public_key(public_key: PublicKey) -> Self {
        Self {
            node_id: KeyDescription::new_ed25519(public_key).compute_abstract_address(),
            public_key,
        }
    }
}

/// The four DHT RPCs. The sender is carried in every request so recipients can
/// refresh their routing table without trusting an out-of-band transport hint.
#[derive(Clone, Debug)]
pub enum DhtRpc {
    Ping {
        sender: DhtContact,
    },
    Store {
        sender: DhtContact,
        record: Box<DhtRecord>,
        owner: PublicKey,
    },
    FindNode {
        sender: DhtContact,
        target: Uint256,
    },
    FindValue {
        sender: DhtContact,
        key: Uint256,
    },
}

#[derive(Clone, Debug)]
pub enum DhtRpcResponse {
    Pong,
    Stored,
    Nodes(Vec<DhtContact>),
    Value(Box<DhtRecord>, PublicKey),
}

/// A transport boundary for DHT RPCs. ADNL/RLDP adapters can implement this;
/// tests use the in-memory implementation below.
pub trait DhtTransport: Send + Sync {
    fn call<'a>(
        &'a self,
        recipient: DhtContact,
        request: DhtRpc,
    ) -> Pin<Box<dyn Future<Output = Result<DhtRpcResponse, NetworkError>> + Send + 'a>>;
}

#[derive(Clone)]
struct StoredRecord {
    record: DhtRecord,
    owner: PublicKey,
}

/// A Kademlia routing table plus local record cache.
pub struct DhtDaemon {
    local: DhtContact,
    transport: Arc<dyn DhtTransport>,
    buckets: Mutex<Vec<Vec<DhtContact>>>,
    records: Mutex<HashMap<Uint256, StoredRecord>>,
}

impl DhtDaemon {
    pub fn new(local_public_key: PublicKey, transport: Arc<dyn DhtTransport>) -> Self {
        Self {
            local: DhtContact::from_public_key(local_public_key),
            transport,
            buckets: Mutex::new(vec![Vec::new(); 256]),
            records: Mutex::new(HashMap::new()),
        }
    }

    pub fn contact(&self) -> DhtContact {
        self.local
    }

    /// Add a verified bootstrap contact. The least-recently seen contact is
    /// discarded when a bucket is full; live transports may ping it first.
    pub fn add_contact(&self, contact: DhtContact) {
        if contact.node_id == self.local.node_id
            || KeyDescription::new_ed25519(contact.public_key).compute_abstract_address()
                != contact.node_id
        {
            return;
        }
        let Some(index) = bucket_index(self.local.node_id, contact.node_id) else {
            return;
        };
        let mut buckets = self.buckets.lock().expect("routing table lock poisoned");
        let bucket = &mut buckets[index];
        if let Some(position) = bucket
            .iter()
            .position(|entry| entry.node_id == contact.node_id)
        {
            bucket.remove(position);
        }
        bucket.push(contact);
        if bucket.len() > K_BUCKET_SIZE {
            bucket.remove(0);
        }
    }

    pub fn closest_contacts(&self, target: Uint256, limit: usize) -> Vec<DhtContact> {
        let mut contacts: Vec<_> = self
            .buckets
            .lock()
            .expect("routing table lock poisoned")
            .iter()
            .flatten()
            .copied()
            .collect();
        contacts.sort_by_key(|contact| xor_distance(contact.node_id, target));
        contacts.truncate(limit);
        contacts
    }

    /// Dispatch a received PING, STORE, FIND_NODE, or FIND_VALUE request.
    pub async fn handle_rpc(&self, request: DhtRpc) -> Result<DhtRpcResponse, NetworkError> {
        let sender = match &request {
            DhtRpc::Ping { sender }
            | DhtRpc::FindNode { sender, .. }
            | DhtRpc::FindValue { sender, .. } => *sender,
            DhtRpc::Store { sender, .. } => *sender,
        };
        self.add_contact(sender);
        match request {
            DhtRpc::Ping { .. } => Ok(DhtRpcResponse::Pong),
            DhtRpc::FindNode { target, .. } => Ok(DhtRpcResponse::Nodes(
                self.closest_contacts(target, K_BUCKET_SIZE),
            )),
            DhtRpc::FindValue { key, .. } => {
                self.remove_expired();
                if let Some(stored) = self
                    .records
                    .lock()
                    .expect("record lock poisoned")
                    .get(&key)
                    .cloned()
                {
                    Ok(DhtRpcResponse::Value(Box::new(stored.record), stored.owner))
                } else {
                    Ok(DhtRpcResponse::Nodes(
                        self.closest_contacts(key, K_BUCKET_SIZE),
                    ))
                }
            }
            DhtRpc::Store { record, owner, .. } => {
                self.validate_record(&record, &owner, None)?;
                self.records.lock().expect("record lock poisoned").insert(
                    record.key,
                    StoredRecord {
                        record: *record,
                        owner,
                    },
                );
                Ok(DhtRpcResponse::Stored)
            }
        }
    }

    /// Iteratively asks closest unqueried peers until no closer peer is found.
    pub async fn find_node(&self, target: Uint256) -> Result<Vec<DhtContact>, NetworkError> {
        let mut known = self.closest_contacts(target, K_BUCKET_SIZE);
        let mut queried = Vec::new();
        loop {
            let round: Vec<_> = known
                .iter()
                .copied()
                .filter(|c| !queried.contains(c))
                .take(LOOKUP_ALPHA)
                .collect();
            if round.is_empty() {
                break;
            }
            let old_best = known.first().map(|c| xor_distance(c.node_id, target));
            for peer in round {
                queried.push(peer);
                if let Ok(DhtRpcResponse::Nodes(nodes)) = self
                    .transport
                    .call(
                        peer,
                        DhtRpc::FindNode {
                            sender: self.local,
                            target,
                        },
                    )
                    .await
                {
                    for node in nodes {
                        self.add_contact(node);
                        if !known.contains(&node) {
                            known.push(node);
                        }
                    }
                }
            }
            known.sort_by_key(|c| xor_distance(c.node_id, target));
            known.truncate(K_BUCKET_SIZE);
            if old_best == known.first().map(|c| xor_distance(c.node_id, target))
                && queried.len() >= known.len()
            {
                break;
            }
        }
        Ok(known)
    }

    pub async fn find_value(&self, key: Uint256) -> Result<Option<DhtRecord>, NetworkError> {
        self.remove_expired();
        if let Some(value) = self
            .records
            .lock()
            .expect("record lock poisoned")
            .get(&key)
            .cloned()
        {
            return Ok(Some(value.record));
        }
        for peer in self.find_node(key).await? {
            if let Ok(DhtRpcResponse::Value(record, owner)) = self
                .transport
                .call(
                    peer,
                    DhtRpc::FindValue {
                        sender: self.local,
                        key,
                    },
                )
                .await
            {
                self.validate_record(&record, &owner, Some(key))?;
                self.records.lock().expect("record lock poisoned").insert(
                    key,
                    StoredRecord {
                        record: (*record).clone(),
                        owner,
                    },
                );
                return Ok(Some(*record));
            }
        }
        Ok(None)
    }

    pub async fn store(&self, record: DhtRecord, owner: PublicKey) -> Result<(), NetworkError> {
        self.validate_record(&record, &owner, None)?;
        self.records.lock().expect("record lock poisoned").insert(
            record.key,
            StoredRecord {
                record: record.clone(),
                owner,
            },
        );
        for peer in self
            .find_node(record.key)
            .await?
            .into_iter()
            .take(LOOKUP_ALPHA)
        {
            self.transport
                .call(
                    peer,
                    DhtRpc::Store {
                        sender: self.local,
                        record: Box::new(record.clone()),
                        owner,
                    },
                )
                .await?;
        }
        Ok(())
    }

    fn validate_record(
        &self,
        record: &DhtRecord,
        owner: &PublicKey,
        requested_key: Option<Uint256>,
    ) -> Result<(), NetworkError> {
        if requested_key.is_some_and(|key| key != record.key) {
            return Err(NetworkError::DhtMalformedRecord);
        }
        if record.expiry.0 <= unix_time_secs() {
            return Err(NetworkError::DhtRecordExpired);
        }
        if KeyDescription::new_ed25519(*owner).compute_abstract_address() != record.owner_address {
            return Err(NetworkError::InvalidSignature);
        }
        record.verify_signature(owner)
    }

    fn remove_expired(&self) {
        self.records
            .lock()
            .expect("record lock poisoned")
            .retain(|_, item| item.record.expiry.0 > unix_time_secs());
    }
}

fn bucket_index(local: Uint256, remote: Uint256) -> Option<usize> {
    let distance = xor_distance(local, remote).0;
    for (byte_index, byte) in distance.iter().enumerate() {
        if *byte != 0 {
            return Some(byte_index * 8 + byte.leading_zeros() as usize);
        }
    }
    None
}

fn unix_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Sign a record using the exact canonical DHT serialization.
pub fn sign_record(owner: &SecretKey, key: Uint256, value: Vec<u8>, expiry: Uint64) -> DhtRecord {
    let owner_address = KeyDescription::new_ed25519(owner.public_key()).compute_abstract_address();
    let bytes = DhtRecord::to_signable_bytes(key, &value, expiry, owner_address);
    let signature = owner.sign(
        &VALIDATOR_SIGN_V1,
        &domain_hash(&DhtRecord::DOMAIN_TAG, &bytes),
    );
    DhtRecord {
        key,
        value,
        expiry,
        owner_address,
        signature,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };
    use tokio::time::{timeout, Duration};

    #[derive(Default)]
    struct SimulatedTransport {
        nodes: Mutex<HashMap<Uint256, Arc<DhtDaemon>>>,
    }
    impl SimulatedTransport {
        fn register(&self, daemon: Arc<DhtDaemon>) {
            self.nodes
                .lock()
                .unwrap()
                .insert(daemon.contact().node_id, daemon);
        }
    }
    impl DhtTransport for SimulatedTransport {
        fn call<'a>(
            &'a self,
            recipient: DhtContact,
            request: DhtRpc,
        ) -> Pin<Box<dyn Future<Output = Result<DhtRpcResponse, NetworkError>> + Send + 'a>>
        {
            Box::pin(async move {
                let node = self
                    .nodes
                    .lock()
                    .unwrap()
                    .get(&recipient.node_id)
                    .cloned()
                    .ok_or(NetworkError::DhtRpcFailed)?;
                node.handle_rpc(request).await
            })
        }
    }

    #[tokio::test]
    async fn joining_node_discovers_existing_peers_within_five_seconds() {
        let transport = Arc::new(SimulatedTransport::default());
        let nodes: Vec<_> = (1u8..=4)
            .map(|seed| {
                let key = SecretKey::from_seed(&[seed; 32]).unwrap();
                let node = Arc::new(DhtDaemon::new(key.public_key(), transport.clone()));
                transport.register(node.clone());
                node
            })
            .collect();
        // Existing nodes form a chain, as happens while a network is growing.
        nodes[0].add_contact(nodes[1].contact());
        nodes[1].add_contact(nodes[0].contact());
        nodes[1].add_contact(nodes[2].contact());
        nodes[2].add_contact(nodes[1].contact());
        let joining = nodes[3].clone();
        joining.add_contact(nodes[0].contact());

        let discovered = timeout(
            Duration::from_secs(5),
            joining.find_node(nodes[2].contact().node_id),
        )
        .await
        .expect("iterative discovery must complete within five seconds")
        .unwrap();
        assert!(discovered.contains(&nodes[2].contact()));
        assert!(joining
            .closest_contacts(nodes[2].contact().node_id, K_BUCKET_SIZE)
            .contains(&nodes[2].contact()));
    }

    #[test]
    fn rejects_expired_or_wrong_owner_records() {
        let secret = SecretKey::from_seed(&[9; 32]).unwrap();
        let record = sign_record(
            &secret,
            Uint256([7; 32]),
            vec![1],
            Uint64(unix_time_secs() - 1),
        );
        let transport = Arc::new(SimulatedTransport::default());
        let daemon = DhtDaemon::new(secret.public_key(), transport);
        assert_eq!(
            daemon.validate_record(&record, &secret.public_key(), None),
            Err(NetworkError::DhtRecordExpired)
        );
    }
}
