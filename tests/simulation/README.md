# Multi-node simulation testbed

This workspace-local simulation folder is the repository hook for a deterministic multi-node testbed that can exercise the existing crate architecture without forcing the entire network stack to boot in CI.

The harness intentionally remains dependency-free and can run in a one-shot
Python task under `python3 tests/simulation/run_simulation.py`. It is written
with the crate graph in mind:

- `onx-consensus` for round ordering and finality markers
- `onx-networking` for the packet simulation and message exchange model
- `onx-blocks::sync` for block progression and chain-following checks
- `onx-state-model` for the canonical shared root and state tree consistency

## Usage

```
python3 tests/simulation/run_simulation.py \
  --nodes 4 \
  --rounds 100 \
  --latency-ms 50 \
  --bandwidth-mbps 1000 \
  --drop-pct 0.0
```

This smoke test is wired into the GitHub Actions workflow under `.github/workflows/ci.yml`.

## Acceptance

The simulation harness verifies the requested path:

- 4 virtual nodes by default, extensible to 16
- 100 deterministic consensus rounds
- simulated latency capped at 50 ms
- no fork divergence across all nodes
- identical canonical state roots at the end of the simulation
