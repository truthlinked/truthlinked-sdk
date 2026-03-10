//! TruthLinked Smart Contract SDK
//!
//! This SDK provides a complete toolkit for building smart contracts on the TruthLinked blockchain.
//!
//! # Features
//!
//! - **Storage**: Type-safe collections (maps, vectors, blobs) with dual API pattern
//! - **Codec**: Efficient encoding/decoding for 32-byte and variable-length data
//! - **Events**: Structured logging with indexed topics
//! - **Manifests**: Declare storage access patterns for parallel execution
//! - **Oracle**: HTTP requests for external data
//! - **Testing**: In-memory storage harness for unit tests
//!
//! # Quick Start
//!
//! ```ignore
//! use truthlinked_sdk::prelude::*;
//!
//! #[derive(Manifest)]
//! struct Counter {
//!     #[manifest(read, write)]
//!     value: Slot,
//! }
//!
//! fn increment() -> Result<()> {
//!     let counter = Counter {
//!         value: Slot::from_label("count"),
//!     };
//!     let current = counter.value.read_u64()?;
//!     counter.value.write_u64(current + 1)?;
//!     Ok(())
//! }
//!
//! contract_entry!(increment);
//! ```
//!
//! # Module Overview
//!
//! - [`abi`] - Function selectors and calldata parsing
//! - [`backend`] - Storage backend abstraction
//! - [`call`] - Cross-contract calls
//! - [`codec`] - Encoding/decoding traits and builders
//! - [`collections`] - StorageMap, StorageVec, StorageBlob
//! - [`context`] - Execution context (caller, height, timestamp, etc.)
//! - [`env`] - Low-level WASM host bindings
//! - [`error`] - Error types and Result alias
//! - [`hashing`] - Cryptographic utilities
//! - [`log`] - Event system
//! - [`manifest`] - Storage access declarations
//! - [`oracle`] - HTTP oracle interface
//! - [`storage`] - Direct storage slot access
//! - [`testing`] - Unit test utilities

#![no_std]

extern crate alloc;
extern crate self as truthlinked_sdk;

pub mod abi;
pub mod backend;
pub mod call;
pub mod codec;
pub mod collections;
pub mod context;
pub mod env;
pub mod error;
pub mod hashing;
pub mod log;
pub mod manifest;
pub mod oracle;
pub mod storage;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use error::{Error, Result};
pub use truthlinked_sdk_macros::{error_code, require, BytesCodec, Codec32, Event, Manifest};

/// Prelude module with commonly used imports.
///
/// Import everything with `use truthlinked_sdk::prelude::*;`
pub mod prelude {
    pub use crate::abi;
    pub use crate::backend::{HostStorage, MemoryStorage, StorageBackend};
    pub use crate::call;
    pub use crate::codec::{BytesCodec, Codec32, Decoder, Encoder};
    pub use crate::collections::{Namespace, StorageBlob, StorageMap, StorageVec};
    pub use crate::context;
    pub use crate::env;
    pub use crate::hashing;
    pub use crate::log;
    pub use crate::manifest::{ContractManifest, Manifest, ManifestBuilder, StorageKeySpec};
    pub use crate::oracle;
    pub use crate::storage::{self, Slot};
    #[cfg(any(test, feature = "testing"))]
    pub use crate::testing::{StorageHarness, TestBlob, TestMap, TestVec};
    pub use crate::{
        contract_entry, error_code, require, slot, BytesCodec, Codec32, Error, Event, Manifest,
        Result,
    };
}

/// Defines the contract entry point.
///
/// # Example
///
/// ```ignore
/// fn my_handler() -> Result<()> {
///     // Contract logic
///     Ok(())
/// }
///
/// contract_entry!(my_handler);
/// ```
#[macro_export]
macro_rules! contract_entry {
    ($handler:path) => {
        #[no_mangle]
        pub extern "C" fn execute() -> i32 {
            match $handler() {
                Ok(()) => 0,
                Err(err) => err.code(),
            }
        }
    };
}
