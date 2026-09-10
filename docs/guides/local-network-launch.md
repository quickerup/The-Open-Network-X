# Launch Guide

This repository ships a deterministic `onx-genesis` bootstrap generator that emits a canonical genesis block object and four bootstrap configuration stubs for the validator network.

## Generate

```sh
cargo run -p onx-genesis -- --config config/genesis.toml --out target/onx-genesis
```

The generator emits:

- `genesis.boc` — a deterministic genesis BoC payload marker and canonical masterchain/genesis record
- `shard-header-0.boc` — the initial shard header bootstrap object
- `node-0.toml` through `node-3.toml` — four bootstrap configuration stubs for fresh `onxd` nodes

## Launch four nodes

Run the four nodes using the generated onxd configuration files:

```sh
cargo run -p onxd -- --config target/onx-genesis/node-0.toml
cargo run -p onxd -- --config target/onx-genesis/node-1.toml
cargo run -p onxd -- --config target/onx-genesis/node-2.toml
cargo run -p onxd -- --config target/onx-genesis/node-3.toml
```

Each node will use the generated `genesis.boc` and bootstrap from the same masterchain genesis and shard header template. Workchain and validator metadata are expressed as a deterministic, inspectable canonical object for the local testnet bootstrap.
