//! Event logging system for smart contracts.
//!
//! This module provides traits and utilities for emitting structured events
//! from smart contracts. Events are indexed logs that can be queried off-chain.
//!
//! # Example
//!
//! ```ignore
//! use truthlinked_sdk::prelude::*;
//!
//! #[derive(Event)]
//! #[event(name = "Transfer")]
//! struct Transfer {
//!     #[topic]
//!     from: [u8; 32],
//!     #[topic]
//!     to: [u8; 32],
//!     amount: u128,
//! }
//!
//! fn transfer(from: [u8; 32], to: [u8; 32], amount: u128) -> Result<()> {
//!     Transfer { from, to, amount }.emit()?;
//!     Ok(())
//! }
//! ```

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use crate::codec::{Codec32, Encoder};
use crate::env;
use crate::error::Result;
use crate::hashing;

/// Private re-exports for macro-generated code.
pub mod __private {
    pub use alloc::vec::Vec;
}

/// Emits a log event with the given topics and data.
pub fn emit(topics: &[[u8; 32]], data: &[u8]) -> Result<()> {
    let mut flat = Vec::with_capacity(topics.len() * 32);
    for topic in topics {
        flat.extend_from_slice(topic);
    }
    env::emit_log_bytes(&flat, data)
}

/// Converts raw bytes to a 32-byte topic (left-padded with zeros if shorter).
pub fn topic_from_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut topic = [0u8; 32];
    let copy_len = bytes.len().min(32);
    topic[..copy_len].copy_from_slice(&bytes[..copy_len]);
    topic
}

/// Computes the event signature hash for a given event name.
pub fn event_signature(signature: &str) -> [u8; 32] {
    hashing::hash32(signature.as_bytes())
}

/// Trait for types that can be converted to event topics.
pub trait EventTopic {
    /// Converts this value to a 32-byte topic.
    fn to_topic(&self) -> [u8; 32];
}

/// Trait for types that can be encoded as event data.
pub trait EventData {
    /// Encodes this value into the event data encoder.
    fn encode_event_data(&self, encoder: &mut Encoder);
}

/// Trait for event types that can be emitted.
///
/// Typically derived using `#[derive(Event)]`.
pub trait Event {
    /// Returns the event name (e.g., "Transfer").
    fn event_name() -> &'static str;

    /// Returns whether this event is anonymous (no signature topic).
    fn is_anonymous() -> bool {
        false
    }

    /// Returns the event signature hash.
    fn event_signature() -> [u8; 32] {
        event_signature(Self::event_name())
    }

    /// Returns the topics for this event instance.
    fn event_topics(&self) -> Vec<[u8; 32]>;

    /// Returns the encoded data for this event instance.
    fn event_data(&self) -> Vec<u8>;

    /// Emits this event.
    fn emit(&self) -> Result<()>
    where
        Self: Sized,
    {
        emit_event(self)
    }
}

/// Emits an event implementing the `Event` trait.
pub fn emit_event<E: Event>(event: &E) -> Result<()> {
    let topics = event.event_topics();
    let data = event.event_data();
    emit(&topics, &data)
}

/// Checks if the given topics match the event signature.
pub fn event_topics_match<E: Event>(topics: &[[u8; 32]]) -> bool {
    if E::is_anonymous() {
        true
    } else {
        topics
            .first()
            .map(|t| *t == E::event_signature())
            .unwrap_or(false)
    }
}

/// Returns the indexed topics (excluding the signature topic).
pub fn indexed_topics<'a, E: Event>(topics: &'a [[u8; 32]]) -> &'a [[u8; 32]] {
    if E::is_anonymous() {
        topics
    } else {
        topics.get(1..).unwrap_or(&[])
    }
}

/// Decodes event data into a typed value.
pub fn decode_event_data<T: crate::codec::BytesCodec>(data: &[u8]) -> Result<T> {
    T::decode_bytes(data)
}

macro_rules! impl_topic_codec32 {
    ($ty:ty) => {
        impl EventTopic for $ty {
            fn to_topic(&self) -> [u8; 32] {
                self.encode_32()
            }
        }
    };
}

impl_topic_codec32!(bool);
impl_topic_codec32!(u8);
impl_topic_codec32!(u16);
impl_topic_codec32!(u32);
impl_topic_codec32!(u64);
impl_topic_codec32!(u128);
impl_topic_codec32!(i8);
impl_topic_codec32!(i16);
impl_topic_codec32!(i32);
impl_topic_codec32!(i64);
impl_topic_codec32!(i128);

impl<const N: usize> EventTopic for [u8; N] {
    fn to_topic(&self) -> [u8; 32] {
        if N == 32 {
            let mut out = [0u8; 32];
            out.copy_from_slice(self);
            out
        } else if N < 32 {
            let mut out = [0u8; 32];
            out[..N].copy_from_slice(self);
            out
        } else {
            hashing::hash32(self)
        }
    }
}

impl EventTopic for [u8] {
    fn to_topic(&self) -> [u8; 32] {
        if self.len() <= 32 {
            topic_from_bytes(self)
        } else {
            hashing::hash32(self)
        }
    }
}

impl EventTopic for Vec<u8> {
    fn to_topic(&self) -> [u8; 32] {
        self.as_slice().to_topic()
    }
}

impl EventTopic for str {
    fn to_topic(&self) -> [u8; 32] {
        hashing::hash32(self.as_bytes())
    }
}

impl EventTopic for String {
    fn to_topic(&self) -> [u8; 32] {
        self.as_str().to_topic()
    }
}

impl<T: crate::codec::BytesCodec> EventData for T {
    fn encode_event_data(&self, encoder: &mut Encoder) {
        encoder.push_bytes(&self.encode_bytes());
    }
}

impl EventData for [u8] {
    fn encode_event_data(&self, encoder: &mut Encoder) {
        encoder.push_bytes(self);
    }
}

impl EventData for str {
    fn encode_event_data(&self, encoder: &mut Encoder) {
        encoder.push_bytes(self.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(crate::Event)]
    #[event(name = "CounterUpdated")]
    struct CounterUpdated {
        #[topic]
        caller: [u8; 32],
        value: u64,
        memo: alloc::string::String,
    }

    #[test]
    fn derive_event_encodes_topics_and_data() {
        let event = CounterUpdated {
            caller: [9u8; 32],
            value: 42,
            memo: alloc::string::String::from("ok"),
        };

        let topics = event.event_topics();
        assert_eq!(topics.len(), 2);
        assert_eq!(topics[1], [9u8; 32]);

        let data = event.event_data();
        assert!(!data.is_empty());
    }

    fn encode_u64_topic(v: &u64) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[..8].copy_from_slice(&v.to_le_bytes());
        out
    }

    fn encode_bool_data(v: &bool, encoder: &mut crate::codec::Encoder) {
        encoder.push_bool(*v);
    }

    #[derive(crate::Event)]
    #[event(name = "OrderFilled", anonymous)]
    struct OrderFilled {
        #[event(topic, with_topic = "encode_u64_topic", type = "uint64")]
        order_id: u64,
        #[event(with_data = "encode_bool_data")]
        settled: bool,
        #[event(skip)]
        _padding: u8,
    }

    #[test]
    fn derive_event_supports_advanced_field_options() {
        let event = OrderFilled {
            order_id: 77,
            settled: true,
            _padding: 0,
        };

        assert!(OrderFilled::is_anonymous());
        let topics = event.event_topics();
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0][..8], 77u64.to_le_bytes());
        assert!(!event.event_data().is_empty());
    }
}
