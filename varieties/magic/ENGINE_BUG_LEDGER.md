# Magic engine bug ledger

This is a public discovery ledger for the Rust RAV engine slice. It deliberately
separates engine defects from policy/harness outcomes. Each open engine item is
reproduced by `cargo run -p cardbench-magic-policies --bin rav-engine-audit`
from `varieties/magic`; the audit exits nonzero while an item remains open.

## Discovery batch 2026-07-30

| ID | Classification | Status | Reproducible observation |
| --- | --- | --- | --- |
| `game-over-accepts-gameplay-action` | Engine defect | Fixed; regression verified | A `PlayLand` policy move succeeds after `PlayerLost`. |
| `game-over-allows-direct-draw-mutation` | Engine defect | Fixed; regression verified | `draw_card` moves a library card after game termination. |
| `opening-hand-event-overstates-cards-drawn` | Engine defect | Fixed; regression verified | A two-card library returns success for a seven-card opening hand and logs seven cards drawn. |
| `deck-validation-allows-sideboard-copy-limit-bypass` | Expansion/deck substrate defect | Fixed; regression verified | Five nonbasic copies in the sideboard pass a four-copy deck rule. |
| `multiplayer-elimination-prematurely-ends-game` | Engine defect | Fixed; regression verified | In a three-seat game, one elimination makes `is_game_over()` true with two survivors and no winner. |
| `eliminated-player-receives-priority` | Engine defect | Fixed; regression verified | The priority cycle assigns priority to an eliminated seat. |
| `transmute-accepts-instant-speed-activation` | Engine/card-rules defect | Fixed; regression verified | Muddle the Mixture's transmute action succeeds in response to a spell on the stack. |
| `dredge-accepts-out-of-window-activation` | Engine/card-rules defect | Fixed; regression verified | The direct public dredge operation changes zones without a pending draw to replace. |
| `unsupported-spell-front-face-resolves-as-noop` | Engine/card-coverage defect | Fixed; regression verified | Muddle the Mixture casts and resolves with no supported front-face effect instead of surfacing a capability gap. |
| `combat-state-breaks-after-token-dies` | Engine invariant defect | Fixed; regression verified | A token blocker dies, is removed, and stale combat state then fails `validate_invariants()`. |
| `combat-view-breaks-after-token-dies` | Engine view/invariant defect | Fixed; regression verified | A token dies in combat, remains in historical combat state, and `GameView` dereferences its removed object. |

## Non-engine result retained for policy work

`policy-matrix-nonwinning-run` was not classified as an engine defect. The
`rav_boros_char_control` versus `rav_selesnya_convoke` pairing reached the old
80-turn development bound for shuffle seed 4 without an invariant failure. The
bound is now 120 turns, which permits the normal empty-library end condition in
a sixty-card two-player game; the 64-match policy matrix now completes with no
finding. It remains fail-closed if a future pairing reaches its bound.

## Corrective-loop rule

The audit is regression infrastructure, not a one-time report. Fixes are made
as a batch only after a discovery ledger is captured, then the same audit,
policy matrix, workspace tests, and Harbor verification are rerun. A new
finding reopens discovery before claims of a clean engine run.

## Regression evidence

After the corrective batch, the repeated audit completed with
`finding_count=0` over its public API probes and 64 shuffled policy matches
(four deck/policy pairings × 16 seeds). The original 16-seed fail-closed
tournament reported `failure_count=0`; `cargo test --workspace`, strict
Clippy, and Harbor's public 11-scenario engine verifier also passed.

## Expansion-round evidence

The broader six-deck policy matrix discovered and fixed the token-combat view
defect above. Its repeated public campaign now runs 90 ordered full-deck games
(six fixtures × five opponents × three deterministic seeds) with
`finding_count=0`. The dedicated wider run completed all 240 traces (six
fixtures × five opponents × eight seeds) with `failure_count=0`.
