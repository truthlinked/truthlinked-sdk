//! Cross-contract call utilities for contract composability.
//!
//! This module provides functions to invoke other contracts from within your contract,
//! enabling composability and modular contract design.
//!
//! # Features
//!
//! - **Legacy calls**: Basic cross-contract invocation without value transfer
//! - **Value transfer**: Call contracts while transferring native tokens
//! - **Return data**: Capture and process return values from called contracts
//!
//! # Example
//!
//! ```ignore
//! use truthlinked_sdk::call;
//!
//! // Call another contract
//! let token_contract = [0x12; 32];
//! let calldata = encode_transfer_call(recipient, amount);
//! let result = call::call_contract(&token_contract, &calldata)?;
//!
//! // Call with value transfer
//! let result = call::call_contract_with_value(
//!     &contract_id,
//!     &calldata,
//!     1000, // Transfer 1000 base units
//! )?;
//! ```

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use crate::env;
use crate::error::Result;

/// Type alias for 32-byte account identifiers.
///
/// Account IDs in TruthLinked are 32-byte arrays, typically BLAKE3 hashes
/// of public keys or deterministic contract addresses.
pub type AccountId = [u8; 32];

/// Invokes another contract without transferring value.
///
/// This is the legacy cross-contract call mechanism. The called contract
/// executes with the same caller context (the original transaction sender).
///
/// # Arguments
///
/// * `contract_id` - The 32-byte address of the contract to call
/// * `calldata` - Encoded function selector and arguments
///
/// # Returns
///
/// * `Ok(Vec<u8>)` - The return data from the called contract
/// * `Err(Error)` - If the call fails or the contract doesn't exist
///
/// # Gas
///
/// The called contract shares the remaining gas budget of the current execution.
///
/// # Example
///
/// ```ignore
/// use truthlinked_sdk::call;
///
/// let token_contract = [0x12; 32];
/// let calldata = encode_balance_of(account);
/// let balance_bytes = call::call_contract(&token_contract, &calldata)?;
/// let balance = decode_u128(&balance_bytes);
/// ```
pub fn call_contract(contract_id: &AccountId, calldata: &[u8]) -> Result<Vec<u8>> {
    let mut out = vec![0u8; env::MAX_RETURN_DATA_SIZE];
    let len = env::call_contract_legacy(contract_id, calldata, &mut out)?;
    out.truncate(len.min(out.len()));
    Ok(out)
}

/// Invokes another contract while transferring native tokens.
///
/// This is the v2 cross-contract call mechanism that supports value transfer.
/// The specified amount of native tokens is transferred from the calling contract
/// to the called contract before execution.
///
/// # Arguments
///
/// * `contract_id` - The 32-byte address of the contract to call
/// * `calldata` - Encoded function selector and arguments
/// * `value` - Amount of native tokens to transfer (in base units)
///
/// # Returns
///
/// * `Ok(Vec<u8>)` - The return data from the called contract
/// * `Err(Error)` - If the call fails, contract doesn't exist, or insufficient balance
///
/// # Errors
///
/// This function will fail if:
/// - The calling contract has insufficient balance
/// - The called contract doesn't exist
/// - The called contract reverts
/// - Gas is exhausted
///
/// # Example
///
/// ```ignore
/// use truthlinked_sdk::call;
///
/// // Call a payable function with 1000 tokens
/// let result = call::call_contract_with_value(
///     &contract_id,
///     &calldata,
///     1000,
/// )?;
/// ```
pub fn call_contract_with_value(
    contract_id: &AccountId,
    calldata: &[u8],
    value: u128,
) -> Result<Vec<u8>> {
    let mut out = vec![0u8; env::MAX_RETURN_DATA_SIZE];
    let len = env::call_contract_v2_with_value(contract_id, calldata, value, &mut out)?;
    out.truncate(len.min(out.len()));
    Ok(out)
}
