//! Contract manifest generation for parallel execution optimization.
//!
//! Manifests declare which storage slots a contract will read/write, enabling
//! the TruthLinked runtime to execute non-conflicting transactions in parallel.
//!
//! # What is a Manifest?
//!
//! A manifest is metadata attached to a contract that declares:
//! - **Read slots**: Storage slots the contract reads from
//! - **Write slots**: Storage slots the contract writes to
//! - **Commutative keys**: Slots that support commutative operations (Add, Max, etc.)
//! - **Key specs**: Dynamic slot extraction rules from calldata
//!
//! # Why Manifests?
//!
//! Manifests enable **parallel execution**:
//! - Transactions with disjoint read/write sets execute in parallel
//! - Commutative operations on the same slot can be batched
//! - Runtime can detect conflicts before execution
//!
//! # Usage
//!
//! ## Automatic (Recommended)
//!
//! Use the `#[derive(Manifest)]` macro:
//!
//! ```ignore
//! #[derive(Manifest)]
//! #[manifest(
//!     read_derived(namespace = "balances", key = "alice"),
//!     write_derived(namespace = "balances", key = "bob"),
//!     key_spec(offset = 4, len = 32)  // Extract account from calldata
//! )]
//! struct TransferManifest;
//! ```
//!
//! ## Manual (Advanced)
//!
//! Use `ManifestBuilder` for programmatic construction:
//!
//! ```ignore
//! use truthlinked_sdk::manifest::ManifestBuilder;
//!
//! let manifest = ManifestBuilder::new()
//!     .read_slot([0x01; 32])
//!     .write_slot([0x02; 32])
//!     .commutative_slot([0x03; 32])
//!     .storage_key_spec(4, 32)
//!     .build();
//! ```
//!
//! # Manifest Hash
//!
//! The manifest hash is computed as:
//! ```text
//! BLAKE3(bytecode || sorted(reads) || sorted(writes) || sorted(commutative))
//! ```
//!
//! This hash is stored on-chain and verified at deployment/upgrade.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use blake3::Hasher;

use crate::codec::Codec32;
use crate::collections::{StorageBlob, StorageMap, StorageVec};

/// Trait for types that can generate a contract manifest.
///
/// Typically implemented via `#[derive(Manifest)]` macro.
///
/// # Example
///
/// ```ignore
/// #[derive(Manifest)]
/// #[manifest(read_slot = "0x01...")]
/// struct MyManifest;
///
/// let manifest = MyManifest::manifest();
/// ```
pub trait Manifest {
    /// Returns the contract manifest.
    fn manifest() -> ContractManifest;
}

/// Helper function to get a manifest from a type implementing `Manifest`.
pub fn manifest_of<M: Manifest>() -> ContractManifest {
    M::manifest()
}

/// Specification for extracting storage keys from calldata.
///
/// Key specs enable dynamic slot extraction at runtime. For example,
/// extracting an account ID from calldata to compute a balance slot.
///
/// # Example
///
/// ```ignore
/// // Extract 32-byte account ID starting at byte 4 of calldata
/// let spec = StorageKeySpec { offset: 4, len: 32 };
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StorageKeySpec {
    /// Byte offset in calldata where the key starts.
    pub offset: usize,
    /// Length of the key in bytes.
    pub len: usize,
}

/// A contract manifest declaring storage access patterns.
///
/// Manifests enable parallel execution by declaring which storage slots
/// a contract will access. The runtime uses this information to:
/// - Execute non-conflicting transactions in parallel
/// - Batch commutative operations
/// - Detect conflicts before execution
///
/// # Fields
///
/// - `declared_reads`: Slots the contract reads from
/// - `declared_writes`: Slots the contract writes to
/// - `commutative_keys`: Slots supporting commutative operations
/// - `storage_key_specs`: Rules for extracting dynamic keys from calldata
///
/// # Example
///
/// ```ignore
/// let mut manifest = ContractManifest::new();
/// manifest.add_read_slot([0x01; 32]);
/// manifest.add_write_slot([0x02; 32]);
/// manifest.normalize(); // Sort and deduplicate
///
/// let hash = manifest.manifest_hash(&bytecode);
/// let json = manifest.to_json_pretty();
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ContractManifest {
    /// Storage slots the contract reads from.
    pub declared_reads: Vec<[u8; 32]>,
    /// Storage slots the contract writes to.
    pub declared_writes: Vec<[u8; 32]>,
    /// Storage slots that support commutative operations (Add, Max, Or, Append).
    pub commutative_keys: Vec<[u8; 32]>,
    /// Rules for extracting dynamic storage keys from calldata.
    pub storage_key_specs: Vec<StorageKeySpec>,
}

impl ContractManifest {
    /// Creates a new empty manifest.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a read slot to the manifest.
    ///
    /// Duplicates are automatically removed during normalization.
    pub fn add_read_slot(&mut self, slot: [u8; 32]) -> &mut Self {
        push_unique_slot(&mut self.declared_reads, slot);
        self
    }

    /// Adds a write slot to the manifest.
    ///
    /// Duplicates are automatically removed during normalization.
    pub fn add_write_slot(&mut self, slot: [u8; 32]) -> &mut Self {
        push_unique_slot(&mut self.declared_writes, slot);
        self
    }

    /// Adds a commutative key to the manifest.
    ///
    /// Commutative keys support operations like Add, Max, Or, Append
    /// that can be executed in any order.
    pub fn add_commutative_key(&mut self, slot: [u8; 32]) -> &mut Self {
        push_unique_slot(&mut self.commutative_keys, slot);
        self
    }

    /// Adds a storage key specification for dynamic slot extraction.
    ///
    /// # Arguments
    ///
    /// * `spec` - Specification defining offset and length in calldata
    pub fn add_storage_key_spec(&mut self, spec: StorageKeySpec) -> &mut Self {
        if !self.storage_key_specs.contains(&spec) {
            self.storage_key_specs.push(spec);
        }
        self
    }

    /// Sorts and deduplicates all manifest entries.
    ///
    /// This ensures canonical representation for hashing and comparison.
    pub fn normalize(&mut self) {
        self.declared_reads.sort();
        self.declared_reads.dedup();
        self.declared_writes.sort();
        self.declared_writes.dedup();
        self.commutative_keys.sort();
        self.commutative_keys.dedup();
        self.storage_key_specs.sort();
        self.storage_key_specs.dedup();
    }

    /// Returns a normalized copy of this manifest.
    ///
    /// Equivalent to cloning and calling `normalize()`.
    pub fn normalized(mut self) -> Self {
        self.normalize();
        self
    }

    /// Computes the manifest hash for on-chain storage.
    ///
    /// The hash is computed as:
    /// ```text
    /// BLAKE3(bytecode || sorted(reads) || sorted(writes) || sorted(commutative))
    /// ```
    ///
    /// This hash is stored on-chain and verified at deployment/upgrade.
    ///
    /// # Arguments
    ///
    /// * `bytecode` - The contract WASM bytecode
    ///
    /// # Returns
    ///
    /// A 32-byte BLAKE3 hash of the manifest and bytecode.
    pub fn manifest_hash(&self, bytecode: &[u8]) -> [u8; 32] {
        let canonical = self.clone().normalized();
        let mut hasher = Hasher::new();
        hasher.update(bytecode);
        for slot in &canonical.declared_reads {
            hasher.update(slot);
        }
        for slot in &canonical.declared_writes {
            hasher.update(slot);
        }
        for slot in &canonical.commutative_keys {
            hasher.update(slot);
        }
        *hasher.finalize().as_bytes()
    }

    /// Exports the manifest as pretty-printed JSON.
    ///
    /// The output format is compatible with the CLI's manifest file format.
    ///
    /// # Example Output
    ///
    /// ```json
    /// {
    ///   "declared_reads": [
    ///     "0101010101010101010101010101010101010101010101010101010101010101"
    ///   ],
    ///   "declared_writes": [
    ///     "0202020202020202020202020202020202020202020202020202020202020202"
    ///   ],
    ///   "commutative_keys": [],
    ///   "storage_key_specs": [
    ///     { "offset": 4, "len": 32 }
    ///   ]
    /// }
    /// ```
    pub fn to_json_pretty(&self) -> String {
        let canonical = self.clone().normalized();
        let mut out = String::new();
        out.push_str("{\n");
        push_slot_list(&mut out, "declared_reads", &canonical.declared_reads, true);
        push_slot_list(
            &mut out,
            "declared_writes",
            &canonical.declared_writes,
            true,
        );
        push_slot_list(
            &mut out,
            "commutative_keys",
            &canonical.commutative_keys,
            true,
        );
        push_specs_list(&mut out, &canonical.storage_key_specs);
        out.push_str("}\n");
        out
    }
}

/// Builder for programmatically constructing contract manifests.
///
/// `ManifestBuilder` provides a fluent API for building manifests with
/// helper methods for common patterns (maps, vecs, blobs).
///
/// # Example
///
/// ```ignore
/// use truthlinked_sdk::manifest::ManifestBuilder;
/// use truthlinked_sdk::collections::{Namespace, StorageMap};
///
/// const NS_BALANCES: Namespace = Namespace([1u8; 32]);
/// let balances = StorageMap::<u64>::new(NS_BALANCES);
///
/// let manifest = ManifestBuilder::new()
///     .read_map_get(&balances, b"alice")
///     .write_map_set(&balances, b"bob")
///     .storage_key_spec(4, 32)
///     .build();
/// ```
#[derive(Clone, Debug, Default)]
pub struct ManifestBuilder {
    manifest: ContractManifest,
}

impl ManifestBuilder {
    /// Creates a new empty manifest builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a read slot (fluent API).
    pub fn read_slot(mut self, slot: [u8; 32]) -> Self {
        self.manifest.add_read_slot(slot);
        self
    }

    /// Adds a write slot (fluent API).
    pub fn write_slot(mut self, slot: [u8; 32]) -> Self {
        self.manifest.add_write_slot(slot);
        self
    }

    /// Adds a commutative slot (fluent API).
    pub fn commutative_slot(mut self, slot: [u8; 32]) -> Self {
        self.manifest.add_commutative_key(slot);
        self
    }

    /// Adds a storage key specification (fluent API).
    pub fn storage_key_spec(mut self, offset: usize, len: usize) -> Self {
        self.manifest
            .add_storage_key_spec(StorageKeySpec { offset, len });
        self
    }

    /// Declares reading a map's existence check slot.
    pub fn read_map_contains<V: Codec32>(mut self, map: &StorageMap<V>, key: &[u8]) -> Self {
        self.manifest.add_read_slot(map.exists_slot_for(key));
        self
    }

    /// Declares reading a map entry (both existence and value slots).
    pub fn read_map_get<V: Codec32>(mut self, map: &StorageMap<V>, key: &[u8]) -> Self {
        let (exists, value) = map.slots_for_key(key);
        self.manifest.add_read_slot(exists).add_read_slot(value);
        self
    }

    /// Declares writing a map entry (both existence and value slots).
    pub fn write_map_set<V: Codec32>(mut self, map: &StorageMap<V>, key: &[u8]) -> Self {
        let (exists, value) = map.slots_for_key(key);
        self.manifest.add_write_slot(exists).add_write_slot(value);
        self
    }

    /// Declares removing a map entry (both existence and value slots).
    pub fn write_map_remove<V: Codec32>(mut self, map: &StorageMap<V>, key: &[u8]) -> Self {
        let (exists, value) = map.slots_for_key(key);
        self.manifest.add_write_slot(exists).add_write_slot(value);
        self
    }

    /// Declares reading a vector element (length + element slots).
    pub fn read_vec_index<V: Codec32>(mut self, list: &StorageVec<V>, index: u64) -> Self {
        self.manifest
            .add_read_slot(list.len_slot())
            .add_read_slot(list.slot_for_index(index));
        self
    }

    /// Declares writing a vector element (reads length, writes element).
    pub fn write_vec_index<V: Codec32>(mut self, list: &StorageVec<V>, index: u64) -> Self {
        self.manifest
            .add_read_slot(list.len_slot())
            .add_write_slot(list.slot_for_index(index));
        self
    }

    /// Declares writing the vector length.
    pub fn write_vec_len<V: Codec32>(mut self, list: &StorageVec<V>) -> Self {
        self.manifest.add_write_slot(list.len_slot());
        self
    }

    /// Declares reading a blob chunk (length + chunk slots).
    pub fn read_blob_chunk(mut self, blob: &StorageBlob, chunk_index: u64) -> Self {
        self.manifest
            .add_read_slot(blob.len_slot())
            .add_read_slot(blob.slot_for_chunk(chunk_index));
        self
    }

    /// Declares writing a blob chunk (length + chunk slots).
    pub fn write_blob_chunk(mut self, blob: &StorageBlob, chunk_index: u64) -> Self {
        self.manifest
            .add_write_slot(blob.len_slot())
            .add_write_slot(blob.slot_for_chunk(chunk_index));
        self
    }

    /// Builds the final manifest, normalizing all entries.
    ///
    /// This consumes the builder and returns a normalized `ContractManifest`.
    pub fn build(mut self) -> ContractManifest {
        self.manifest.normalize();
        self.manifest
    }
}

/// Helper function to add a slot to a vector if not already present.
fn push_unique_slot(slots: &mut Vec<[u8; 32]>, slot: [u8; 32]) {
    if !slots.contains(&slot) {
        slots.push(slot);
    }
}

fn push_slot_list(out: &mut String, name: &str, slots: &[[u8; 32]], trailing_comma: bool) {
    out.push_str("  \"");
    out.push_str(name);
    out.push_str("\": [\n");
    for (idx, slot) in slots.iter().enumerate() {
        out.push_str("    \"");
        out.push_str(&hex32(slot));
        out.push('"');
        if idx + 1 != slots.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]");
    if trailing_comma {
        out.push(',');
    }
    out.push('\n');
}

fn push_specs_list(out: &mut String, specs: &[StorageKeySpec]) {
    out.push_str("  \"storage_key_specs\": [\n");
    for (idx, spec) in specs.iter().enumerate() {
        out.push_str("    { \"offset\": ");
        push_usize(out, spec.offset);
        out.push_str(", \"len\": ");
        push_usize(out, spec.len);
        out.push_str(" }");
        if idx + 1 != specs.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]\n");
}

fn push_usize(out: &mut String, value: usize) {
    // no_std-safe decimal conversion
    let mut buf = [0u8; 20];
    let mut n = value;
    let mut cursor = buf.len();
    if n == 0 {
        out.push('0');
        return;
    }
    while n > 0 {
        cursor -= 1;
        buf[cursor] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    for b in &buf[cursor..] {
        out.push(*b as char);
    }
}

fn hex32(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collections::Namespace;

    #[test]
    fn manifest_builder_dedups_and_hashes() {
        let map = StorageMap::<u64>::new(Namespace([1u8; 32]));
        let manifest = ManifestBuilder::new()
            .read_map_get(&map, b"alice")
            .read_map_get(&map, b"alice")
            .write_map_set(&map, b"alice")
            .storage_key_spec(4, 32)
            .storage_key_spec(4, 32)
            .build();

        assert_eq!(manifest.storage_key_specs.len(), 1);
        assert_eq!(manifest.declared_reads.len(), 2);
        assert_eq!(manifest.declared_writes.len(), 2);

        let hash_a = manifest.manifest_hash(&[1, 2, 3]);
        let hash_b = manifest.clone().normalized().manifest_hash(&[1, 2, 3]);
        assert_eq!(hash_a, hash_b);
    }

    #[test]
    fn manifest_json_contains_required_keys() {
        let manifest = ContractManifest::new();
        let json = manifest.to_json_pretty();
        assert!(json.contains("\"declared_reads\""));
        assert!(json.contains("\"declared_writes\""));
        assert!(json.contains("\"commutative_keys\""));
        assert!(json.contains("\"storage_key_specs\""));
    }

    #[derive(crate::Manifest)]
    #[manifest(
        read_derived(namespace = "tests.counter", key = "value"),
        write_derived(namespace = "tests.counter", key = "value"),
        key_spec(offset = 4, len = 32)
    )]
    struct CounterManifestSpec;

    #[test]
    fn derive_manifest_builds_contract_manifest() {
        let manifest = <CounterManifestSpec as Manifest>::manifest();
        assert_eq!(manifest.declared_reads.len(), 1);
        assert_eq!(manifest.declared_writes.len(), 1);
        assert_eq!(manifest.storage_key_specs.len(), 1);
    }

    const MANUAL_SLOT: [u8; 32] = [0xAA; 32];

    #[derive(crate::Manifest)]
    #[manifest(
        read_map(namespace = "tests.map", key = "alice"),
        write_map(namespace = "tests.map", key = "alice"),
        read_vec_index(namespace = "tests.vec", index = 3),
        write_vec_index(namespace = "tests.vec", index = 5),
        read_blob_chunk(namespace = "tests.blob", chunk = 0),
        write_blob_chunk(namespace = "tests.blob", chunk = 2),
        read_slot_expr = "crate::manifest::tests::MANUAL_SLOT",
        commutative_label = "tests.delta",
        key_spec(offset = 8, len = 24)
    )]
    struct RichManifestSpec;

    #[test]
    fn derive_manifest_supports_map_vec_blob_helpers() {
        let manifest = <RichManifestSpec as Manifest>::manifest();
        assert_eq!(manifest.declared_reads.len(), 7);
        assert_eq!(manifest.declared_writes.len(), 5);
        assert_eq!(manifest.commutative_keys.len(), 1);
        assert_eq!(manifest.storage_key_specs.len(), 1);
    }
}
