//! HTTP oracle interface for external data access.
//!
//! This module provides functions for making HTTP requests from smart contracts
//! via the TruthLinked oracle system.
//!
//! # Example
//!
//! ```ignore
//! use truthlinked_sdk::oracle::{http_get, OracleStatus};
//!
//! let response = http_get("https://api.example.com/data")?;
//! match response.status {
//!     OracleStatus::Ready => {
//!         // Process response.body
//!     }
//!     OracleStatus::Pending => {
//!         // Retry later
//!     }
//!     OracleStatus::Expired => {
//!         // Request expired
//!     }
//! }
//! ```

extern crate alloc;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::env;
use crate::error::{Error, Result};

/// Status of an oracle request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleStatus {
    /// Response is ready.
    Ready,
    /// Request is pending (retry later).
    Pending,
    /// Request has expired.
    Expired,
}

/// Response from an oracle HTTP call.
#[derive(Debug, Clone)]
pub struct OracleResponse {
    /// Status of the request.
    pub status: OracleStatus,
    /// Response body (empty if not ready).
    pub body: Vec<u8>,
}

/// Makes an HTTP request via the oracle.
///
/// Returns `OracleResponse` with status and body.
pub fn http_call(url: &str, method: &str, body: &[u8]) -> Result<OracleResponse> {
    let mut out = vec![0u8; env::MAX_RETURN_DATA_SIZE];
    match env::http_call_bytes(url.as_bytes(), method.as_bytes(), body, &mut out) {
        Ok(len) => {
            out.truncate(len.min(out.len()));
            Ok(OracleResponse {
                status: OracleStatus::Ready,
                body: out,
            })
        }
        Err(err) if err.code() == env::oracle_rc::PENDING => Ok(OracleResponse {
            status: OracleStatus::Pending,
            body: Vec::new(),
        }),
        Err(err) if err.code() == env::oracle_rc::EXPIRED => Ok(OracleResponse {
            status: OracleStatus::Expired,
            body: Vec::new(),
        }),
        Err(err) => Err(Error::new(err.code())),
    }
}

/// Makes an HTTP GET request.
pub fn http_get(url: &str) -> Result<OracleResponse> {
    http_call(url, "GET", &[])
}

/// Makes an HTTP POST request.
pub fn http_post(url: &str, body: &[u8]) -> Result<OracleResponse> {
    http_call(url, "POST", body)
}

/// Converts the response body to a UTF-8 string.
pub fn body_utf8(response: &OracleResponse) -> core::result::Result<String, core::str::Utf8Error> {
    let s = core::str::from_utf8(&response.body)?;
    Ok(String::from(s))
}
