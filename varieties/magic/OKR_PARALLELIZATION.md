# RAV coverage OKR lane contract

The current parallel audit runs seven pinned Luna xhigh lanes from the clean
integration baseline. Each lane owns a separate worktree and branch; the base
checkout remains the integration lane.

| Lane | Worktree suffix | Branch | Ownership |
| --- | --- | --- | --- |
| White | `-wt-white` | `magic/okr-white` | White RAV definitions, contracts, scenarios, and coverage gaps |
| Blue | `-wt-blue` | `magic/okr-blue` | Blue RAV definitions, contracts, scenarios, and coverage gaps |
| Black | `-wt-black` | `magic/okr-black` | Black RAV definitions, contracts, scenarios, and coverage gaps |
| Red | `-wt-red` | `magic/okr-red` | Red RAV definitions, contracts, scenarios, and coverage gaps |
| Green | `-wt-green` | `magic/okr-green` | Green RAV definitions, contracts, scenarios, and coverage gaps |
| Multicolor | `-wt-multicolor` | `magic/okr-multicolor` | Guild, artifact, land, hybrid, Radiance, and remaining cross-color coverage |
| Core engine | `-wt-engine` | `magic/okr-engine` | Expansion-neutral stack, priority, targets, turns, SBAs, layers, replacements, multiplayer, and invariants |

## Merge protocol

1. A lane adds a minimal red regression and commits it separately.
2. The lane records the exact failing command and event-log evidence in the
   ledger, then adds a narrow fix or an ignored capability-gap probe.
3. The lane runs focused tests, workspace tests, strict Clippy, RAV parity, and
   relevant policy/event-log checks before committing the green result.
4. The integration lane cherry-picks red and green commits in order, resolving
   only intentional shared-manifest or scenario-index conflicts.
5. After each merge batch, the integration lane runs the complete verification
   gate and `rav-coverage-report`.

Card lanes must not change `engine/src` directly. When several cards need a
missing shared rule, they leave red probes and the Core engine lane owns the
substrate fix. The Core engine lane does not promote card definitions.

The base integration gate is:

```text
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p cardbench-magic-rav --bin rav-engine-parity
cargo run -p cardbench-magic-rav --bin rav-coverage-report
cargo run -p cardbench-magic-policies --bin rav-engine-audit
./adapters/harbor/run.sh engine verify magic
```

No lane may claim full-card fidelity when an activated ability, triggered
ability, replacement, or response window is omitted. Such boundaries remain
explicit in `ENGINE_BUG_LEDGER.md` and in ignored red probes.
