//! ABI (Application Binary Interface) helpers for function dispatch and calldata parsing.
//!
//! This module provides utilities for:
//! - Computing function selectors from names
//! - Extracting selectors from calldata
//! - Reading typed values from calldata at specific offsets
//!
//! # Function Selectors
//!
//! TruthLinked uses FNV-1a 32-bit hashing for function selectors (4 bytes).
//! This is deterministic, no-std compatible, and provides good distribution.
//!
//! # Example
//!
//! ```ignore
//! use truthlinked_sdk::abi;
//!
//! // Compute selector for "transfer" function
//! let sel = abi::selector_of("transfer");
//!
//! // Parse selector from calldata
//! let calldata = context::calldata()?;
//! let selector = abi::selector(&calldata)?;
//!
//! // Read arguments
//! let amount = abi::read_u64(&calldata, 4)?;
//! let recipient = abi::read_account(&calldata, 12)?;
//! ```

use crate::error::{Error, Result};

/// Error code for malformed or insufficient calldata.
pub const ERR_BAD_CALLDATA: i32 = 10;

/// Computes a 4-byte function selector from a function name.
///
/// Uses FNV-1a 32-bit hash algorithm for deterministic, no-std-friendly hashing.
/// The selector is returned in little-endian byte order.
///
/// # Arguments
///
/// * `name` - Function name (e.g., "transfer", "increment")
///
/// # Returns
///
/// A 4-byte selector that uniquely identifies the function.
///
/// # Example
///
/// ```ignore
/// let sel = abi::selector_of("transfer");
/// // sel is a deterministic 4-byte array
/// ```
pub fn selector_of(name: &str) -> [u8; 4] {
    // FNV-1a 32-bit hash: deterministic, no-std-friendly selector hash
    let mut hash: u32 = 0x811c9dc5;
    for b in name.as_bytes() {
        hash ^= *b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash.to_le_bytes()
}

/// Extracts the 4-byte function selector from calldata.
///
/// The selector is expected to be the first 4 bytes of the calldata.
/// Returns an error if calldata is shorter than 4 bytes.
///
/// # Arguments
///
/// * `calldata` - Raw calldata bytes (selector + arguments)
///
/// # Returns
///
/// * `Ok([u8; 4])` - The extracted selector
/// * `Err(Error)` - If calldata is too short
///
/// # Example
///
/// ```ignore
/// let calldata = context::calldata()?;
/// let selector = abi::selector(&calldata)?;
///
/// if selector == abi::selector_of("transfer") {
///     // Handle transfer function
/// }
/// ```
pub fn selector(calldata: &[u8]) -> Result<[u8; 4]> {
    if calldata.len() < 4 {
        return Err(Error::new(ERR_BAD_CALLDATA));
    }
    let mut out = [0u8; 4];
    out.copy_from_slice(&calldata[..4]);
    Ok(out)
}

/// Reads a `u64` value from calldata at the specified byte offset.
///
/// Interprets 8 bytes starting at `offset` as a little-endian `u64`.
/// Returns an error if there aren't enough bytes available.
///
/// # Arguments
///
/// * `calldata` - Raw calldata bytes
/// * `offset` - Byte offset where the u64 starts (typically 4+ for arguments after selector)
///
/// # Returns
///
/// * `Ok(u64)` - The decoded value
/// * `Err(Error)` - If offset is out of bounds or calldata is too short
///
/// # Example
///
/// ```ignore
/// let calldata = context::calldata()?;
/// let amount = abi::read_u64(&calldata, 4)?; // Read first argument after selector
/// ```
pub fn read_u64(calldata: &[u8], offset: usize) -> Result<u64> {
    let end = offset
        .checked_add(8)
        .ok_or_else(|| Error::new(ERR_BAD_CALLDATA))?;
    if calldata.len() < end {
        return Err(Error::new(ERR_BAD_CALLDATA));
    }
    let mut out = [0u8; 8];
    out.copy_from_slice(&calldata[offset..end]);
    Ok(u64::from_le_bytes(out))
}

/// Reads a `u128` value from calldata at the specified byte offset.
///
/// Interprets 16 bytes starting at `offset` as a little-endian `u128`.
/// Useful for reading large token amounts or 128-bit integers.
///
/// # Arguments
///
/// * `calldata` - Raw calldata bytes
/// * `offset` - Byte offset where the u128 starts
///
/// # Returns
///
/// * `Ok(u128)` - The decoded value
/// * `Err(Error)` - If offset is out of bounds or calldata is too short
///
/// # Example
///
/// ```ignore
/// let calldata = context::calldata()?;
/// let token_amount = abi::read_u128(&calldata, 4)?;
/// ```
pub fn read_u128(calldata: &[u8], offset: usize) -> Result<u128> {
    let end = offset
        .checked_add(16)
        .ok_or_else(|| Error::new(ERR_BAD_CALLDATA))?;
    if calldata.len() < end {
        return Err(Error::new(ERR_BAD_CALLDATA));
    }
    let mut out = [0u8; 16];
    out.copy_from_slice(&calldata[offset..end]);
    Ok(u128::from_le_bytes(out))
}

/// Reads a 32-byte account ID from calldata at the specified byte offset.
///
/// Account IDs in TruthLinked are 32-byte arrays (typically BLAKE3 hashes of public keys).
///
/// # Arguments
///
/// * `calldata` - Raw calldata bytes
/// * `offset` - Byte offset where the account ID starts
///
/// # Returns
///
/// * `Ok([u8; 32])` - The account ID
/// * `Err(Error)` - If offset is out of bounds or calldata is too short
///
/// # Example
///
/// ```ignore
/// let calldata = context::calldata()?;
/// let recipient = abi::read_account(&calldata, 4)?;
/// let sender = context::caller()?;
/// ```
pub fn read_account(calldata: &[u8], offset: usize) -> Result<[u8; 32]> {
    let end = offset
        .checked_add(32)
        .ok_or_else(|| Error::new(ERR_BAD_CALLDATA))?;
    if calldata.len() < end {
        return Err(Error::new(ERR_BAD_CALLDATA));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&calldata[offset..end]);
    Ok(out)
}
