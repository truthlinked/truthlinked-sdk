# TruthLinked SDK

Its fast to reason about, and safe under deterministic execution. It is a Rust-first toolkit for `wasm32-unknown-unknown`, `no_std`, and production-grade deployment.

Start with `truthlinked-sdk` (runtime APIs) and use `truthlinked-sdk-macros` for derives and contract ergonomics. The template in `templates/rust-contract` is the quickest way to get a buildable project.

## What this repo contains
- `truthlinked-sdk`: storage, codec, manifest, events, ABI helpers, oracle bindings, testing utilities.
- `truthlinked-sdk-macros`: derive macros and attributes for events, manifests, codecs, and error handling.
- `templates/rust-contract`: a starter contract wired to the published SDK crate.

## Quickstart
```bash
cd templates/rust-contract
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
```

If you want a quick reference to the SDK APIs, read `truthlinked-sdk/README.md`.

## How contracts are expected to work
- Deterministic storage: use slots and namespacing; avoid hidden global state.
- Explicit manifests: declare reads/writes for conflict-aware execution.
- Stable ABI: encode inputs/outputs with the SDK codecs.
- No panics: build with `panic = "abort"` (already set in the template).

## Dev workflow
```bash
cargo check -p truthlinked-sdk
cargo check -p truthlinked-sdk-macros
```


## Support
Issues and requests: https://github.com/truth-linked/truthlinked-sdk/issues
