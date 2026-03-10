# truthlinked-sdk-macros

Procedural macros used by `truthlinked-sdk`. Publish this crate before `truthlinked-sdk`.

## What’s included
- `#[derive(Event)]` with `#[topic]`, `#[event(name = \"...\")]`, `#[event(anonymous)]`, `#[event(skip)]`, `#[event(type = \"...\")]`
- `#[derive(Manifest)]` to declare storage access (read/write/commutative slots, map/vec/blob helpers)
- `#[derive(BytesCodec)]` / `#[derive(Codec32)]` for deterministic encoding
- `#[error_code]` to declare numeric error codes
- `#[require]` to enforce preconditions with custom errors

## Example
```rust
use truthlinked_sdk::prelude::*;

#[derive(Event)]
#[event(name = "Transfer")]
struct Transfer {
    #[topic] from: [u8; 32],
    #[topic] to: [u8; 32],
    amount: u128,
}

#[derive(Manifest)]
#[manifest(
    read_derived(namespace = "demo.token", key = "balance"),
    write_derived(namespace = "demo.token", key = "balance")
)]
struct TokenManifest;

#[error_code(base = 10_000)]
#[derive(Clone, Copy)]
enum Error {
    ZeroAmount = 1,
}

#[require(amount > 0, Error::ZeroAmount)]
fn transfer(amount: u128) -> Result<()> {
    // contract logic...
    Ok(())
}
```

## License
MIT
