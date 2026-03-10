//! Storage backend abstraction for contract state persistence.
//!
//! This module provides a trait-based abstraction over storage operations,
//! enabling both production (on-chain) and testing (in-memory) backends.
//!
//! # Storage Backends
//!
//! - **`HostStorage`**: Production backend that delegates to WASM host imports.
//!   Used when contracts run on-chain.
//! - **`MemoryStorage`**: In-memory backend using `BTreeMap` for testing.
//!   Enables fast, isolated unit tests without WASM runtime.
//!
//! # Example
//!
//! ```ignore
//! use truthlinked_sdk::backend::{StorageBackend, MemoryStorage};
//!
//! // Testing with in-memory storage
//! let mut storage = MemoryStorage::new();
//! let slot = [1u8; 32];
//! storage.write_32(slot, [42u8; 32])?;
//! let value = storage.read_32(&slot)?;
//! assert_eq!(value[0], 42);
//! ```

extern crate alloc;

use alloc::collections::BTreeMap;

use crate::env;
use crate::error::Result;

/// Trait for storage backends that support 32-byte slot read/write operations.
///
/// All storage in TruthLinked contracts operates on fixed 32-byte slots.
/// This trait abstracts the underlying storage mechanism, allowing contracts
/// to work with both on-chain storage (via WASM host) and in-memory storage (for testing).
pub trait StorageBackend {
    /// Reads a 32-byte value from the specified storage slot.
    ///
    /// # Arguments
    ///
    /// * `key` - The 32-byte storage slot address
    ///
    /// # Returns
    ///
    /// The 32-byte value stored at the slot, or `[0u8; 32]` if uninitialized.
    fn read_32(&self, key: &[u8; 32]) -> Result<[u8; 32]>;

    /// Writes a 32-byte value to the specified storage slot.
    ///
    /// # Arguments
    ///
    /// * `key` - The 32-byte storage slot address
    /// * `value` - The 32-byte value to store
    fn write_32(&mut self, key: [u8; 32], value: [u8; 32]) -> Result<()>;
}

/// Storage backend that delegates to WASM host imports.
///
/// This is a zero-sized type that forwards all storage operations to the
/// TruthLinked runtime via host function calls. Used when contracts execute on-chain.
///
/// # Example
///
/// ```ignore
/// use truthlinked_sdk::backend::{HostStorage, StorageBackend};
///
/// let storage = HostStorage;
/// let value = storage.read_32(&slot)?;
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct HostStorage;

impl StorageBackend for HostStorage {
    fn read_32(&self, key: &[u8; 32]) -> Result<[u8; 32]> {
        env::storage_read_32(key)
    }

    fn write_32(&mut self, key: [u8; 32], value: [u8; 32]) -> Result<()> {
        env::storage_write_32(&key, &value)
    }
}

/// In-memory storage backend for testing and development.
///
/// Uses a `BTreeMap` to store slot values in memory. This enables fast,
/// isolated unit tests without requiring a WASM runtime or blockchain.
///
/// # Example
///
/// ```ignore
/// use truthlinked_sdk::backend::MemoryStorage;
///
/// let mut storage = MemoryStorage::new()
///     .with_slot([1u8; 32], [42u8; 32])
///     .with_slot([2u8; 32], [99u8; 32]);
///
/// let snapshot = storage.snapshot(); // Clone current state
/// ```
#[derive(Clone, Debug, Default)]
pub struct MemoryStorage {
    slots: BTreeMap<[u8; 32], [u8; 32]>,
}

impl MemoryStorage {
    /// Creates a new empty in-memory storage backend.
    pub fn new() -> Self {
        Self {
            slots: BTreeMap::new(),
        }
    }

    /// Builder method to pre-populate a storage slot.
    ///
    /// Useful for setting up test fixtures with initial state.
    ///
    /// # Arguments
    ///
    /// * `key` - Storage slot address
    /// * `value` - Initial value for the slot
    ///
    /// # Example
    ///
    /// ```ignore
    /// let storage = MemoryStorage::new()
    ///     .with_slot([1u8; 32], [42u8; 32]);
    /// ```
    pub fn with_slot(mut self, key: [u8; 32], value: [u8; 32]) -> Self {
        self.slots.insert(key, value);
        self
    }

    /// Creates a snapshot of the current storage state.
    ///
    /// Returns a clone of the internal slot map. Useful for:
    /// - Saving state before mutations
    /// - Comparing state changes in tests
    /// - Debugging storage operations
    ///
    /// # Returns
    ///
    /// A `BTreeMap` containing all current slot values.
    pub fn snapshot(&self) -> BTreeMap<[u8; 32], [u8; 32]> {
        self.slots.clone()
    }
}

impl StorageBackend for MemoryStorage {
    fn read_32(&self, key: &[u8; 32]) -> Result<[u8; 32]> {
        Ok(self.slots.get(key).copied().unwrap_or([0u8; 32]))
    }

    fn write_32(&mut self, key: [u8; 32], value: [u8; 32]) -> Result<()> {
        self.slots.insert(key, value);
        Ok(())
    }
}
