//! High-level storage collections built on 32-byte slots.
//!
//! This module provides three storage collection types that abstract over raw slot operations:
//!
//! - **`StorageMap<V>`**: Key-value map (like `HashMap`)
//! - **`StorageVec<V>`**: Dynamic array (like `Vec`)
//! - **`StorageBlob`**: Variable-length byte storage
//!
//! All collections use namespace isolation to prevent slot collisions and support
//! both production (`HostStorage`) and testing (`MemoryStorage`) backends.
//!
//! # Namespace Isolation
//!
//! Each collection requires a unique namespace (32-byte identifier) to isolate
//! its storage slots from other collections:
//!
//! ```ignore
//! const NS_BALANCES: Namespace = Namespace([1u8; 32]);
//! const NS_ALLOWANCES: Namespace = Namespace([2u8; 32]);
//!
//! let balances = StorageMap::<u64>::new(NS_BALANCES);
//! let allowances = StorageMap::<u64>::new(NS_ALLOWANCES);
//! ```
//!
//! # Dual API Pattern
//!
//! Each collection provides two API surfaces:
//!
//! - **`_in()` methods**: Accept explicit backend parameter (for testing)
//! - **Regular methods**: Use `HostStorage` implicitly (for production)
//!
//! ```ignore
//! // Production: uses HostStorage
//! map.insert(b"key", &value)?;
//!
//! // Testing: explicit backend
//! let mut storage = MemoryStorage::new();
//! map.insert_in(&mut storage, b"key", &value)?;
//! ```
//!
//! # Example: Token Balances
//!
//! ```ignore
//! use truthlinked_sdk::collections::{Namespace, StorageMap};
//!
//! const NS_BALANCES: Namespace = Namespace([1u8; 32]);
//!
//! fn transfer(from: [u8; 32], to: [u8; 32], amount: u64) -> Result<()> {
//!     let balances = StorageMap::<u64>::new(NS_BALANCES);
//!     
//!     let from_balance = balances.get_typed_key(&from)?.unwrap_or(0);
//!     let to_balance = balances.get_typed_key(&to)?.unwrap_or(0);
//!     
//!     if from_balance < amount {
//!         return Err(Error::new(ERR_INSUFFICIENT_BALANCE));
//!     }
//!     
//!     balances.insert_typed_key(&from, &(from_balance - amount))?;
//!     balances.insert_typed_key(&to, &(to_balance + amount))?;
//!     
//!     Ok(())
//! }
//! ```

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::backend::{HostStorage, StorageBackend};
use crate::codec::{BytesCodec, Codec32};
use crate::error::Result;
use crate::hashing;

/// A 32-byte namespace identifier for storage isolation.
///
/// Namespaces prevent slot collisions between different collections.
/// Each collection should use a unique namespace.
///
/// # Example
///
/// ```ignore
/// const NS_BALANCES: Namespace = Namespace([1u8; 32]);
/// const NS_ALLOWANCES: Namespace = Namespace([2u8; 32]);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Namespace(pub [u8; 32]);

impl Namespace {
    /// Creates a new namespace from a 32-byte array.
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// A persistent key-value map stored in contract storage.
///
/// `StorageMap` provides a `HashMap`-like interface over contract storage slots.
/// Each entry uses two slots: one for existence check, one for the value.
///
/// # Type Parameters
///
/// * `V` - Value type, must implement `Codec32` for 32-byte encoding
///
/// # Storage Layout
///
/// For each key, two slots are derived:
/// - `derive_slot(namespace, ["map:exists", key])` - Existence flag (1 byte)
/// - `derive_slot(namespace, ["map:value", key])` - Encoded value (32 bytes)
///
/// # Example
///
/// ```ignore
/// const NS_BALANCES: Namespace = Namespace([1u8; 32]);
/// let balances = StorageMap::<u64>::new(NS_BALANCES);
///
/// // Insert
/// balances.insert(b"alice", &1000)?;
///
/// // Get
/// let balance = balances.get(b"alice")?.unwrap_or(0);
///
/// // Check existence
/// if balances.contains_key(b"alice")? {
///     // Key exists
/// }
///
/// // Remove
/// balances.remove(b"alice")?;
/// ```
pub struct StorageMap<V: Codec32> {
    namespace: Namespace,
    _marker: PhantomData<V>,
}

impl<V: Codec32> StorageMap<V> {
    /// Creates a new map with the specified namespace.
    ///
    /// # Arguments
    ///
    /// * `namespace` - Unique 32-byte identifier for this map
    pub const fn new(namespace: Namespace) -> Self {
        Self {
            namespace,
            _marker: PhantomData,
        }
    }

    /// Computes the existence slot for a given key.
    ///
    /// Returns the storage slot address where the existence flag is stored.
    pub fn exists_slot_for(&self, key: &[u8]) -> [u8; 32] {
        hashing::derive_slot(&self.namespace.0, &[b"map:exists", key])
    }

    /// Computes the value slot for a given key.
    ///
    /// Returns the storage slot address where the encoded value is stored.
    pub fn value_slot_for(&self, key: &[u8]) -> [u8; 32] {
        hashing::derive_slot(&self.namespace.0, &[b"map:value", key])
    }

    /// Returns both slots (existence, value) for a given key.
    ///
    /// Useful for manifest generation.
    pub fn slots_for_key(&self, key: &[u8]) -> ([u8; 32], [u8; 32]) {
        (self.exists_slot_for(key), self.value_slot_for(key))
    }

    /// Checks if a key exists in the map (with explicit backend).
    pub fn contains_key_in<B: StorageBackend>(&self, backend: &B, key: &[u8]) -> Result<bool> {
        let exists = backend.read_32(&self.exists_slot_for(key))?;
        Ok(exists[0] == 1)
    }

    /// Gets the value for a key (with explicit backend).
    ///
    /// Returns `None` if the key doesn't exist.
    pub fn get_in<B: StorageBackend>(&self, backend: &B, key: &[u8]) -> Result<Option<V>> {
        if !self.contains_key_in(backend, key)? {
            return Ok(None);
        }
        let raw = backend.read_32(&self.value_slot_for(key))?;
        Ok(Some(V::decode_32(&raw)?))
    }

    /// Inserts a key-value pair (with explicit backend).
    pub fn insert_in<B: StorageBackend>(
        &self,
        backend: &mut B,
        key: &[u8],
        value: &V,
    ) -> Result<()> {
        let mut exists = [0u8; 32];
        exists[0] = 1;
        backend.write_32(self.exists_slot_for(key), exists)?;
        backend.write_32(self.value_slot_for(key), value.encode_32())?;
        Ok(())
    }

    /// Removes a key-value pair (with explicit backend).
    pub fn remove_in<B: StorageBackend>(&self, backend: &mut B, key: &[u8]) -> Result<()> {
        backend.write_32(self.exists_slot_for(key), [0u8; 32])?;
        backend.write_32(self.value_slot_for(key), [0u8; 32])?;
        Ok(())
    }

    /// Checks if a typed key exists (with explicit backend).
    ///
    /// The key is encoded using `BytesCodec` before lookup.
    pub fn contains_typed_key_in<B: StorageBackend, K: BytesCodec>(
        &self,
        backend: &B,
        key: &K,
    ) -> Result<bool> {
        let key_bytes = key.encode_bytes();
        self.contains_key_in(backend, &key_bytes)
    }

    /// Gets the value for a typed key (with explicit backend).
    pub fn get_typed_key_in<B: StorageBackend, K: BytesCodec>(
        &self,
        backend: &B,
        key: &K,
    ) -> Result<Option<V>> {
        let key_bytes = key.encode_bytes();
        self.get_in(backend, &key_bytes)
    }

    /// Inserts a typed key-value pair (with explicit backend).
    pub fn insert_typed_key_in<B: StorageBackend, K: BytesCodec>(
        &self,
        backend: &mut B,
        key: &K,
        value: &V,
    ) -> Result<()> {
        let key_bytes = key.encode_bytes();
        self.insert_in(backend, &key_bytes, value)
    }

    /// Removes a typed key (with explicit backend).
    pub fn remove_typed_key_in<B: StorageBackend, K: BytesCodec>(
        &self,
        backend: &mut B,
        key: &K,
    ) -> Result<()> {
        let key_bytes = key.encode_bytes();
        self.remove_in(backend, &key_bytes)
    }

    /// Checks if a key exists (production, uses `HostStorage`).
    pub fn contains_key(&self, key: &[u8]) -> Result<bool> {
        let host = HostStorage;
        self.contains_key_in(&host, key)
    }

    /// Gets the value for a key (production, uses `HostStorage`).
    pub fn get(&self, key: &[u8]) -> Result<Option<V>> {
        let host = HostStorage;
        self.get_in(&host, key)
    }

    /// Inserts a key-value pair (production, uses `HostStorage`).
    pub fn insert(&self, key: &[u8], value: &V) -> Result<()> {
        let mut host = HostStorage;
        self.insert_in(&mut host, key, value)
    }

    /// Removes a key-value pair (production, uses `HostStorage`).
    pub fn remove(&self, key: &[u8]) -> Result<()> {
        let mut host = HostStorage;
        self.remove_in(&mut host, key)
    }

    /// Checks if a typed key exists (production, uses `HostStorage`).
    pub fn contains_typed_key<K: BytesCodec>(&self, key: &K) -> Result<bool> {
        let host = HostStorage;
        self.contains_typed_key_in(&host, key)
    }

    /// Gets the value for a typed key (production, uses `HostStorage`).
    pub fn get_typed_key<K: BytesCodec>(&self, key: &K) -> Result<Option<V>> {
        let host = HostStorage;
        self.get_typed_key_in(&host, key)
    }

    /// Inserts a typed key-value pair (production, uses `HostStorage`).
    pub fn insert_typed_key<K: BytesCodec>(&self, key: &K, value: &V) -> Result<()> {
        let mut host = HostStorage;
        self.insert_typed_key_in(&mut host, key, value)
    }

    /// Removes a typed key (production, uses `HostStorage`).
    pub fn remove_typed_key<K: BytesCodec>(&self, key: &K) -> Result<()> {
        let mut host = HostStorage;
        self.remove_typed_key_in(&mut host, key)
    }
}

/// A persistent dynamic array stored in contract storage.
///
/// `StorageVec` provides a `Vec`-like interface over contract storage slots.
/// It stores a length counter and individual elements at derived slot addresses.
///
/// # Type Parameters
///
/// * `V` - Element type, must implement `Codec32` for 32-byte encoding
///
/// # Storage Layout
///
/// - `derive_slot(namespace, ["vec:len"])` - Length as u64
/// - `derive_slot(namespace, ["vec:elem", index])` - Element at index
///
/// # Example
///
/// ```ignore
/// const NS_HISTORY: Namespace = Namespace([1u8; 32]);
/// let history = StorageVec::<u64>::new(NS_HISTORY);
///
/// // Push elements
/// history.push(&100)?;
/// history.push(&200)?;
///
/// // Get length
/// let len = history.len()?; // 2
///
/// // Get element
/// let first = history.get(0)?.unwrap(); // 100
///
/// // Pop element
/// let last = history.pop()?.unwrap(); // 200
/// ```
pub struct StorageVec<V: Codec32> {
    namespace: Namespace,
    _marker: PhantomData<V>,
}

impl<V: Codec32> StorageVec<V> {
    /// Creates a new vector with the specified namespace.
    pub const fn new(namespace: Namespace) -> Self {
        Self {
            namespace,
            _marker: PhantomData,
        }
    }

    /// Returns the storage slot for the length counter.
    pub fn len_slot(&self) -> [u8; 32] {
        hashing::derive_slot(&self.namespace.0, &[b"vec:len"])
    }

    /// Returns the storage slot for an element at the given index.
    pub fn slot_for_index(&self, index: u64) -> [u8; 32] {
        let idx = hashing::index_u64(index);
        hashing::derive_slot(&self.namespace.0, &[b"vec:elem", &idx])
    }

    /// Returns the number of elements (with explicit backend).
    pub fn len_in<B: StorageBackend>(&self, backend: &B) -> Result<u64> {
        let raw = backend.read_32(&self.len_slot())?;
        <u64 as Codec32>::decode_32(&raw)
    }

    /// Checks if the vector is empty (with explicit backend).
    pub fn is_empty_in<B: StorageBackend>(&self, backend: &B) -> Result<bool> {
        Ok(self.len_in(backend)? == 0)
    }

    /// Gets the element at index (with explicit backend).
    ///
    /// Returns `None` if index is out of bounds.
    pub fn get_in<B: StorageBackend>(&self, backend: &B, index: u64) -> Result<Option<V>> {
        let len = self.len_in(backend)?;
        if index >= len {
            return Ok(None);
        }
        let raw = backend.read_32(&self.slot_for_index(index))?;
        Ok(Some(V::decode_32(&raw)?))
    }

    /// Sets the element at index (with explicit backend).
    ///
    /// Returns `false` if index is out of bounds.
    pub fn set_in<B: StorageBackend>(
        &self,
        backend: &mut B,
        index: u64,
        value: &V,
    ) -> Result<bool> {
        let len = self.len_in(backend)?;
        if index >= len {
            return Ok(false);
        }
        backend.write_32(self.slot_for_index(index), value.encode_32())?;
        Ok(true)
    }

    /// Appends an element to the end (with explicit backend).
    ///
    /// Returns the index where the element was inserted.
    pub fn push_in<B: StorageBackend>(&self, backend: &mut B, value: &V) -> Result<u64> {
        let len = self.len_in(backend)?;
        backend.write_32(self.slot_for_index(len), value.encode_32())?;
        backend.write_32(self.len_slot(), (len + 1).encode_32())?;
        Ok(len)
    }

    /// Removes and returns the last element (with explicit backend).
    ///
    /// Returns `None` if the vector is empty.
    pub fn pop_in<B: StorageBackend>(&self, backend: &mut B) -> Result<Option<V>> {
        let len = self.len_in(backend)?;
        if len == 0 {
            return Ok(None);
        }
        let index = len - 1;
        let raw = backend.read_32(&self.slot_for_index(index))?;
        backend.write_32(self.len_slot(), index.encode_32())?;
        backend.write_32(self.slot_for_index(index), [0u8; 32])?;
        Ok(Some(V::decode_32(&raw)?))
    }

    /// Clears the vector by setting length to 0 (with explicit backend).
    ///
    /// Note: This doesn't zero out element slots, just resets the length.
    pub fn clear_in<B: StorageBackend>(&self, backend: &mut B) -> Result<()> {
        backend.write_32(self.len_slot(), 0u64.encode_32())
    }

    /// Returns the number of elements (production, uses `HostStorage`).
    pub fn len(&self) -> Result<u64> {
        let host = HostStorage;
        self.len_in(&host)
    }

    /// Gets the element at index (production, uses `HostStorage`).
    pub fn get(&self, index: u64) -> Result<Option<V>> {
        let host = HostStorage;
        self.get_in(&host, index)
    }

    /// Sets the element at index (production, uses `HostStorage`).
    pub fn set(&self, index: u64, value: &V) -> Result<bool> {
        let mut host = HostStorage;
        self.set_in(&mut host, index, value)
    }

    /// Appends an element to the end (production, uses `HostStorage`).
    pub fn push(&self, value: &V) -> Result<u64> {
        let mut host = HostStorage;
        self.push_in(&mut host, value)
    }

    /// Removes and returns the last element (production, uses `HostStorage`).
    pub fn pop(&self) -> Result<Option<V>> {
        let mut host = HostStorage;
        self.pop_in(&mut host)
    }

    /// Clears the vector (production, uses `HostStorage`).
    pub fn clear(&self) -> Result<()> {
        let mut host = HostStorage;
        self.clear_in(&mut host)
    }
}

/// A persistent variable-length byte storage.
///
/// `StorageBlob` stores arbitrary-length byte arrays by chunking them into
/// 32-byte segments. Useful for storing strings, serialized structs, or large data.
///
/// # Storage Layout
///
/// - `derive_slot(namespace, ["blob:len"])` - Length in bytes as u64
/// - `derive_slot(namespace, ["blob:chunk", i])` - Chunk i (32 bytes)
///
/// # Example
///
/// ```ignore
/// const NS_METADATA: Namespace = Namespace([1u8; 32]);
/// let metadata = StorageBlob::new(NS_METADATA);
///
/// // Write raw bytes
/// metadata.write(b"Hello, TruthLinked!")?;
///
/// // Read raw bytes
/// let data = metadata.read()?;
///
/// // Write typed value
/// let config = Config { version: 1, enabled: true };
/// metadata.put(&config)?;
///
/// // Read typed value
/// let config: Config = metadata.get()?;
/// ```
pub struct StorageBlob {
    namespace: Namespace,
}

impl StorageBlob {
    /// Creates a new blob with the specified namespace.
    pub const fn new(namespace: Namespace) -> Self {
        Self { namespace }
    }

    /// Returns the storage slot for the length counter.
    pub fn len_slot(&self) -> [u8; 32] {
        hashing::derive_slot(&self.namespace.0, &[b"blob:len"])
    }

    /// Returns the storage slot for a chunk at the given index.
    pub fn slot_for_chunk(&self, index: u64) -> [u8; 32] {
        let idx = hashing::index_u64(index);
        hashing::derive_slot(&self.namespace.0, &[b"blob:chunk", &idx])
    }

    /// Writes raw bytes to storage (with explicit backend).
    ///
    /// Data is chunked into 32-byte segments. The length is stored separately.
    pub fn write_in<B: StorageBackend>(&self, backend: &mut B, data: &[u8]) -> Result<()> {
        let chunks = if data.is_empty() {
            0
        } else {
            ((data.len() - 1) / 32) + 1
        };
        for i in 0..chunks {
            let start = i * 32;
            let end = (start + 32).min(data.len());
            let mut chunk = [0u8; 32];
            chunk[..(end - start)].copy_from_slice(&data[start..end]);
            backend.write_32(self.slot_for_chunk(i as u64), chunk)?;
        }
        backend.write_32(self.len_slot(), (data.len() as u64).encode_32())?;
        Ok(())
    }

    /// Reads raw bytes from storage (with explicit backend).
    ///
    /// Reconstructs the original byte array from stored chunks.
    pub fn read_in<B: StorageBackend>(&self, backend: &B) -> Result<Vec<u8>> {
        let raw_len = backend.read_32(&self.len_slot())?;
        let len = <u64 as Codec32>::decode_32(&raw_len)? as usize;
        if len == 0 {
            return Ok(Vec::new());
        }
        let chunks = ((len - 1) / 32) + 1;
        let mut out = vec![0u8; chunks * 32];
        for i in 0..chunks {
            let chunk = backend.read_32(&self.slot_for_chunk(i as u64))?;
            let start = i * 32;
            out[start..start + 32].copy_from_slice(&chunk);
        }
        out.truncate(len);
        Ok(out)
    }

    /// Clears the blob by setting length to 0 (with explicit backend).
    pub fn clear_in<B: StorageBackend>(&self, backend: &mut B) -> Result<()> {
        backend.write_32(self.len_slot(), 0u64.encode_32())
    }

    /// Writes a typed value to storage (with explicit backend).
    ///
    /// The value is encoded using `BytesCodec` before storage.
    pub fn put_in<B: StorageBackend, T: BytesCodec>(
        &self,
        backend: &mut B,
        value: &T,
    ) -> Result<()> {
        self.write_in(backend, &value.encode_bytes())
    }

    /// Reads a typed value from storage (with explicit backend).
    ///
    /// The bytes are decoded using `BytesCodec`.
    pub fn get_in<B: StorageBackend, T: BytesCodec>(&self, backend: &B) -> Result<T> {
        let raw = self.read_in(backend)?;
        T::decode_bytes(&raw)
    }

    /// Writes raw bytes to storage (production, uses `HostStorage`).
    pub fn write(&self, data: &[u8]) -> Result<()> {
        let mut host = HostStorage;
        self.write_in(&mut host, data)
    }

    /// Reads raw bytes from storage (production, uses `HostStorage`).
    pub fn read(&self) -> Result<Vec<u8>> {
        let host = HostStorage;
        self.read_in(&host)
    }

    /// Writes a typed value to storage (production, uses `HostStorage`).
    pub fn put<T: BytesCodec>(&self, value: &T) -> Result<()> {
        let mut host = HostStorage;
        self.put_in(&mut host, value)
    }

    /// Reads a typed value from storage (production, uses `HostStorage`).
    pub fn get<T: BytesCodec>(&self) -> Result<T> {
        let host = HostStorage;
        self.get_in(&host)
    }

    /// Clears the blob (production, uses `HostStorage`).
    pub fn clear(&self) -> Result<()> {
        let mut host = HostStorage;
        self.clear_in(&mut host)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MemoryStorage;

    const NS_MAP: Namespace = Namespace([1u8; 32]);
    const NS_VEC: Namespace = Namespace([2u8; 32]);
    const NS_BLOB: Namespace = Namespace([3u8; 32]);

    #[test]
    fn map_roundtrip() {
        let mut mem = MemoryStorage::new();
        let map = StorageMap::<u64>::new(NS_MAP);

        assert_eq!(map.get_in(&mem, b"alice").unwrap(), None);
        map.insert_in(&mut mem, b"alice", &42).unwrap();
        assert_eq!(map.get_in(&mem, b"alice").unwrap(), Some(42));
        map.remove_in(&mut mem, b"alice").unwrap();
        assert_eq!(map.get_in(&mem, b"alice").unwrap(), None);
    }

    #[test]
    fn vec_roundtrip() {
        let mut mem = MemoryStorage::new();
        let list = StorageVec::<u64>::new(NS_VEC);

        assert_eq!(list.len_in(&mem).unwrap(), 0);
        list.push_in(&mut mem, &7).unwrap();
        list.push_in(&mut mem, &9).unwrap();
        assert_eq!(list.len_in(&mem).unwrap(), 2);
        assert_eq!(list.get_in(&mem, 1).unwrap(), Some(9));
        assert_eq!(list.pop_in(&mut mem).unwrap(), Some(9));
        assert_eq!(list.len_in(&mem).unwrap(), 1);
    }

    #[test]
    fn blob_roundtrip() {
        let mut mem = MemoryStorage::new();
        let blob = StorageBlob::new(NS_BLOB);
        let data = b"hello storage blob".to_vec();

        blob.write_in(&mut mem, &data).unwrap();
        assert_eq!(blob.read_in(&mem).unwrap(), data);
    }

    #[test]
    fn map_typed_keys_roundtrip() {
        let mut mem = MemoryStorage::new();
        let map = StorageMap::<u64>::new(NS_MAP);
        let key = [7u8; 32];

        map.insert_typed_key_in(&mut mem, &key, &88).unwrap();
        assert_eq!(map.get_typed_key_in(&mem, &key).unwrap(), Some(88));
        map.remove_typed_key_in(&mut mem, &key).unwrap();
        assert_eq!(map.get_typed_key_in(&mem, &key).unwrap(), None);
    }
}
