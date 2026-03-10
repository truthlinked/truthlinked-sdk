//! Execution context accessors for contract runtime information.
//!
//! This module provides functions to query the current execution context,
//! including caller information, block data, and call parameters.
//!
//! # Available Context
//!
//! - **Caller**: Who invoked this contract
//! - **Owner**: Who deployed/owns this contract
//! - **Contract ID**: This contract's address
//! - **Block data**: Current height and timestamp
//! - **Call value**: Native tokens sent with the call
//! - **Calldata**: Function selector and arguments
//!
//! # Example
//!
//! ```ignore
//! use truthlinked_sdk::context;
//!
//! fn handle_execute() -> Result<()> {
//!     let caller = context::caller()?;
//!     let owner = context::owner()?;
//!     
//!     // Only owner can call this function
//!     if caller != owner {
//!         return Err(Error::new(ERR_UNAUTHORIZED));
//!     }
//!     
//!     let height = context::block_height()?;
//!     let value = context::call_value()?;
//!     
//!     // Process transaction...
//!     Ok(())
//! }
//! ```

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use crate::env;
use crate::error::Result;

/// Type alias for 32-byte account identifiers.
pub type AccountId = [u8; 32];

/// Returns the account ID of the transaction sender.
///
/// This is the original caller who initiated the transaction, not necessarily
/// the immediate caller in case of cross-contract calls.
///
/// # Returns
///
/// The 32-byte account ID of the caller.
///
/// # Example
///
/// ```ignore
/// let caller = context::caller()?;
/// if caller == owner {
///     // Caller is the owner
/// }
/// ```
pub fn caller() -> Result<AccountId> {
    env::get_caller_32()
}

/// Returns the account ID of the contract owner.
///
/// The owner is set at deployment time and can be transferred via ownership
/// transfer mechanisms. Typically used for access control.
///
/// # Returns
///
/// The 32-byte account ID of the contract owner.
///
/// # Example
///
/// ```ignore
/// let owner = context::owner()?;
/// let caller = context::caller()?;
/// 
/// if caller != owner {
///     return Err(Error::new(ERR_NOT_OWNER));
/// }
/// ```
pub fn owner() -> Result<AccountId> {
    env::get_owner_32()
}

/// Returns this contract's account ID.
///
/// Useful for:
/// - Self-referential operations
/// - Checking if caller is this contract (for callbacks)
/// - Logging contract address
///
/// # Returns
///
/// The 32-byte account ID of the current contract.
///
/// # Example
///
/// ```ignore
/// let my_address = context::contract_id()?;
/// log::emit_event(&ContractDeployed { address: my_address })?;
/// ```
pub fn contract_id() -> Result<AccountId> {
    env::get_contract_id_32()
}

/// Returns the current block height.
///
/// Block height increments with each finalized batch (typically every 200ms).
/// Useful for time-based logic, expiration checks, and replay protection.
///
/// # Returns
///
/// The current block height as a `u64`.
///
/// # Example
///
/// ```ignore
/// let current_height = context::block_height()?;
/// let expiration = storage.read_expiration()?;
/// 
/// if current_height > expiration {
///     return Err(Error::new(ERR_EXPIRED));
/// }
/// ```
pub fn block_height() -> Result<u64> {
    env::get_height_u64()
}

/// Returns the current block timestamp (Unix seconds).
///
/// This is the timestamp of the current batch being executed.
/// Useful for time-based logic, but note that it updates in ~200ms intervals.
///
/// # Returns
///
/// Unix timestamp in seconds as a `u64`.
///
/// # Example
///
/// ```ignore
/// let now = context::block_timestamp()?;
/// let deadline = storage.read_deadline()?;
/// 
/// if now > deadline {
///     return Err(Error::new(ERR_DEADLINE_PASSED));
/// }
/// ```
pub fn block_timestamp() -> Result<u64> {
    env::get_timestamp_u64()
}

/// Returns the amount of native tokens sent with this call.
///
/// Only non-zero when called via `call_contract_with_value()`.
/// The value is transferred before contract execution begins.
///
/// # Returns
///
/// The amount of native tokens (in base units) sent with the call.
///
/// # Example
///
/// ```ignore
/// let value = context::call_value()?;
/// 
/// if value < MINIMUM_DEPOSIT {
///     return Err(Error::new(ERR_INSUFFICIENT_VALUE));
/// }
/// 
/// // Credit the caller's balance
/// balances.insert(&caller, &value)?;
/// ```
pub fn call_value() -> Result<u128> {
    env::get_value_u128()
}

/// Returns the raw calldata (function selector + arguments).
///
/// Calldata format:
/// - Bytes 0-3: Function selector (4 bytes)
/// - Bytes 4+: Encoded arguments
///
/// # Returns
///
/// A `Vec<u8>` containing the complete calldata.
///
/// # Example
///
/// ```ignore
/// let calldata = context::calldata()?;
/// let selector = abi::selector(&calldata)?;
/// 
/// if selector == abi::selector_of("transfer") {
///     let to = abi::read_account(&calldata, 4)?;
///     let amount = abi::read_u64(&calldata, 36)?;
///     // Handle transfer...
/// }
/// ```
pub fn calldata() -> Result<Vec<u8>> {
    let mut bytes = vec![0u8; env::MAX_CALLDATA_SIZE];
    let len = env::read_calldata(&mut bytes)?;
    bytes.truncate(len);
    Ok(bytes)
}

/// Sets the return data for this contract call.
///
/// The return data is sent back to the caller and can be decoded by the client.
/// Typically used to return function results (balances, status codes, etc.).
///
/// # Arguments
///
/// * `data` - The bytes to return to the caller
///
/// # Example
///
/// ```ignore
/// // Return a u64 balance
/// let balance = 1000u64;
/// context::set_return_data(&balance.to_le_bytes())?;
///
/// // Return multiple values (encode them first)
/// let mut encoder = Encoder::new();
/// encoder.push_u64(balance);
/// encoder.push_bool(is_active);
/// context::set_return_data(encoder.as_slice())?;
/// ```
pub fn set_return_data(data: &[u8]) -> Result<()> {
    env::set_return_data_bytes(data)
}
