# Masterchain system contracts

This repository ships deterministic TVM assembly stubs and reference source adapters for:

- `elector.tvm` / `elector.rs` — validator stake deposit and election winner logic.
- `config.tvm` / `config.rs` — parameters for gas rates and validator set config.
- `storage.rs` — global workchain configuration root contract source.

The Rust source adapters converge on the existing consensus election primitives in `onx-consensus` rather than inventing a second election engine.
