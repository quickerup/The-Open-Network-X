//! Deterministic slow-path hypercube routing for internal messages.
//!
//! This module implements the step-by-step neighbor routing required by
//! `transactions.md` §3.4.  It intentionally accepts shard identifiers from
//! the caller: shard discovery and queue ownership belong to the sharding and
//! node-runtime layers, while this consensus-critical code only derives and
//! validates the canonical path.

use crate::error::TransactionsError;
use onx_data_structures::{Message, MessageType, ShardIdent};

/// Logical-time lifetime for cross-workchain messages, from ADR-0018.
pub const MAX_CROSS_WORKCHAIN_LT_WINDOW: u64 = 1_000_000;

/// One directed, neighboring transition in a slow-path route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteHop {
    pub from: ShardIdent,
    pub to: ShardIdent,
}

/// A canonical route together with its deterministic transit cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutePlan {
    pub hops: Vec<RouteHop>,
    pub transit_fee: u128,
    pub remaining_value: u128,
}

/// Derives the canonical slow path by changing differing prefix bits from
/// most significant to least significant.  This makes all validators choose
/// the same path when several hypercube paths are mathematically possible.
pub fn plan_hypercube_route(
    message: &Message,
    source: ShardIdent,
    destination: ShardIdent,
    fee_per_hop: u128,
) -> Result<RoutePlan, TransactionsError> {
    if message.msg_type != MessageType::Internal
        || message.validate_source_shard(&source).is_err()
        || message.validate_dest_shard(&destination).is_err()
    {
        return Err(TransactionsError::RouteEndpointMismatch);
    }

    let prefix_len = source
        .prefix_len()
        .map_err(|_| TransactionsError::IncompatibleRoute)?;
    if source.workchain_id != destination.workchain_id
        || destination
            .prefix_len()
            .map_err(|_| TransactionsError::IncompatibleRoute)?
            != prefix_len
    {
        return Err(TransactionsError::IncompatibleRoute);
    }

    let mask = if prefix_len == 0 {
        0
    } else {
        (!0u64) << (64 - prefix_len)
    };
    let target_bits = destination.shard_prefix_ident.0 & mask;
    let mut current_bits = source.shard_prefix_ident.0 & mask;
    let mut current = source;
    let mut hops = Vec::new();
    for bit_index in 0..prefix_len {
        let bit = 63 - bit_index;
        if ((current_bits >> bit) & 1) != ((target_bits >> bit) & 1) {
            current_bits ^= 1u64 << bit;
            let next = ShardIdent::from_prefix_bits(source.workchain_id, current_bits, prefix_len)
                .map_err(|_| TransactionsError::IncompatibleRoute)?;
            hops.push(RouteHop {
                from: current,
                to: next,
            });
            current = next;
        }
    }
    debug_assert_eq!(current, destination);

    let transit_fee = fee_per_hop.checked_mul(hops.len() as u128).ok_or(
        TransactionsError::InsufficientTransitFee {
            available: message.amount_nanos.0,
            required: u128::MAX,
        },
    )?;
    let remaining_value = message.amount_nanos.0.checked_sub(transit_fee).ok_or(
        TransactionsError::InsufficientTransitFee {
            available: message.amount_nanos.0,
            required: transit_fee,
        },
    )?;
    Ok(RoutePlan {
        hops,
        transit_fee,
        remaining_value,
    })
}

/// Verifies an externally supplied route before a block commits its transit
/// step.  Every hop must preserve workchain and depth and flip exactly one
/// prefix bit; the final shard must be `destination`.
pub fn validate_hypercube_route(
    hops: &[RouteHop],
    source: ShardIdent,
    destination: ShardIdent,
) -> Result<(), TransactionsError> {
    let mut current = source;
    for hop in hops {
        if hop.from != current || !are_neighbors(hop.from, hop.to) {
            return Err(TransactionsError::InvalidHypercubeHop);
        }
        current = hop.to;
    }
    if current == destination {
        Ok(())
    } else {
        Err(TransactionsError::InvalidHypercubeHop)
    }
}

/// Returns true precisely when two equal-depth shards differ in one prefix bit.
pub fn are_neighbors(left: ShardIdent, right: ShardIdent) -> bool {
    let Ok(length) = left.prefix_len() else {
        return false;
    };
    if left.workchain_id != right.workchain_id || right.prefix_len().ok() != Some(length) {
        return false;
    }
    let mask = if length == 0 {
        0
    } else {
        (!0u64) << (64 - length)
    };
    ((left.shard_prefix_ident.0 ^ right.shard_prefix_ident.0) & mask).count_ones() == 1
}

/// Returns the inclusive-expiry boundary for a cross-workchain message.
pub fn cross_workchain_expiry(message: &Message) -> Result<u64, TransactionsError> {
    message
        .created_lt
        .0
        .checked_add(MAX_CROSS_WORKCHAIN_LT_WINDOW)
        .ok_or(TransactionsError::ExpiryOverflow)
}

/// Expiry is deterministic: a message is expired only after its expiry LT.
pub fn is_cross_workchain_expired(
    message: &Message,
    current_lt: u64,
) -> Result<bool, TransactionsError> {
    Ok(current_lt > cross_workchain_expiry(message)?)
}
