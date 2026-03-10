# TruthLinked SDK Workspace

This directory is now a standalone Cargo workspace for the publishable SDK crates:

- `truthlinked-sdk`
- `truthlinked-sdk-macros`

Local checks:

```bash
cargo check --manifest-path sdk/Cargo.toml -p truthlinked-sdk
cargo check --manifest-path sdk/Cargo.toml -p truthlinked-sdk-macros
```

Publish flow:

```bash
cd sdk
cargo publish -p truthlinked-sdk-macros
cargo publish -p truthlinked-sdk
```

The macros crate must be published first because `truthlinked-sdk` depends on it.
