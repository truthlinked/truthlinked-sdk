//! Storage slot abstraction for direct storage access.
//!
//! This module provides the `Slot` type for working with individual storage slots
//! and helper functions for deriving storage keys.
//!
//! # Example
//!
//! ```ignore
//! use truthlinked_sdk::storage::Slot;
//!
//! let slot = Slot::from_label("counter");
//! slot.write_u64(42)?;
//! let value = slot.read_u64()?;
//! ```

use crate::codec::Codec32;
use crate::env;
use crate::error::Result;
use crate::hashing;

/// A 32-byte storage slot identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Slot(pub [u8; 32]);

impl Slot {
    /// Creates a new slot from a 32-byte array.
    pub const fn new(slot: [u8; 32]) -> Self {
        Self(slot)
    }

    /// Creates a slot from a string label.
    pub fn from_label(label: &str) -> Self {
        Self(hashing::derive_slot(b"trth:sdk:slot", &[label.as_bytes()]))
    }

    /// Creates a derived slot from a namespace and parts.
    pub fn derived(namespace: &[u8; 32], parts: &[&[u8]]) -> Self {
        Self(hashing::derive_slot(namespace, parts))
    }

    /// Reads the raw 32-byte value from this slot.
    pub fn read(self) -> Result<[u8; 32]> {
        env::storage_read_32(&self.0)
    }

    /// Writes a raw 32-byte value to this slot.
    pub fn write(self, value: &[u8; 32]) -> Result<()> {
        env::storage_write_32(&self.0, value)
    }

    /// Writes a typed value implementing `Codec32`.
    pub fn write_typed<T: Codec32>(self, value: &T) -> Result<()> {
        let encoded = value.encode_32();
        self.write(&encoded)
    }

    /// Reads a typed value implementing `Codec32`.
    pub fn read_typed<T: Codec32>(self) -> Result<T> {
        let raw = self.read()?;
        T::decode_32(&raw)
    }

    /// Writes a `u64` value (little-endian).
    pub fn write_u64(self, value: u64) -> Result<()> {
        let mut bytes = [0u8; 32];
        bytes[..8].copy_from_slice(&value.to_le_bytes());
        self.write(&bytes)
    }

    /// Reads a `u64` value (little-endian).
    pub fn read_u64(self) -> Result<u64> {
        let bytes = self.read()?;
        let mut out = [0u8; 8];
        out.copy_from_slice(&bytes[..8]);
        Ok(u64::from_le_bytes(out))
    }

    /// Writes a `u128` value (little-endian).
    pub fn write_u128(self, value: u128) -> Result<()> {
        let mut bytes = [0u8; 32];
        bytes[..16].copy_from_slice(&value.to_le_bytes());
        self.write(&bytes)
    }

    /// Reads a `u128` value (little-endian).
    pub fn read_u128(self) -> Result<u128> {
        let bytes = self.read()?;
        let mut out = [0u8; 16];
        out.copy_from_slice(&bytes[..16]);
        Ok(u128::from_le_bytes(out))
    }
}

/// Creates a namespace hash from a label.
pub fn namespace(label: &str) -> [u8; 32] {
    hashing::hash32(label.as_bytes())
}

/// Creates a slot for a namespace and key.
pub fn slot_for(namespace: &[u8; 32], key: &[u8]) -> Slot {
    Slot::derived(namespace, &[key])
}

/// Creates a slot for a namespace and multiple key parts.
pub fn slot_for_parts(namespace: &[u8; 32], parts: &[&[u8]]) -> Slot {
    Slot::derived(namespace, parts)
}

/// Macro for creating a `Slot` from a 32-byte array.
#[macro_export]
macro_rules! slot {
    ($bytes:expr) => {{
        $crate::storage::Slot::new($bytes)
    }};
}
