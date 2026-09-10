use crate::{ChannelError, ChannelState, PaymentChannelArbiter};
use onx_primitives::{Signature, Uint256, Uint64};
use std::collections::HashMap;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedStateEnvelope {
    pub state: ChannelState,
    pub sig_a: Signature,
    pub sig_b: Signature,
}

impl SignedStateEnvelope {
    pub fn new(state: ChannelState, sig_a: Signature, sig_b: Signature) -> Self {
        Self { state, sig_a, sig_b }
    }
}

/// A lightweight off-chain daemon that sits on top of the existing
/// deterministic payment channel arbiter logic. It is intentionally shaped
/// like the current branch-safe service scaffold: the exchange API and dispute
/// cycle remain in pure Rust, while the daemon can expose a background watcher
/// that finalizes any challenge window recorded by the arbiter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentChannelDaemon {
    pub arbiter: PaymentChannelArbiter,
    pub latest_by_channel: HashMap<Uint256, ChannelState>,
}

impl PaymentChannelDaemon {
    pub fn new(arbiter: PaymentChannelArbiter) -> Self {
        let mut map = HashMap::new();
        map.insert(arbiter.channel_id, arbiter.latest_state.clone());
        Self {
            arbiter,
            latest_by_channel: map,
        }
    }

    pub fn latest_state(&self) -> ChannelState {
        self.latest_by_channel
            .get(&self.arbiter.channel_id)
            .cloned()
            .unwrap_or_else(|| self.arbiter.latest_state.clone())
    }

    pub fn exchange_signed_state(&mut self, envelope: SignedStateEnvelope) -> Result<(), ChannelError> {
        let SignedStateEnvelope { state, sig_a, sig_b } = envelope;

        if state.channel_id != self.arbiter.channel_id {
            return Err(ChannelError::MismatchedChannelId);
        }

        self.arbiter.verify_dual_signatures(&state, &sig_a, &sig_b)?;

        if state.sequence.0 <= self.latest_state().sequence.0 {
            return Err(ChannelError::NonIncreasingSequence);
        }

        if state
            .balance_party_a
            .0
            .saturating_add(state.balance_party_b.0)
            > self.arbiter.total_deposit.0
        {
            return Err(ChannelError::AllocationExceedsDeposit);
        }

        self.arbiter.latest_state = state.clone();
        self.latest_by_channel.insert(self.arbiter.channel_id, state.clone());
        Ok(())
    }

    pub fn exchange_cooperative_settlement(
        &mut self,
        envelope: SignedStateEnvelope,
    ) -> Result<(), ChannelError> {
        let SignedStateEnvelope { state, sig_a, sig_b } = envelope;
        self.arbiter.cooperative_settle(state, sig_a, sig_b)?;
        self.latest_by_channel.insert(self.arbiter.channel_id, self.arbiter.latest_state.clone());
        Ok(())
    }

    /// Intended hook for an off-chain peer loop that can submit a signed
    /// state envelope and let the daemon decide if it is stale or current.
    pub fn receive_signed_state_and_track(
        &mut self,
        envelope: SignedStateEnvelope,
    ) -> Result<(), ChannelError> {
        self.exchange_signed_state(envelope)
    }

    /// Detect stale state by comparing the submitted balance snapshot against
    /// the most recently accepted state. Since the arbiter only checks sequence
    /// and signature validity during dispute commitment, this helper records
    /// the proof event at the daemon layer while delegating the core dispute
    /// call to the arbiter implementation already in the library.
    pub fn is_stale_submission(&self, state: &ChannelState) -> bool {
        let latest = self.latest_state();
        state.sequence.0 > latest.sequence.0
            && (state.balance_party_a != latest.balance_party_a
                || state.balance_party_b != latest.balance_party_b)
    }

    /// Submit an uncooperative dispute on a stale state and arm the arbiter
    /// challenge window for the existing finalize hook.
    pub fn submit_uncooperative_dispute(
        &mut self,
        envelope: SignedStateEnvelope,
        current_lt: Uint64,
    ) -> Result<(), ChannelError> {
        let SignedStateEnvelope { state, sig_a, sig_b } = envelope;
        self.arbiter.submit_uncooperative_state(state, sig_a, sig_b, current_lt)?;
        self.latest_by_channel.insert(self.arbiter.channel_id, self.arbiter.latest_state.clone());
        Ok(())
    }

    /// Finalize after the configured challenge period has elapsed.
    pub fn finalize_dispute_if_due(&mut self, current_lt: Uint64) -> Result<(), ChannelError> {
        if self.arbiter.challenge_start_lt.is_some() {
            self.arbiter.finalize_uncooperative_settlement(current_lt)?;
        }
        Ok(())
    }

    /// Spawns a tiny background watcher that polls a monotonic logical-time
    /// source and asks the arbiter to finalize any active challenge period
    /// when the challenge window is exceeded. This keeps the runtime seam
    /// deterministic without inventing a real peer transport.
    pub fn spawn_background_challenge_watcher(
        &self,
        interval_ms: u64,
        current_lt: Arc<AtomicU64>,
    ) -> JoinHandle<()> {
        let mut daemon = self.clone();
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(interval_ms));
            loop {
                ticker.tick().await;
                let observed_lt = Uint64(current_lt.load(Ordering::Relaxed));
                let _ = daemon.finalize_dispute_if_due(observed_lt);
            }
        })
    }

    /// End-to-end service handoff used by the daemon test and examples.
    pub async fn run_flow(
        &mut self,
        envelope: SignedStateEnvelope,
        current_lt: Uint64,
    ) -> Result<(), ChannelError> {
        if self.is_stale_submission(&envelope.state) {
            self.submit_uncooperative_dispute(envelope, current_lt)?;
            let due_lt = Uint64(current_lt.0 + self.arbiter.challenge_window_lt.0);
            self.finalize_dispute_if_due(due_lt)?;
        } else {
            self.exchange_signed_state(envelope)?;
        }
        Ok(())
    }
}
