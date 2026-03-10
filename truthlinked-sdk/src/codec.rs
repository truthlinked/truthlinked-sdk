//! Serialization and deserialization traits and utilities.
//!
//! This module provides two encoding strategies for contract data:
//!
//! - **`Codec32`**: Fixed 32-byte encoding for storage slots
//! - **`BytesCodec`**: Variable-length encoding for complex types
//!
//! Both traits support `#[derive]` macros for automatic implementation.
//!
//! # Codec32 - Fixed-Size Storage
//!
//! Used for values stored in 32-byte storage slots. All primitive integers,
//! booleans, and 32-byte arrays implement this trait.
//!
//! ```ignore
//! use truthlinked_sdk::codec::Codec32;
//!
//! let value = 42u64;
//! let encoded = value.encode_32(); // [42, 0, 0, ..., 0] (32 bytes)
//! let decoded = u64::decode_32(&encoded)?; // 42
//! ```
//!
//! # BytesCodec - Variable-Length Encoding
//!
//! Used for complex types, strings, and variable-length data. Supports
//! automatic derivation for structs and enums.
//!
//! ```ignore
//! #[derive(BytesCodec)]
//! struct Transfer {
//!     to: [u8; 32],
//!     amount: u64,
//! }
//!
//! let transfer = Transfer { to: [0; 32], amount: 100 };
//! let bytes = transfer.encode_bytes();
//! let decoded = Transfer::decode_bytes(&bytes)?;
//! ```
//!
//! # Encoder/Decoder - Builder Pattern
//!
//! For manual encoding of complex types:
//!
//! ```ignore
//! use truthlinked_sdk::codec::{Encoder, Decoder};
//!
//! // Encoding
//! let mut enc = Encoder::new();
//! enc.push_u64(42);
//! enc.push_string("hello");
//! enc.push_bool(true);
//! let bytes = enc.into_vec();
//!
//! // Decoding
//! let mut dec = Decoder::new(&bytes);
//! let num = dec.read_u64()?;
//! let text = dec.read_string()?;
//! let flag = dec.read_bool()?;
//! dec.finish()?; // Ensure all bytes consumed
//! ```

extern crate alloc;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::error::{Error, Result};

/// Error code for general codec failures.
pub const ERR_CODEC: i32 = 20;

/// Error code for unexpected end of input during decoding.
pub const ERR_CODEC_EOF: i32 = 21;

/// Error code for invalid UTF-8 sequences in string decoding.
pub const ERR_CODEC_UTF8: i32 = 22;

/// Trait for types that can be encoded/decoded to/from 32-byte slots.
///
/// This trait is used for storage operations where all values must fit
/// in fixed 32-byte slots. Primitive integers are stored in little-endian
/// format with zero-padding.
///
/// # Derivable
///
/// Use `#[derive(Codec32)]` for custom types (requires `BytesCodec`):
///
/// ```ignore
/// #[derive(BytesCodec, Codec32)]
/// struct Balance {
///     amount: u64,
///     locked: u64,
/// }
/// ```
///
/// # Example
///
/// ```ignore
/// let value = 1000u64;
/// let slot = value.encode_32();
/// let decoded = u64::decode_32(&slot)?;
/// assert_eq!(decoded, 1000);
/// ```
pub trait Codec32: Sized {
    /// Encodes the value into a 32-byte array.
    fn encode_32(&self) -> [u8; 32];

    /// Decodes a value from a 32-byte array.
    fn decode_32(bytes: &[u8; 32]) -> Result<Self>;
}

/// Trait for types that can be encoded/decoded to/from variable-length bytes.
///
/// This trait is used for complex types, strings, and data that doesn't fit
/// in 32 bytes. All encoding uses little-endian format for integers.
///
/// # Derivable
///
/// Use `#[derive(BytesCodec)]` for automatic implementation:
///
/// ```ignore
/// #[derive(BytesCodec)]
/// struct User {
///     name: String,
///     age: u32,
///     active: bool,
/// }
/// ```
///
/// # Example
///
/// ```ignore
/// let data = vec![1, 2, 3, 4];
/// let bytes = data.encode_bytes();
/// let decoded = Vec::<u8>::decode_bytes(&bytes)?;
/// assert_eq!(decoded, data);
/// ```
pub trait BytesCodec: Sized {
    /// Encodes the value into a byte vector.
    fn encode_bytes(&self) -> Vec<u8>;

    /// Decodes a value from a byte slice.
    fn decode_bytes(bytes: &[u8]) -> Result<Self>;
}

/// Builder for encoding multiple values into a byte stream.
///
/// `Encoder` provides a convenient API for serializing complex types
/// by chaining method calls. All integers are encoded in little-endian format.
///
/// # Example
///
/// ```ignore
/// let mut enc = Encoder::with_capacity(64);
/// enc.push_u64(42);
/// enc.push_string("hello");
/// enc.push_bool(true);
/// enc.push_bytes(&[1, 2, 3]);
///
/// let bytes = enc.into_vec();
/// ```
#[derive(Clone, Debug, Default)]
pub struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    /// Creates a new empty encoder.
    pub fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    /// Creates a new encoder with pre-allocated capacity.
    ///
    /// Use this when you know the approximate size to avoid reallocations.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
        }
    }

    /// Appends a `u8` value.
    pub fn push_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    /// Appends a `u16` value (little-endian).
    pub fn push_u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    /// Appends a `u32` value (little-endian).
    pub fn push_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    /// Appends a `u64` value (little-endian).
    pub fn push_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    /// Appends a `u128` value (little-endian).
    pub fn push_u128(&mut self, value: u128) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    /// Appends a boolean value (0 or 1).
    pub fn push_bool(&mut self, value: bool) {
        self.push_u8(if value { 1 } else { 0 });
    }

    /// Appends raw bytes without length prefix.
    pub fn push_raw(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    /// Appends a byte slice with a u32 length prefix.
    ///
    /// Format: `[length: u32][data: bytes]`
    pub fn push_bytes(&mut self, bytes: &[u8]) {
        self.push_u32(bytes.len() as u32);
        self.push_raw(bytes);
    }

    /// Appends a string with a u32 length prefix.
    ///
    /// The string is encoded as UTF-8 bytes.
    pub fn push_string(&mut self, value: &str) {
        self.push_bytes(value.as_bytes());
    }

    /// Appends a value that implements `BytesCodec` with a length prefix.
    pub fn push_codec<T: BytesCodec>(&mut self, value: &T) {
        self.push_bytes(&value.encode_bytes());
    }

    /// Returns the encoded bytes as a slice.
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    /// Consumes the encoder and returns the encoded bytes.
    pub fn into_vec(self) -> Vec<u8> {
        self.bytes
    }
}

/// Parser for decoding values from a byte stream.
///
/// `Decoder` provides a cursor-based API for deserializing values encoded
/// by `Encoder`. All integers are decoded from little-endian format.
///
/// # Example
///
/// ```ignore
/// let mut dec = Decoder::new(&bytes);
/// let num = dec.read_u64()?;
/// let text = dec.read_string()?;
/// let flag = dec.read_bool()?;
/// dec.finish()?; // Verify all bytes consumed
/// ```
pub struct Decoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Decoder<'a> {
    /// Creates a new decoder from a byte slice.
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    /// Returns the number of unread bytes remaining.
    pub fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.cursor)
    }

    /// Reads exactly `len` bytes, advancing the cursor.
    ///
    /// Returns an error if not enough bytes are available.
    fn read_exact(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .cursor
            .checked_add(len)
            .ok_or_else(|| Error::new(ERR_CODEC_EOF))?;
        if end > self.bytes.len() {
            return Err(Error::new(ERR_CODEC_EOF));
        }
        let out = &self.bytes[self.cursor..end];
        self.cursor = end;
        Ok(out)
    }

    /// Reads a fixed-size byte array.
    pub fn read_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let bytes = self.read_exact(N)?;
        let mut out = [0u8; N];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    /// Reads a `u8` value.
    pub fn read_u8(&mut self) -> Result<u8> {
        Ok(self.read_exact(1)?[0])
    }

    /// Reads a `u16` value (little-endian).
    pub fn read_u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.read_array()?))
    }

    /// Reads a `u32` value (little-endian).
    pub fn read_u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read_array()?))
    }

    /// Reads a `u64` value (little-endian).
    pub fn read_u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read_array()?))
    }

    /// Reads a `u128` value (little-endian).
    pub fn read_u128(&mut self) -> Result<u128> {
        Ok(u128::from_le_bytes(self.read_array()?))
    }

    /// Reads a boolean value (0 = false, 1 = true).
    ///
    /// Returns an error if the byte is neither 0 nor 1.
    pub fn read_bool(&mut self) -> Result<bool> {
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::new(ERR_CODEC)),
        }
    }

    /// Reads exactly `len` raw bytes without a length prefix.
    pub fn read_raw(&mut self, len: usize) -> Result<&'a [u8]> {
        self.read_exact(len)
    }

    /// Reads a byte slice with a u32 length prefix.
    ///
    /// Format: `[length: u32][data: bytes]`
    pub fn read_bytes(&mut self) -> Result<&'a [u8]> {
        let len = self.read_u32()? as usize;
        self.read_exact(len)
    }

    /// Reads a UTF-8 string with a u32 length prefix.
    ///
    /// Returns an error if the bytes are not valid UTF-8.
    pub fn read_string(&mut self) -> Result<String> {
        let bytes = self.read_bytes()?;
        let value = core::str::from_utf8(bytes).map_err(|_| Error::new(ERR_CODEC_UTF8))?;
        Ok(String::from(value))
    }

    /// Reads a value that implements `BytesCodec` with a length prefix.
    pub fn read_codec<T: BytesCodec>(&mut self) -> Result<T> {
        let bytes = self.read_bytes()?;
        T::decode_bytes(bytes)
    }

    /// Verifies that all bytes have been consumed.
    ///
    /// Returns an error if there are unread bytes remaining.
    /// Use this at the end of decoding to ensure data integrity.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut dec = Decoder::new(&bytes);
    /// let value = dec.read_u64()?;
    /// dec.finish()?; // Ensure no trailing bytes
    /// ```
    pub fn finish(self) -> Result<()> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(Error::new(ERR_CODEC))
        }
    }
}

// ============================================================================
// Codec32 Implementations
// ============================================================================

impl Codec32 for [u8; 32] {
    fn encode_32(&self) -> [u8; 32] {
        *self
    }

    fn decode_32(bytes: &[u8; 32]) -> Result<Self> {
        Ok(*bytes)
    }
}

impl Codec32 for bool {
    fn encode_32(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[0] = if *self { 1 } else { 0 };
        out
    }

    fn decode_32(bytes: &[u8; 32]) -> Result<Self> {
        match bytes[0] {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::new(ERR_CODEC)),
        }
    }
}

/// Implements `Codec32` for integer types using little-endian encoding.
macro_rules! impl_codec32_int {
    ($ty:ty, $width:expr) => {
        impl Codec32 for $ty {
            fn encode_32(&self) -> [u8; 32] {
                let mut out = [0u8; 32];
                out[..$width].copy_from_slice(&self.to_le_bytes());
                out
            }

            fn decode_32(bytes: &[u8; 32]) -> Result<Self> {
                let mut raw = [0u8; $width];
                raw.copy_from_slice(&bytes[..$width]);
                Ok(<$ty>::from_le_bytes(raw))
            }
        }
    };
}

impl_codec32_int!(u8, 1);
impl_codec32_int!(u16, 2);
impl_codec32_int!(u32, 4);
impl_codec32_int!(u64, 8);
impl_codec32_int!(u128, 16);
impl_codec32_int!(i8, 1);
impl_codec32_int!(i16, 2);
impl_codec32_int!(i32, 4);
impl_codec32_int!(i64, 8);
impl_codec32_int!(i128, 16);

// ============================================================================
// BytesCodec Implementations
// ============================================================================

impl BytesCodec for bool {
    fn encode_bytes(&self) -> Vec<u8> {
        vec![if *self { 1 } else { 0 }]
    }

    fn decode_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 1 {
            return Err(Error::new(ERR_CODEC));
        }
        match bytes[0] {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::new(ERR_CODEC)),
        }
    }
}

/// Implements `BytesCodec` for integer types using little-endian encoding.
macro_rules! impl_bytescodec_int {
    ($ty:ty, $width:expr) => {
        impl BytesCodec for $ty {
            fn encode_bytes(&self) -> Vec<u8> {
                self.to_le_bytes().to_vec()
            }

            fn decode_bytes(bytes: &[u8]) -> Result<Self> {
                if bytes.len() != $width {
                    return Err(Error::new(ERR_CODEC));
                }
                let mut raw = [0u8; $width];
                raw.copy_from_slice(bytes);
                Ok(<$ty>::from_le_bytes(raw))
            }
        }
    };
}

impl_bytescodec_int!(u8, 1);
impl_bytescodec_int!(u16, 2);
impl_bytescodec_int!(u32, 4);
impl_bytescodec_int!(u64, 8);
impl_bytescodec_int!(u128, 16);
impl_bytescodec_int!(i8, 1);
impl_bytescodec_int!(i16, 2);
impl_bytescodec_int!(i32, 4);
impl_bytescodec_int!(i64, 8);
impl_bytescodec_int!(i128, 16);

impl BytesCodec for Vec<u8> {
    fn encode_bytes(&self) -> Vec<u8> {
        self.clone()
    }

    fn decode_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(bytes.to_vec())
    }
}

impl<const N: usize> BytesCodec for [u8; N] {
    fn encode_bytes(&self) -> Vec<u8> {
        self.to_vec()
    }

    fn decode_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != N {
            return Err(Error::new(ERR_CODEC));
        }
        let mut out = [0u8; N];
        out.copy_from_slice(bytes);
        Ok(out)
    }
}

impl BytesCodec for String {
    fn encode_bytes(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }

    fn decode_bytes(bytes: &[u8]) -> Result<Self> {
        let s = core::str::from_utf8(bytes).map_err(|_| Error::new(ERR_CODEC_UTF8))?;
        Ok(String::from(s))
    }
}

impl<T: BytesCodec> BytesCodec for Option<T> {
    fn encode_bytes(&self) -> Vec<u8> {
        match self {
            Some(value) => {
                let mut out = Vec::with_capacity(1);
                out.push(1);
                out.extend_from_slice(&value.encode_bytes());
                out
            }
            None => vec![0],
        }
    }

    fn decode_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            return Err(Error::new(ERR_CODEC));
        }
        match bytes[0] {
            0 => {
                if bytes.len() != 1 {
                    return Err(Error::new(ERR_CODEC));
                }
                Ok(None)
            }
            1 => Ok(Some(T::decode_bytes(&bytes[1..])?)),
            _ => Err(Error::new(ERR_CODEC)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;

    #[derive(crate::BytesCodec, crate::Codec32, Debug, PartialEq, Eq)]
    struct SmallRecord {
        id: u16,
        enabled: bool,
    }

    #[derive(crate::BytesCodec, Debug, PartialEq, Eq)]
    enum Action {
        Inc(u64),
        Set { value: u64 },
        Reset,
    }

    #[test]
    fn codec32_roundtrip_u64() {
        let raw = 55u64.encode_32();
        let decoded = u64::decode_32(&raw).unwrap();
        assert_eq!(decoded, 55);
    }

    #[test]
    fn bytescodec_roundtrip_string() {
        let s = String::from("hello");
        let bytes = s.encode_bytes();
        let decoded = String::decode_bytes(&bytes).unwrap();
        assert_eq!(decoded, "hello");
    }

    #[test]
    fn encoder_decoder_roundtrip() {
        let mut enc = Encoder::new();
        enc.push_u64(99);
        enc.push_bool(true);
        enc.push_string("abc");

        let bytes = enc.into_vec();
        let mut dec = Decoder::new(&bytes);
        assert_eq!(dec.read_u64().unwrap(), 99);
        assert!(dec.read_bool().unwrap());
        assert_eq!(dec.read_string().unwrap(), "abc");
        dec.finish().unwrap();
    }

    #[test]
    fn derive_bytescodec_roundtrip_struct_and_enum() {
        let rec = SmallRecord {
            id: 7,
            enabled: true,
        };
        let rec_bytes = rec.encode_bytes();
        let rec_decoded = SmallRecord::decode_bytes(&rec_bytes).unwrap();
        assert_eq!(rec, rec_decoded);

        let action = Action::Set { value: 44 };
        let action_bytes = action.encode_bytes();
        let action_decoded = Action::decode_bytes(&action_bytes).unwrap();
        assert_eq!(action, action_decoded);
    }

    #[test]
    fn derive_codec32_roundtrip_small_record() {
        let rec = SmallRecord {
            id: 42,
            enabled: false,
        };
        let raw = rec.encode_32();
        let decoded = SmallRecord::decode_32(&raw).unwrap();
        assert_eq!(decoded, rec);
    }
}
