//! Testing utilities for smart contract unit tests.
//!
//! This module provides `StorageHarness` and wrapper types for testing
//! storage operations without deploying to a blockchain.
//!
//! # Example
//!
//! ```ignore
//! use truthlinked_sdk::testing::StorageHarness;
//! use truthlinked_sdk::collections::Namespace;
//!
//! let mut harness = StorageHarness::new();
//! let mut map = harness.map::<u64>(Namespace([1u8; 32]));
//! map.insert(b"key", &42)?;
//! assert_eq!(map.get(b"key")?, Some(42));
//! ```

extern crate alloc;

use alloc::vec::Vec;

use crate::backend::MemoryStorage;
use crate::codec::{BytesCodec, Codec32};
use crate::collections::{Namespace, StorageBlob, StorageMap, StorageVec};
use crate::error::Result;

/// Test harness for storage operations in unit tests.
#[derive(Clone, Debug, Default)]
pub struct StorageHarness {
    backend: MemoryStorage,
}

impl StorageHarness {
    /// Creates a new empty storage harness.
    pub fn new() -> Self {
        Self {
            backend: MemoryStorage::new(),
        }
    }

    /// Creates a harness from an existing memory backend.
    pub fn from_backend(backend: MemoryStorage) -> Self {
        Self { backend }
    }

    /// Returns a reference to the underlying memory backend.
    pub fn backend(&self) -> &MemoryStorage {
        &self.backend
    }

    /// Returns a mutable reference to the underlying memory backend.
    pub fn backend_mut(&mut self) -> &mut MemoryStorage {
        &mut self.backend
    }

    /// Reads a raw 32-byte value from storage.
    pub fn read_raw(&self, key: &[u8; 32]) -> Result<[u8; 32]> {
        use crate::backend::StorageBackend;
        self.backend.read_32(key)
    }

    /// Writes a raw 32-byte value to storage.
    pub fn write_raw(&mut self, key: [u8; 32], value: [u8; 32]) -> Result<()> {
        use crate::backend::StorageBackend;
        self.backend.write_32(key, value)
    }

    /// Creates a test map wrapper for the given namespace.
    pub fn map<V: Codec32>(&mut self, namespace: Namespace) -> TestMap<'_, V> {
        TestMap {
            map: StorageMap::new(namespace),
            backend: &mut self.backend,
        }
    }

    /// Creates a test vector wrapper for the given namespace.
    pub fn vec<V: Codec32>(&mut self, namespace: Namespace) -> TestVec<'_, V> {
        TestVec {
            list: StorageVec::new(namespace),
            backend: &mut self.backend,
        }
    }

    /// Creates a test blob wrapper for the given namespace.
    pub fn blob(&mut self, namespace: Namespace) -> TestBlob<'_> {
        TestBlob {
            blob: StorageBlob::new(namespace),
            backend: &mut self.backend,
        }
    }
}

/// Test wrapper for `StorageMap`.
pub struct TestMap<'a, V: Codec32> {
    map: StorageMap<V>,
    backend: &'a mut MemoryStorage,
}

impl<'a, V: Codec32> TestMap<'a, V> {
    /// Checks if the map contains the given key.
    pub fn contains_key(&self, key: &[u8]) -> Result<bool> {
        self.map.contains_key_in(&*self.backend, key)
    }

    /// Gets the value for the given key.
    pub fn get(&self, key: &[u8]) -> Result<Option<V>> {
        self.map.get_in(&*self.backend, key)
    }

    /// Inserts a key-value pair.
    pub fn insert(&mut self, key: &[u8], value: &V) -> Result<()> {
        self.map.insert_in(self.backend, key, value)
    }

    /// Removes a key-value pair.
    pub fn remove(&mut self, key: &[u8]) -> Result<()> {
        self.map.remove_in(self.backend, key)
    }
}

/// Test wrapper for `StorageVec`.
pub struct TestVec<'a, V: Codec32> {
    list: StorageVec<V>,
    backend: &'a mut MemoryStorage,
}

impl<'a, V: Codec32> TestVec<'a, V> {
    /// Returns the length of the vector.
    pub fn len(&self) -> Result<u64> {
        self.list.len_in(&*self.backend)
    }

    /// Gets the element at the given index.
    pub fn get(&self, index: u64) -> Result<Option<V>> {
        self.list.get_in(&*self.backend, index)
    }

    /// Sets the element at the given index.
    pub fn set(&mut self, index: u64, value: &V) -> Result<bool> {
        self.list.set_in(self.backend, index, value)
    }

    /// Pushes a value to the end of the vector.
    pub fn push(&mut self, value: &V) -> Result<u64> {
        self.list.push_in(self.backend, value)
    }

    /// Pops a value from the end of the vector.
    pub fn pop(&mut self) -> Result<Option<V>> {
        self.list.pop_in(self.backend)
    }

    /// Clears all elements from the vector.
    pub fn clear(&mut self) -> Result<()> {
        self.list.clear_in(self.backend)
    }
}

/// Test wrapper for `StorageBlob`.
pub struct TestBlob<'a> {
    blob: StorageBlob,
    backend: &'a mut MemoryStorage,
}

impl<'a> TestBlob<'a> {
    /// Writes raw bytes to the blob.
    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        self.blob.write_in(self.backend, data)
    }

    /// Reads all bytes from the blob.
    pub fn read(&self) -> Result<Vec<u8>> {
        self.blob.read_in(&*self.backend)
    }

    /// Encodes and writes a typed value to the blob.
    pub fn put<T: BytesCodec>(&mut self, value: &T) -> Result<()> {
        self.blob.put_in(self.backend, value)
    }

    /// Reads and decodes a typed value from the blob.
    pub fn get<T: BytesCodec>(&self) -> Result<T> {
        self.blob.get_in(&*self.backend)
    }

    /// Clears the blob.
    pub fn clear(&mut self) -> Result<()> {
        self.blob.clear_in(self.backend)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NS_MAP: Namespace = Namespace([9u8; 32]);
    const NS_VEC: Namespace = Namespace([8u8; 32]);
    const NS_BLOB: Namespace = Namespace([7u8; 32]);

    #[test]
    fn harness_map_vec_blob() {
        let mut harness = StorageHarness::new();

        {
            let mut map = harness.map::<u64>(NS_MAP);
            map.insert(b"alice", &7).unwrap();
            assert_eq!(map.get(b"alice").unwrap(), Some(7));
        }

        {
            let mut list = harness.vec::<u64>(NS_VEC);
            list.push(&11).unwrap();
            assert_eq!(list.get(0).unwrap(), Some(11));
        }

        {
            let mut blob = harness.blob(NS_BLOB);
            blob.put(&alloc::string::String::from("hello")).unwrap();
            let decoded: alloc::string::String = blob.get().unwrap();
            assert_eq!(decoded, "hello");
        }
    }
}
