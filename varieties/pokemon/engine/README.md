# SetBench Engine Core

Public Rust workspace for SetBench engine tasks.

This repo contains:

- `tcg_core/`: shared Pokemon TCG engine primitives and runtime hooks
- `tcg_rules_ex/`: rules extensions used by the engine
- `scaffold/src/df/`: Dragon Frontiers reference set implementation
- `scaffold/src/hp/`: Holon Phantoms reference set implementation
- `scaffold/src/cg/`: neutral Crystal Guardians stubs used as task overlays

This repo intentionally does not contain:

- Crystal Guardians gold implementations
- hidden evaluation fixtures
- held-out decklists or policy fixtures
- task-specific train data

## Build

```bash
cargo check --package tcg_expansions
```

## Image

The companion container image is intended to be published as:

- `ghcr.io/<org>/setbench-engine-core`

Consumers should pin an immutable digest instead of using a mutable tag.
