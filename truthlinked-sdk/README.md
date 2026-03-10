# truthlinked-sdk

Rust SDK for writing TruthLinked WASM smart contracts. It is `no_std`, comes with storage, codec, manifest, event, oracle, and test utilities, and pairs with `truthlinked-sdk-macros` for derive support.

## Highlights
- Slot-based storage with `Slot`, `StorageMap`, `StorageVec`, `StorageBlob`
- ABI helpers, selectors, and calldata decoding
- Manifest builder and derive tooling for conflict-aware execution
- Structured events with indexed topics
- Deterministic HTTP oracle bindings
- `contract_entry!` macro for exports; panic=abort compatible

## Install
```toml
[dependencies]
truthlinked-sdk = "0.1.1"
```

Target: `wasm32-unknown-unknown` (build with `panic = "abort"`).

## Quickstart
```rust
#![no_std]
extern crate alloc;
use truthlinked_sdk::prelude::*;

#[derive(Manifest)]
#[manifest(
    read_derived(namespace = "demo.counter", key = "value"),
    write_derived(namespace = "demo.counter", key = "value")
)]
struct CounterManifest;

#[derive(Event)]
#[event(name = "CounterUpdated")]
struct CounterUpdated {
    #[topic] caller: [u8; 32],
    value: u64,
}

fn handle() -> Result<()> {
    let slot = storage::slot_for(&storage::namespace("demo.counter"), b"value");
    let next = slot.read_u64()?.saturating_add(1);
    slot.write_u64(next)?;
    log::emit_event(&CounterUpdated { caller: context::caller()?, value: next })?;
    context::set_return_data(&next.to_le_bytes())?;
    Ok(())
}

contract_entry!(handle);
```

Build:
```bash
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
```

## Feature flags
- `testing`: enables in-memory storage harness types for unit tests.

## License
MIT
