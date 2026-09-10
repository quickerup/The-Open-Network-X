//! Reliable Large Datagram Protocol framing and loss recovery for ADNL.
//!
//! RLDP uses systematic erasure coding: every source symbol is emitted several
//! times before retransmission rounds begin.  The receiver acknowledges the
//! sequence numbers it has recovered, allowing later rounds to contain only
//! erased symbols.  This deliberately small code is suitable for the ADNL UDP
//! MTU and avoids buffering an unbounded stream in the transport layer.

use crate::{adnl_transport::AdnlTransportNode, NetworkError};
use onx_primitives::{PublicKey, Uint256};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::net::SocketAddr;
use tokio::time::{timeout, Duration};

pub const DATA_TAG: u32 = 0x71d2_b841;
pub const ACK_TAG: u32 = 0x3a4f_1092;
pub const DEFAULT_CHUNK_SIZE: usize = 1024;
pub const MAX_TRANSFER_SIZE: usize = 16 * 1024 * 1024;
const DATA_HEADER_LEN: usize = 4 + 32 + 32 + 4 + 2 + 2 + 2 + 2;
const ACK_HEADER_LEN: usize = 4 + 32 + 4 + 2 + 2;

/// Bounds transmission work so a single transfer cannot monopolize ADNL UDP.
#[derive(Debug, Clone, Copy)]
pub struct RldpConfig {
    pub chunk_size: usize,
    /// Number of systematic erasure symbols emitted per source symbol per round.
    pub redundancy: usize,
    pub packets_per_round: usize,
    pub max_rounds: usize,
}

impl Default for RldpConfig {
    fn default() -> Self {
        Self {
            chunk_size: DEFAULT_CHUNK_SIZE,
            redundancy: 3,
            packets_per_round: 128,
            max_rounds: 20,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RldpDataFrame {
    pub transfer_id: Uint256,
    pub payload_digest: [u8; 32],
    pub total_size: u32,
    pub chunk_size: u16,
    pub chunk_count: u16,
    pub seq_no: u16,
    /// Fixed-width symbol, zero-padded only in the final source symbol.
    pub chunk_data: Vec<u8>,
}

impl RldpDataFrame {
    pub fn encode(&self) -> Result<Vec<u8>, NetworkError> {
        validate_dimensions(self.total_size, self.chunk_size, self.chunk_count)?;
        if self.seq_no >= self.chunk_count || self.chunk_data.len() != usize::from(self.chunk_size)
        {
            return Err(NetworkError::MalformedRldpFrame);
        }
        let mut wire = Vec::with_capacity(DATA_HEADER_LEN + self.chunk_data.len());
        wire.extend_from_slice(&DATA_TAG.to_be_bytes());
        wire.extend_from_slice(&self.transfer_id.0);
        wire.extend_from_slice(&self.payload_digest);
        wire.extend_from_slice(&self.total_size.to_be_bytes());
        wire.extend_from_slice(&self.chunk_size.to_be_bytes());
        wire.extend_from_slice(&self.chunk_count.to_be_bytes());
        wire.extend_from_slice(&self.seq_no.to_be_bytes());
        wire.extend_from_slice(&(self.chunk_data.len() as u16).to_be_bytes());
        wire.extend_from_slice(&self.chunk_data);
        Ok(wire)
    }

    pub fn decode(wire: &[u8]) -> Result<Self, NetworkError> {
        if wire.len() < DATA_HEADER_LEN
            || u32::from_be_bytes(wire[..4].try_into().unwrap()) != DATA_TAG
        {
            return Err(NetworkError::MalformedRldpFrame);
        }
        let mut transfer = [0; 32];
        transfer.copy_from_slice(&wire[4..36]);
        let mut digest = [0; 32];
        digest.copy_from_slice(&wire[36..68]);
        let total_size = u32::from_be_bytes(wire[68..72].try_into().unwrap());
        let chunk_size = u16::from_be_bytes(wire[72..74].try_into().unwrap());
        let chunk_count = u16::from_be_bytes(wire[74..76].try_into().unwrap());
        let seq_no = u16::from_be_bytes(wire[76..78].try_into().unwrap());
        let data_len = usize::from(u16::from_be_bytes(wire[78..80].try_into().unwrap()));
        validate_dimensions(total_size, chunk_size, chunk_count)?;
        if seq_no >= chunk_count
            || data_len != usize::from(chunk_size)
            || wire.len() != DATA_HEADER_LEN + data_len
        {
            return Err(NetworkError::MalformedRldpFrame);
        }
        Ok(Self {
            transfer_id: Uint256(transfer),
            payload_digest: digest,
            total_size,
            chunk_size,
            chunk_count,
            seq_no,
            chunk_data: wire[80..].to_vec(),
        })
    }
}

/// A bitmap ACK. `ack_seq` is monotonically increasing, so stale ACKs cannot
/// undo progress at the sender.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RldpAckFrame {
    pub transfer_id: Uint256,
    pub ack_seq: u32,
    pub received_mask: Vec<u8>,
}

impl RldpAckFrame {
    pub fn encode(&self) -> Vec<u8> {
        let mut wire = Vec::with_capacity(ACK_HEADER_LEN + self.received_mask.len());
        wire.extend_from_slice(&ACK_TAG.to_be_bytes());
        wire.extend_from_slice(&self.transfer_id.0);
        wire.extend_from_slice(&self.ack_seq.to_be_bytes());
        wire.extend_from_slice(&(self.received_mask.len() as u16).to_be_bytes());
        wire.extend_from_slice(&self.received_mask);
        wire
    }
    pub fn decode(wire: &[u8]) -> Result<Self, NetworkError> {
        if wire.len() < ACK_HEADER_LEN
            || u32::from_be_bytes(wire[..4].try_into().unwrap()) != ACK_TAG
        {
            return Err(NetworkError::MalformedRldpFrame);
        }
        let mut id = [0; 32];
        id.copy_from_slice(&wire[4..36]);
        let ack_seq = u32::from_be_bytes(wire[36..40].try_into().unwrap());
        let len = usize::from(u16::from_be_bytes(wire[40..42].try_into().unwrap()));
        if wire.len() != ACK_HEADER_LEN + len {
            return Err(NetworkError::MalformedRldpFrame);
        }
        Ok(Self {
            transfer_id: Uint256(id),
            ack_seq,
            received_mask: wire[42..].to_vec(),
        })
    }
}

/// Sender state. Call `next_round`, transmit the returned datagrams at the
/// configured rate, then apply ACKs before the retry timeout expires.
pub struct RldpSender {
    frames: Vec<RldpDataFrame>,
    transfer_id: Uint256,
    acknowledged: HashSet<u16>,
    last_ack_seq: u32,
    config: RldpConfig,
    cursor: usize,
}

impl RldpSender {
    pub fn new(
        transfer_id: Uint256,
        payload: &[u8],
        config: RldpConfig,
    ) -> Result<Self, NetworkError> {
        if payload.is_empty()
            || payload.len() > MAX_TRANSFER_SIZE
            || config.chunk_size == 0
            || config.chunk_size > u16::MAX as usize
            || config.redundancy == 0
            || config.packets_per_round == 0
        {
            return Err(NetworkError::MalformedRldpFrame);
        }
        let count = payload.len().div_ceil(config.chunk_size);
        if count > u16::MAX as usize {
            return Err(NetworkError::MalformedRldpFrame);
        }
        let digest: [u8; 32] = Sha256::digest(payload).into();
        let frames = (0..count)
            .map(|index| {
                let start = index * config.chunk_size;
                let end = (start + config.chunk_size).min(payload.len());
                let mut data = vec![0; config.chunk_size];
                data[..end - start].copy_from_slice(&payload[start..end]);
                RldpDataFrame {
                    transfer_id,
                    payload_digest: digest,
                    total_size: payload.len() as u32,
                    chunk_size: config.chunk_size as u16,
                    chunk_count: count as u16,
                    seq_no: index as u16,
                    chunk_data: data,
                }
            })
            .collect();
        Ok(Self {
            frames,
            transfer_id,
            acknowledged: HashSet::new(),
            last_ack_seq: 0,
            config,
            cursor: 0,
        })
    }
    pub fn is_complete(&self) -> bool {
        self.acknowledged.len() == self.frames.len()
    }
    pub fn next_round(&mut self) -> Vec<Vec<u8>> {
        let mut packets = Vec::with_capacity(self.config.packets_per_round);
        let mut inspected = 0;
        while packets.len() < self.config.packets_per_round && inspected < self.frames.len() {
            let index = self.cursor;
            self.cursor = (self.cursor + 1) % self.frames.len();
            inspected += 1;
            let frame = &self.frames[index];
            if self.acknowledged.contains(&frame.seq_no) {
                continue;
            }
            for _ in 0..self.config.redundancy {
                if packets.len() == self.config.packets_per_round {
                    break;
                }
                packets.push(frame.encode().expect("validated frame"));
            }
        }
        packets
    }
    pub fn apply_ack(&mut self, ack: &RldpAckFrame) -> Result<(), NetworkError> {
        if ack.transfer_id != self.transfer_id || ack.ack_seq <= self.last_ack_seq {
            return Ok(());
        }
        if ack.received_mask.len() != self.frames.len().div_ceil(8) {
            return Err(NetworkError::MalformedRldpFrame);
        }
        self.last_ack_seq = ack.ack_seq;
        for (i, byte) in ack.received_mask.iter().enumerate() {
            for bit in 0..8 {
                let seq = i * 8 + bit;
                if seq < self.frames.len() && byte & (1 << bit) != 0 {
                    self.acknowledged.insert(seq as u16);
                }
            }
        }
        Ok(())
    }
    pub fn max_rounds(&self) -> usize {
        self.config.max_rounds
    }
}

/// Reassembles one transfer and verifies the uncompressed payload SHA-256
/// before exposing it to callers.
pub struct RldpReceiver {
    transfer_id: Uint256,
    digest: [u8; 32],
    total_size: usize,
    chunk_size: usize,
    chunk_count: usize,
    chunks: BTreeMap<u16, Vec<u8>>,
    ack_seq: u32,
}

impl RldpReceiver {
    pub fn from_frame(frame: &RldpDataFrame) -> Result<Self, NetworkError> {
        validate_dimensions(frame.total_size, frame.chunk_size, frame.chunk_count)?;
        Ok(Self {
            transfer_id: frame.transfer_id,
            digest: frame.payload_digest,
            total_size: frame.total_size as usize,
            chunk_size: frame.chunk_size as usize,
            chunk_count: frame.chunk_count as usize,
            chunks: BTreeMap::new(),
            ack_seq: 0,
        })
    }
    pub fn ingest(&mut self, frame: RldpDataFrame) -> Result<Option<Vec<u8>>, NetworkError> {
        if frame.transfer_id != self.transfer_id
            || frame.payload_digest != self.digest
            || frame.total_size as usize != self.total_size
            || frame.chunk_size as usize != self.chunk_size
            || frame.chunk_count as usize != self.chunk_count
        {
            return Err(NetworkError::MalformedRldpFrame);
        }
        self.chunks.entry(frame.seq_no).or_insert(frame.chunk_data);
        if self.chunks.len() != self.chunk_count {
            return Ok(None);
        }
        let mut payload = Vec::with_capacity(self.chunk_count * self.chunk_size);
        for seq in 0..self.chunk_count as u16 {
            payload.extend_from_slice(
                self.chunks
                    .get(&seq)
                    .ok_or(NetworkError::MalformedRldpFrame)?,
            );
        }
        payload.truncate(self.total_size);
        if <[u8; 32]>::from(Sha256::digest(&payload)) != self.digest {
            return Err(NetworkError::PayloadDigestMismatch);
        }
        Ok(Some(payload))
    }
    pub fn ack(&mut self) -> RldpAckFrame {
        self.ack_seq = self.ack_seq.wrapping_add(1);
        let mut mask = vec![0; self.chunk_count.div_ceil(8)];
        for seq in self.chunks.keys() {
            mask[*seq as usize / 8] |= 1 << (*seq as usize % 8);
        }
        RldpAckFrame {
            transfer_id: self.transfer_id,
            ack_seq: self.ack_seq,
            received_mask: mask,
        }
    }
}

fn validate_dimensions(
    total_size: u32,
    chunk_size: u16,
    chunk_count: u16,
) -> Result<(), NetworkError> {
    if total_size == 0
        || total_size as usize > MAX_TRANSFER_SIZE
        || chunk_size == 0
        || chunk_count == 0
        || (total_size as usize).div_ceil(chunk_size as usize) != chunk_count as usize
    {
        Err(NetworkError::MalformedRldpFrame)
    } else {
        Ok(())
    }
}

impl AdnlTransportNode {
    /// Sends a payload as rate-limited RLDP frames over the encrypted ADNL UDP
    /// channel. The remote peer must run [`Self::recv_rldp`].
    pub async fn send_rldp(
        &self,
        peer_public_key: PublicKey,
        remote_endpoint: SocketAddr,
        transfer_id: Uint256,
        payload: &[u8],
        config: RldpConfig,
        retry_timeout: Duration,
    ) -> Result<(), NetworkError> {
        let mut sender = RldpSender::new(transfer_id, payload, config)?;
        for _ in 0..sender.max_rounds() {
            for packet in sender.next_round() {
                self.send_datagram(peer_public_key, remote_endpoint, &packet)
                    .await?;
            }
            if sender.is_complete() {
                return Ok(());
            }
            // ACK loss is harmless: the next timeout repeats only symbols not
            // known to be acknowledged, and receiver de-duplicates symbols.
            while let Ok(Ok((_, packet, _))) = timeout(retry_timeout, self.recv_datagram()).await {
                if let Ok(ack) = RldpAckFrame::decode(&packet) {
                    sender.apply_ack(&ack)?;
                    if sender.is_complete() {
                        return Ok(());
                    }
                }
            }
        }
        Err(NetworkError::RldpTimeout)
    }

    /// Receives one RLDP transfer and returns it only after SHA-256 validation.
    /// It sends a sequence-numbered bitmap ACK after every accepted symbol,
    /// which provides prompt loss feedback without waiting for a whole message.
    pub async fn recv_rldp(
        &self,
        peer_public_key: PublicKey,
        remote_endpoint: SocketAddr,
    ) -> Result<Vec<u8>, NetworkError> {
        let (_, first_packet, _) = self.recv_datagram().await?;
        let first = RldpDataFrame::decode(&first_packet)?;
        let mut receiver = RldpReceiver::from_frame(&first)?;
        let result = receiver.ingest(first)?;
        self.send_datagram(peer_public_key, remote_endpoint, &receiver.ack().encode())
            .await?;
        if let Some(payload) = result {
            return Ok(payload);
        }
        loop {
            let (_, packet, _) = self.recv_datagram().await?;
            let frame = match RldpDataFrame::decode(&packet) {
                Ok(frame) => frame,
                Err(_) => continue,
            };
            let result = receiver.ingest(frame)?;
            self.send_datagram(peer_public_key, remote_endpoint, &receiver.ack().encode())
                .await?;
            if let Some(payload) = result {
                return Ok(payload);
            }
        }
    }
}
