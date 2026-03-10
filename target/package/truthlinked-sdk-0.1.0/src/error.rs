//! Error handling types and utilities for contract execution.
//!
//! This module provides a lightweight error type based on integer error codes,
//! suitable for `no_std` environments. Contracts return error codes to the runtime,
//! which can be interpreted by clients.
//!
//! # Error Codes
//!
//! Error codes are `i32` values where:
//! - `0` = Success (OK)
//! - `1-99` = SDK/runtime errors
//! - `100+` = Contract-specific errors
//!
//! # Custom Errors
//!
//! Use the `#[error_code]` macro to define custom error enums:
//!
//! ```ignore
//! #[error_code(base = 1000)]
//! enum TokenError {
//!     InsufficientBalance,  // 1000
//!     Unauthorized,         // 1001
//!     InvalidAmount,        // 1002
//! }
//! ```
//!
//! # Guards
//!
//! Use the `#[require]` macro for precondition checks:
//!
//! ```ignore
//! #[require(amount > 0, TokenError::InvalidAmount)]
//! fn transfer(amount: u64) -> Result<()> {
//!     // amount is guaranteed to be > 0
//! }
//! ```

#![allow(clippy::module_name_repetitions)]

/// Lightweight error type wrapping an `i32` error code.
///
/// This type is designed for `no_std` environments and minimal overhead.
/// Contracts return error codes to the runtime, which propagates them to clients.
///
/// # Error Code Ranges
///
/// - `0`: Success (OK)
/// - `1-9`: Core SDK errors
/// - `10-19`: ABI/calldata errors
/// - `20-29`: Codec/serialization errors
/// - `30-39`: Storage errors
/// - `40-49`: Validation errors (require)
/// - `100+`: Contract-specific errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(i32);

/// Error code for failed `#[require]` guards without custom error.
pub const ERR_REQUIRE: i32 = 40;

impl Error {
    /// Success code (no error).
    pub const OK: i32 = 0;

    /// Creates a new error with the specified code.
    ///
    /// # Arguments
    ///
    /// * `code` - The error code (non-zero indicates failure)
    ///
    /// # Example
    ///
    /// ```ignore
    /// const ERR_UNAUTHORIZED: i32 = 100;
    /// return Err(Error::new(ERR_UNAUTHORIZED));
    /// ```
    pub const fn new(code: i32) -> Self {
        Self(code)
    }

    /// Returns the error code.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let err = Error::new(100);
    /// assert_eq!(err.code(), 100);
    /// ```
    pub const fn code(self) -> i32 {
        self.0
    }

    /// Checks if this error represents success (code == 0).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let ok = Error::new(0);
    /// assert!(ok.is_ok());
    ///
    /// let err = Error::new(100);
    /// assert!(!err.is_ok());
    /// ```
    pub const fn is_ok(self) -> bool {
        self.0 == Self::OK
    }
}

impl From<i32> for Error {
    fn from(value: i32) -> Self {
        Self::new(value)
    }
}

/// Result type alias using `Error` as the error type.
///
/// This is the standard result type for all SDK and contract functions.
///
/// # Example
///
/// ```ignore
/// fn transfer(to: AccountId, amount: u64) -> Result<()> {
///     if amount == 0 {
///         return Err(Error::new(ERR_INVALID_AMOUNT));
///     }
///     // ... perform transfer
///     Ok(())
/// }
/// ```
pub type Result<T> = core::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[crate::error_code(base = 7000)]
    #[derive(Clone, Copy)]
    enum ContractError {
        NotOwner,
        Overflow,
    }

    #[crate::require(2 + 2 == 4)]
    fn guarded_ok() -> Result<u64> {
        Ok(1)
    }

    #[crate::require(false, ContractError::NotOwner)]
    fn guarded_fail() -> Result<u64> {
        Ok(1)
    }

    #[test]
    fn error_code_attribute_generates_codes() {
        assert_eq!(ContractError::NotOwner.code(), 7000);
        assert_eq!(ContractError::Overflow.code(), 7001);
        let err: Error = ContractError::NotOwner.into();
        assert_eq!(err.code(), 7000);
    }

    #[test]
    fn require_attribute_guards_function() {
        assert_eq!(guarded_ok().unwrap(), 1);
        let err = guarded_fail().unwrap_err();
        assert_eq!(err.code(), 7000);
    }
}
