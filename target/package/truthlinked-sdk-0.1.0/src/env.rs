//! Low-level WASM host environment bindings.
//!
//! This module provides direct access to the TruthLinked runtime host functions
//! via WASM imports. These are the raw building blocks used by higher-level SDK APIs.
//!
//! Most contract developers should use the higher-level APIs in other modules
//! rather than calling these functions directly.

use crate::error::{Error, Result};

/// Size of storage keys in bytes (32 bytes / 256 bits).
pub const STORAGE_KEY_BYTES: usize = 32;

/// Size of storage values in bytes (32 bytes / 256 bits).
pub const STORAGE_VALUE_BYTES: usize = 32;

/// Maximum size of calldata in bytes (256 KB).
pub const MAX_CALLDATA_SIZE: usize = 262_144;

/// Maximum size of return data in bytes (256 KB).
pub const MAX_RETURN_DATA_SIZE: usize = 262_144;

/// Maximum size of log data in bytes (64 KB).
pub const MAX_LOG_DATA_SIZE: usize = 65_536;

/// Maximum number of topics per log event.
pub const MAX_LOG_TOPICS: usize = 8;

/// Oracle error codes returned by `http_call`.
pub mod oracle_rc {
    /// Memory allocation error.
    pub const MEM_ERR: i32 = -1;
    /// Encoding/decoding error.
    pub const ENCODING_ERR: i32 = -2;
    /// URL not in approved list.
    pub const URL_NOT_APPROVED: i32 = -3;
    /// Oracle request is pending.
    pub const PENDING: i32 = -5;
    /// Oracle request expired.
    pub const EXPIRED: i32 = -6;
    /// Response too large.
    pub const RESPONSE_TOO_LARGE: i32 = -7;
    /// Oracle depth limit exceeded.
    pub const DEPTH_LIMIT_EXCEEDED: i32 = -8;
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
extern "C" {
    fn storage_read(key_ptr: i32, key_len: i32, val_ptr: i32, val_len: i32) -> i32;
    fn storage_write(key_ptr: i32, key_len: i32, val_ptr: i32, val_len: i32) -> i32;

    fn get_caller(ptr: i32) -> i32;
    fn get_owner(ptr: i32) -> i32;
    fn get_contract_id(ptr: i32) -> i32;
    fn get_height(ptr: i32) -> i32;
    fn get_timestamp(ptr: i32) -> i32;
    fn get_value(ptr: i32) -> i32;
    fn get_calldata(ptr: i32) -> i32;

    fn set_return_data(ptr: i32, len: i32) -> i32;
    fn emit_log(topics_ptr: i32, topics_len: i32, data_ptr: i32, data_len: i32) -> i32;

    fn call_contract(
        contract_ptr: i32,
        calldata_ptr: i32,
        calldata_len: i32,
        result_ptr: i32,
    ) -> i32;
    fn call_contract_v2(
        contract_ptr: i32,
        calldata_ptr: i32,
        calldata_len: i32,
        value_lo: i64,
        value_hi: i64,
        result_ptr: i32,
        result_len: i32,
    ) -> i32;

    fn http_call(
        url_ptr: i32,
        url_len: i32,
        method_ptr: i32,
        method_len: i32,
        body_ptr: i32,
        body_len: i32,
        result_ptr: i32,
    ) -> i32;
}

#[cfg(not(target_arch = "wasm32"))]
mod host_stub {
    // Host stubs are intentionally explicit to make host-side tests fail loudly.
    pub unsafe fn storage_read(_: i32, _: i32, _: i32, _: i32) -> i32 {
        -127
    }
    pub unsafe fn storage_write(_: i32, _: i32, _: i32, _: i32) -> i32 {
        -127
    }

    pub unsafe fn get_caller(_: i32) -> i32 {
        -127
    }
    pub unsafe fn get_owner(_: i32) -> i32 {
        -127
    }
    pub unsafe fn get_contract_id(_: i32) -> i32 {
        -127
    }
    pub unsafe fn get_height(_: i32) -> i32 {
        -127
    }
    pub unsafe fn get_timestamp(_: i32) -> i32 {
        -127
    }
    pub unsafe fn get_value(_: i32) -> i32 {
        -127
    }
    pub unsafe fn get_calldata(_: i32) -> i32 {
        -127
    }

    pub unsafe fn set_return_data(_: i32, _: i32) -> i32 {
        -127
    }
    pub unsafe fn emit_log(_: i32, _: i32, _: i32, _: i32) -> i32 {
        -127
    }

    pub unsafe fn call_contract(_: i32, _: i32, _: i32, _: i32) -> i32 {
        -127
    }
    pub unsafe fn call_contract_v2(_: i32, _: i32, _: i32, _: i64, _: i64, _: i32, _: i32) -> i32 {
        -127
    }

    pub unsafe fn http_call(_: i32, _: i32, _: i32, _: i32, _: i32, _: i32, _: i32) -> i32 {
        -127
    }
}

#[cfg(not(target_arch = "wasm32"))]
use host_stub::*;

/// Checks a return code from a host function.
///
/// Returns `Ok(rc)` if non-negative, otherwise wraps in `Error`.
#[inline]
fn check_rc(rc: i32) -> Result<i32> {
    if rc < 0 {
        return Err(Error::new(rc));
    }
    Ok(rc)
}

/// Reads a 32-byte value from storage at the given 32-byte key.
pub fn storage_read_32(key: &[u8; STORAGE_KEY_BYTES]) -> Result<[u8; STORAGE_VALUE_BYTES]> {
    let mut out = [0u8; STORAGE_VALUE_BYTES];
    let rc = unsafe {
        storage_read(
            key.as_ptr() as i32,
            STORAGE_KEY_BYTES as i32,
            out.as_mut_ptr() as i32,
            STORAGE_VALUE_BYTES as i32,
        )
    };
    check_rc(rc)?;
    Ok(out)
}

/// Writes a 32-byte value to storage at the given 32-byte key.
pub fn storage_write_32(
    key: &[u8; STORAGE_KEY_BYTES],
    value: &[u8; STORAGE_VALUE_BYTES],
) -> Result<()> {
    let rc = unsafe {
        storage_write(
            key.as_ptr() as i32,
            STORAGE_KEY_BYTES as i32,
            value.as_ptr() as i32,
            STORAGE_VALUE_BYTES as i32,
        )
    };
    check_rc(rc)?;
    Ok(())
}

/// Retrieves the caller's address (32 bytes).
pub fn get_caller_32() -> Result<[u8; STORAGE_KEY_BYTES]> {
    let mut out = [0u8; STORAGE_KEY_BYTES];
    let rc = unsafe { get_caller(out.as_mut_ptr() as i32) };
    check_rc(rc)?;
    Ok(out)
}

/// Retrieves the contract owner's address (32 bytes).
pub fn get_owner_32() -> Result<[u8; STORAGE_KEY_BYTES]> {
    let mut out = [0u8; STORAGE_KEY_BYTES];
    let rc = unsafe { get_owner(out.as_mut_ptr() as i32) };
    check_rc(rc)?;
    Ok(out)
}

/// Retrieves the current contract's ID (32 bytes).
pub fn get_contract_id_32() -> Result<[u8; STORAGE_KEY_BYTES]> {
    let mut out = [0u8; STORAGE_KEY_BYTES];
    let rc = unsafe { get_contract_id(out.as_mut_ptr() as i32) };
    check_rc(rc)?;
    Ok(out)
}

/// Retrieves the current block height.
pub fn get_height_u64() -> Result<u64> {
    let mut bytes = [0u8; 8];
    let rc = unsafe { get_height(bytes.as_mut_ptr() as i32) };
    check_rc(rc)?;
    Ok(u64::from_le_bytes(bytes))
}

/// Retrieves the current block timestamp (Unix seconds).
pub fn get_timestamp_u64() -> Result<u64> {
    let mut bytes = [0u8; 8];
    let rc = unsafe { get_timestamp(bytes.as_mut_ptr() as i32) };
    check_rc(rc)?;
    Ok(u64::from_le_bytes(bytes))
}

/// Retrieves the value sent with the current call (in smallest units).
pub fn get_value_u128() -> Result<u128> {
    let mut bytes = [0u8; 16];
    let rc = unsafe { get_value(bytes.as_mut_ptr() as i32) };
    check_rc(rc)?;
    Ok(u128::from_le_bytes(bytes))
}

/// Reads the calldata into the provided buffer.
///
/// Returns the actual length of calldata written.
pub fn read_calldata(buf: &mut [u8]) -> Result<usize> {
    if buf.len() > MAX_CALLDATA_SIZE {
        return Err(Error::new(-2));
    }
    let rc = unsafe { get_calldata(buf.as_mut_ptr() as i32) };
    let len = check_rc(rc)? as usize;
    Ok(len.min(buf.len()))
}

/// Sets the return data for the current contract call.
pub fn set_return_data_bytes(bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_RETURN_DATA_SIZE {
        return Err(Error::new(-2));
    }
    let rc = unsafe { set_return_data(bytes.as_ptr() as i32, bytes.len() as i32) };
    check_rc(rc)?;
    Ok(())
}

/// Emits a log event with topics and data.
pub fn emit_log_bytes(flat_topics: &[u8], data: &[u8]) -> Result<()> {
    if data.len() > MAX_LOG_DATA_SIZE {
        return Err(Error::new(-2));
    }
    if flat_topics.len() > MAX_LOG_TOPICS * STORAGE_KEY_BYTES
        || flat_topics.len() % STORAGE_KEY_BYTES != 0
    {
        return Err(Error::new(-2));
    }
    let rc = unsafe {
        emit_log(
            flat_topics.as_ptr() as i32,
            flat_topics.len() as i32,
            data.as_ptr() as i32,
            data.len() as i32,
        )
    };
    check_rc(rc)?;
    Ok(())
}

/// Calls another contract (legacy version without value transfer).
pub fn call_contract_legacy(
    contract_id: &[u8; STORAGE_KEY_BYTES],
    calldata: &[u8],
    out: &mut [u8],
) -> Result<usize> {
    let rc = unsafe {
        call_contract(
            contract_id.as_ptr() as i32,
            calldata.as_ptr() as i32,
            calldata.len() as i32,
            out.as_mut_ptr() as i32,
        )
    };
    Ok(check_rc(rc)? as usize)
}

/// Calls another contract with value transfer (v2 API).
pub fn call_contract_v2_with_value(
    contract_id: &[u8; STORAGE_KEY_BYTES],
    calldata: &[u8],
    value: u128,
    out: &mut [u8],
) -> Result<usize> {
    let lo = value as u64 as i64;
    let hi = (value >> 64) as u64 as i64;
    let rc = unsafe {
        call_contract_v2(
            contract_id.as_ptr() as i32,
            calldata.as_ptr() as i32,
            calldata.len() as i32,
            lo,
            hi,
            out.as_mut_ptr() as i32,
            out.len() as i32,
        )
    };
    Ok(check_rc(rc)? as usize)
}

/// Makes an HTTP oracle call.
///
/// Returns the number of bytes written to `out`, or a negative error code.
pub fn http_call_bytes(url: &[u8], method: &[u8], body: &[u8], out: &mut [u8]) -> Result<usize> {
    let rc = unsafe {
        http_call(
            url.as_ptr() as i32,
            url.len() as i32,
            method.as_ptr() as i32,
            method.len() as i32,
            body.as_ptr() as i32,
            body.len() as i32,
            out.as_mut_ptr() as i32,
        )
    };
    Ok(check_rc(rc)? as usize)
}
