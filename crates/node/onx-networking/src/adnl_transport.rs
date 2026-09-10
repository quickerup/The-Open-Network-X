//! ADNL UDP Transport Layer implementation.
//!
//! Complies with `docs/specification/networking-adnl.md` and `ADR-0009-networking-adnl-and-rldp.md`.

use crate::{derive_channel_id, KeyDescription, NetworkError};
use aes::cipher::{KeyIvInit, StreamCipher};
use ed25519_dalek::VerifyingKey;
use onx_primitives::{domain_hash, DomainTag, PublicKey, SecretKey, Signature, Uint256};
use rand::RngCore;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::net::UdpSocket;
use x25519_dalek::StaticSecret;

type Aes256Ctr64BE = ctr::Ctr64BE<aes::Aes256>;

pub const ADNL_SHARED_SECRET_TAG: DomainTag = DomainTag::from_ascii("ONX_ADNL_SHARED_SECRET_V1");

/// Converts an Ed25519 public key into an X25519 Montgomery public key.
pub fn ed25519_to_x25519_public(pk: &PublicKey) -> Result<x25519_dalek::PublicKey, NetworkError> {
    let vk = VerifyingKey::from_bytes(&pk.encode()).map_err(|_| NetworkError::InvalidSignature)?;
    Ok(x25519_dalek::PublicKey::from(vk.to_montgomery().to_bytes()))
}

/// Converts an Ed25519 secret key into an X25519 static secret key using its scalar representation.
pub fn ed25519_to_x25519_secret(sk: &SecretKey) -> StaticSecret {
    StaticSecret::from(sk.to_scalar_bytes())
}

/// Derive symmetric key (32 bytes) and IV (16 bytes) from shared secret and nonce.
pub fn derive_symmetric_key_iv(shared_secret: &[u8; 32], nonce: &[u8; 32]) -> ([u8; 32], [u8; 16]) {
    let mut key_input = Vec::with_capacity(64);
    key_input.extend_from_slice(shared_secret);
    key_input.extend_from_slice(nonce);

    let key_hash = domain_hash(&ADNL_SHARED_SECRET_TAG, &key_input);

    let mut iv_input = Vec::with_capacity(64);
    iv_input.extend_from_slice(nonce);
    iv_input.extend_from_slice(shared_secret);

    let iv_hash = domain_hash(&ADNL_SHARED_SECRET_TAG, &iv_input);

    let mut key = [0u8; 32];
    key.copy_from_slice(&key_hash);

    let mut iv = [0u8; 16];
    iv.copy_from_slice(&iv_hash[..16]);

    (key, iv)
}

/// Encrypt or decrypt payload in-place with AES-256-CTR using key and iv.
pub fn apply_aes256_ctr(key: &[u8; 32], iv: &[u8; 16], buf: &mut [u8]) {
    let mut cipher = Aes256Ctr64BE::new(key.into(), iv.into());
    cipher.apply_keystream(buf);
}

/// Represents a Full Public-Key Encrypted Packet (§4.2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullPacket {
    pub recipient_address: Uint256,
    pub sender_address: Uint256,
    pub timestamp: u32,
    pub nonce: u32,
    pub payload: Vec<u8>,
    pub signature: Signature,
}

impl FullPacket {
    pub fn to_signable_bytes(
        sender_address: Uint256,
        timestamp: u32,
        nonce: u32,
        payload: &[u8],
    ) -> Vec<u8> {
        let mut buf = Vec::with_capacity(32 + 4 + 4 + 2 + payload.len());
        buf.extend_from_slice(&sender_address.encode());
        buf.extend_from_slice(&timestamp.to_be_bytes());
        buf.extend_from_slice(&nonce.to_be_bytes());
        buf.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        buf.extend_from_slice(payload);
        buf
    }

    /// Encrypts full packet payload using Ephemeral ECDH exchange with recipient's X25519 converted public key.
    pub fn encode(
        &self,
        _sender_secret_key: &SecretKey,
        recipient_public_key: &PublicKey,
    ) -> Vec<u8> {
        let signable = Self::to_signable_bytes(
            self.sender_address,
            self.timestamp,
            self.nonce,
            &self.payload,
        );

        let mut decrypted_payload = Vec::with_capacity(signable.len() + 64);
        decrypted_payload.extend_from_slice(&signable);
        decrypted_payload.extend_from_slice(&self.signature.encode());

        let mut rng = rand::thread_rng();
        let ephemeral_secret = StaticSecret::random_from_rng(&mut rng);
        let ephemeral_public = x25519_dalek::PublicKey::from(&ephemeral_secret);

        let recipient_x25519 = ed25519_to_x25519_public(recipient_public_key).unwrap();
        let shared_secret = ephemeral_secret.diffie_hellman(&recipient_x25519);

        let mut nonce = [0u8; 32];
        rng.fill_bytes(&mut nonce);

        let (key, iv) = derive_symmetric_key_iv(shared_secret.as_bytes(), &nonce);

        let mut encrypted = decrypted_payload;
        apply_aes256_ctr(&key, &iv, &mut encrypted);

        let mut wire = Vec::with_capacity(32 + 32 + 32 + encrypted.len());
        wire.extend_from_slice(&self.recipient_address.encode());
        wire.extend_from_slice(ephemeral_public.as_bytes());
        wire.extend_from_slice(&nonce);
        wire.extend_from_slice(&encrypted);
        wire
    }

    /// Decrypts full packet using recipient's Ed25519 secret key converted to X25519 scalar.
    pub fn decode(wire: &[u8], recipient_secret_key: &SecretKey) -> Result<Self, NetworkError> {
        if wire.len() < 32 + 32 + 32 + 32 + 4 + 4 + 2 + 64 {
            return Err(NetworkError::TruncatedPacket);
        }

        let mut recipient_addr_bytes = [0u8; 32];
        recipient_addr_bytes.copy_from_slice(&wire[..32]);
        let recipient_address = Uint256(recipient_addr_bytes);

        let mut ephemeral_bytes = [0u8; 32];
        ephemeral_bytes.copy_from_slice(&wire[32..64]);
        let ephemeral_public = x25519_dalek::PublicKey::from(ephemeral_bytes);

        let mut nonce = [0u8; 32];
        nonce.copy_from_slice(&wire[64..96]);

        let encrypted = &wire[96..];

        let recipient_x25519_secret = ed25519_to_x25519_secret(recipient_secret_key);
        let shared_secret = recipient_x25519_secret.diffie_hellman(&ephemeral_public);

        let (key, iv) = derive_symmetric_key_iv(shared_secret.as_bytes(), &nonce);

        let mut decrypted = encrypted.to_vec();
        apply_aes256_ctr(&key, &iv, &mut decrypted);

        if decrypted.len() < 32 + 4 + 4 + 2 + 64 {
            return Err(NetworkError::TruncatedPacket);
        }

        let mut sender_addr_bytes = [0u8; 32];
        sender_addr_bytes.copy_from_slice(&decrypted[..32]);
        let sender_address = Uint256(sender_addr_bytes);

        let timestamp = u32::from_be_bytes(decrypted[32..36].try_into().unwrap());
        let packet_nonce = u32::from_be_bytes(decrypted[36..40].try_into().unwrap());
        let payload_len = u16::from_be_bytes(decrypted[40..42].try_into().unwrap()) as usize;

        if decrypted.len() < 42 + payload_len + 64 {
            return Err(NetworkError::TruncatedPacket);
        }

        let payload = decrypted[42..42 + payload_len].to_vec();
        let sig_bytes = &decrypted[42 + payload_len..42 + payload_len + 64];
        let signature =
            Signature::decode_exact(sig_bytes).map_err(|_| NetworkError::InvalidSignature)?;

        Ok(Self {
            recipient_address,
            sender_address,
            timestamp,
            nonce: packet_nonce,
            payload,
            signature,
        })
    }
}

/// Represents a Fast Channel Encrypted Packet (§4.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastPacket {
    pub channel_or_recipient_id: Uint256,
    pub sender_address: Uint256,
    pub nonce: [u8; 32],
    pub payload: Vec<u8>,
}

impl FastPacket {
    pub fn encode(&self, shared_secret: &[u8; 32]) -> Vec<u8> {
        let (key, iv) = derive_symmetric_key_iv(shared_secret, &self.nonce);

        let payload_hash =
            domain_hash(&DomainTag::from_ascii("ONX_ADNL_PAYLOAD_V1"), &self.payload);

        let mut plaintext = Vec::with_capacity(32 + 2 + self.payload.len());
        plaintext.extend_from_slice(&payload_hash);
        plaintext.extend_from_slice(&(self.payload.len() as u16).to_be_bytes());
        plaintext.extend_from_slice(&self.payload);

        let mut encrypted = plaintext;
        apply_aes256_ctr(&key, &iv, &mut encrypted);

        let mut wire = Vec::with_capacity(32 + 32 + 32 + encrypted.len());
        wire.extend_from_slice(&self.channel_or_recipient_id.encode());
        wire.extend_from_slice(&self.sender_address.encode());
        wire.extend_from_slice(&self.nonce);
        wire.extend_from_slice(&encrypted);
        wire
    }

    pub fn decode(wire: &[u8], shared_secret: &[u8; 32]) -> Result<Self, NetworkError> {
        if wire.len() < 32 + 32 + 32 + 32 + 2 {
            return Err(NetworkError::TruncatedPacket);
        }

        let mut channel_bytes = [0u8; 32];
        channel_bytes.copy_from_slice(&wire[..32]);
        let channel_or_recipient_id = Uint256(channel_bytes);

        let mut sender_bytes = [0u8; 32];
        sender_bytes.copy_from_slice(&wire[32..64]);
        let sender_address = Uint256(sender_bytes);

        let mut nonce = [0u8; 32];
        nonce.copy_from_slice(&wire[64..96]);

        let encrypted = &wire[96..];

        let (key, iv) = derive_symmetric_key_iv(shared_secret, &nonce);

        let mut decrypted = encrypted.to_vec();
        apply_aes256_ctr(&key, &iv, &mut decrypted);

        if decrypted.len() < 34 {
            return Err(NetworkError::TruncatedPacket);
        }

        let mut expected_hash = [0u8; 32];
        expected_hash.copy_from_slice(&decrypted[..32]);

        let payload_len = u16::from_be_bytes(decrypted[32..34].try_into().unwrap()) as usize;
        if decrypted.len() < 34 + payload_len {
            return Err(NetworkError::TruncatedPacket);
        }

        let payload = decrypted[34..34 + payload_len].to_vec();

        let actual_hash = domain_hash(&DomainTag::from_ascii("ONX_ADNL_PAYLOAD_V1"), &payload);
        if actual_hash != expected_hash {
            return Err(NetworkError::DecryptionFailed);
        }

        Ok(Self {
            channel_or_recipient_id,
            sender_address,
            nonce,
            payload,
        })
    }
}

/// Active peer session channel details.
#[derive(Debug, Clone)]
pub struct PeerSession {
    pub peer_address: Uint256,
    pub peer_public_key: PublicKey,
    pub remote_endpoint: SocketAddr,
    pub channel_id: Uint256,
    pub shared_secret: [u8; 32],
}

/// ADNL Transport node running over Tokio UDP socket.
pub struct AdnlTransportNode {
    secret_key: SecretKey,
    public_key: PublicKey,
    key_description: KeyDescription,
    abstract_address: Uint256,
    socket: Arc<UdpSocket>,
    sessions: Arc<Mutex<HashMap<Uint256, PeerSession>>>,
    channel_to_peer: Arc<Mutex<HashMap<Uint256, Uint256>>>,
}

impl AdnlTransportNode {
    pub async fn bind(
        secret_key: SecretKey,
        bind_addr: SocketAddr,
    ) -> Result<Self, std::io::Error> {
        let socket = UdpSocket::bind(bind_addr).await?;
        let public_key = secret_key.public_key();
        let key_description = KeyDescription::new_ed25519(public_key);
        let abstract_address = key_description.compute_abstract_address();

        Ok(Self {
            secret_key,
            public_key,
            key_description,
            abstract_address,
            socket: Arc::new(socket),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            channel_to_peer: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, std::io::Error> {
        self.socket.local_addr()
    }

    pub fn abstract_address(&self) -> Uint256 {
        self.abstract_address
    }

    pub fn public_key(&self) -> PublicKey {
        self.public_key
    }

    pub fn secret_key(&self) -> &SecretKey {
        &self.secret_key
    }

    pub fn key_description(&self) -> &KeyDescription {
        &self.key_description
    }

    /// Establishes or returns an active channel with a peer via X25519 Diffie-Hellman key exchange.
    pub fn connect_peer(
        &self,
        peer_public_key: PublicKey,
        remote_endpoint: SocketAddr,
    ) -> PeerSession {
        let peer_key_desc = KeyDescription::new_ed25519(peer_public_key);
        let peer_address = peer_key_desc.compute_abstract_address();

        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get(&peer_address) {
            return session.clone();
        }

        let my_x25519_secret = ed25519_to_x25519_secret(&self.secret_key);
        let peer_x25519_pub = ed25519_to_x25519_public(&peer_public_key).unwrap();

        let shared_diffie = my_x25519_secret.diffie_hellman(&peer_x25519_pub);
        let mut shared_secret = [0u8; 32];
        shared_secret.copy_from_slice(shared_diffie.as_bytes());

        // Derive channel_id deterministically for point-to-point peer connection
        let channel_id = if self.abstract_address.0 < peer_address.0 {
            derive_channel_id(&shared_secret, self.abstract_address, peer_address)
        } else {
            derive_channel_id(&shared_secret, peer_address, self.abstract_address)
        };

        let session = PeerSession {
            peer_address,
            peer_public_key,
            remote_endpoint,
            channel_id,
            shared_secret,
        };

        sessions.insert(peer_address, session.clone());

        let mut channel_map = self.channel_to_peer.lock().unwrap();
        channel_map.insert(channel_id, peer_address);

        session
    }

    /// Sends datagram payload over established ADNL channel with automatic session lookup/reconnection.
    pub async fn send_datagram(
        &self,
        peer_public_key: PublicKey,
        remote_endpoint: SocketAddr,
        payload: &[u8],
    ) -> Result<(), NetworkError> {
        let session = self.connect_peer(peer_public_key, remote_endpoint);

        let mut nonce = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut nonce);

        let fast_pkt = FastPacket {
            channel_or_recipient_id: session.channel_id,
            sender_address: self.abstract_address,
            nonce,
            payload: payload.to_vec(),
        };

        let wire = fast_pkt.encode(&session.shared_secret);

        self.socket
            .send_to(&wire, remote_endpoint)
            .await
            .map_err(|_| NetworkError::DecryptionFailed)?;

        Ok(())
    }

    /// Receives and decrypts a datagram over ADNL transport. Handles both FullPacket and FastPacket datagrams.
    pub async fn recv_datagram(&self) -> Result<(Uint256, Vec<u8>, SocketAddr), NetworkError> {
        let mut buf = [0u8; 65535];
        let (len, src_addr) = self
            .socket
            .recv_from(&mut buf)
            .await
            .map_err(|_| NetworkError::TruncatedPacket)?;

        let wire = &buf[..len];
        if wire.len() < 32 {
            return Err(NetworkError::TruncatedPacket);
        }

        let mut id_bytes = [0u8; 32];
        id_bytes.copy_from_slice(&wire[..32]);
        let header_id = Uint256(id_bytes);

        // First check if header_id matches our abstract address (FullPacket)
        if header_id == self.abstract_address {
            if let Ok(full_pkt) = FullPacket::decode(wire, &self.secret_key) {
                return Ok((full_pkt.sender_address, full_pkt.payload, src_addr));
            }
        }

        // Check if header_id matches an active session channel_id
        let session_opt = {
            let channel_map = self.channel_to_peer.lock().unwrap();
            let peer_addr = channel_map.get(&header_id).cloned();
            if let Some(peer_addr) = peer_addr {
                let sessions = self.sessions.lock().unwrap();
                sessions.get(&peer_addr).cloned()
            } else {
                None
            }
        };

        if let Some(session) = session_opt {
            let pkt = FastPacket::decode(wire, &session.shared_secret)?;
            return Ok((pkt.sender_address, pkt.payload, src_addr));
        }

        Err(NetworkError::DecryptionFailed)
    }
}
