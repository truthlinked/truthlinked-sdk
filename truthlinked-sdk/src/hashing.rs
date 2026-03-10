//! Cryptographic hashing utilities.
//!
//! This module provides SHA-256 hashing functions for deriving storage slots
//! and creating namespaces.

use sha2::{Digest, Sha256};

/// Computes the SHA-256 hash of the given data.
pub fn hash32(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    let out = h.finalize();
    let mut slot = [0u8; 32];
    slot.copy_from_slice(&out);
    slot
}

/// Derives a storage slot from a namespace and key parts.
///
/// Uses a domain-separated hash to prevent collisions.
pub fn derive_slot(namespace: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"trth:sdk:slot:v1");
    h.update(&[0u8]);
    h.update(namespace);
    for part in parts {
        h.update(&[0xFF]);
        h.update(part);
    }
    let out = h.finalize();
    let mut slot = [0u8; 32];
    slot.copy_from_slice(&out);
    slot
}

/// Converts a `u64` index to little-endian bytes.
#[inline]
pub fn index_u64(i: u64) -> [u8; 8] {
    i.to_le_bytes()
}

/// Creates a namespace hash from a label.
#[inline]
pub fn namespace(label: &str) -> [u8; 32] {
    hash32(label.as_bytes())
}
