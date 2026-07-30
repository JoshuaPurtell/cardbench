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
| `unsupported-spell-front-face-resolves-as-noop` | Engine/card-coverage defect | Fixed; regression verified | Muddle the Mixture's counter face is executable, targets only a visible instant or sorcery spell on the stack, and emits a distinct counter receipt. |
| `combat-state-breaks-after-token-dies` | Engine invariant defect | Fixed; regression verified | A token blocker dies, is removed, and stale combat state then fails `validate_invariants()`. |
| `combat-view-breaks-after-token-dies` | Engine view/invariant defect | Fixed; regression verified | A token dies in combat, remains in historical combat state, and `GameView` dereferences its removed object. |
| `transmute-policy-view-hides-all-legal-search-targets` | Engine policy-view defect | Fixed; regression verified | A whole-deck policy can activate transmute only by naming a hidden library object ID, but its `GameView` exposes no legal candidate. |
| `deck-load-rejection-partially-mutates-library` | Engine setup-transaction defect | Fixed; regression verified | A deck with valid entries before an unknown card returns an error after placing the valid cards into the library. |
| `permanent-continuous-effect-outlives-source` | Engine invariant/effect-lifetime defect | Fixed; regression verified | State-based actions can kill a permanent-effect source while retaining its continuous effect, leaving a successful transition that fails the invariant audit. |
| `blocked-attacker-damages-player-after-blocker-leaves` | Engine combat-rules defect | Fixed; regression verified | A creature blocked before its blocker leaves combat is incorrectly treated as unblocked and deals player damage. |
| `zero-survivor-winner-query-panics` | Engine terminal-state defect | Fixed; regression verified | Simultaneous player losses leave no survivors and `Game::winner()` indexes an empty survivor list instead of returning no winner. |
| `step-marker-follows-automatic-work` | Event/state-machine defect | Fixed; regression verified | Draw, untap, and combat damage are emitted before the `StepBegan` event that must delimit them. |
| `setup-is-logged-after-turn-one-main` | Turn-start/event defect | Fixed; regression verified | A full-deck log begins at turn-one precombat main and only then loads decks and opening hands. |
| `untap-and-cleanup-grant-priority` | Engine turn-rules defect | Fixed; regression verified | Policies can pass, cast, or activate mana in Untap and ordinary Cleanup. |
| `first-draw-skipped-in-multiplayer` | Engine turn-rules defect | Fixed; regression verified | Player zero incorrectly skips the first draw in a three-player game. |
| `empty-combat-enters-blockers-and-damage` | Engine combat-rules defect | Fixed; regression verified | No declared attackers still reaches Declare Blockers and Combat Damage. |
| `pass-priority-creates-hidden-combat-declarations` | Engine transition/audit defect | Fixed; regression verified | A pass silently sets attacker/blocker declaration state without the turn-based action or its event. |
| `active-player-loss-deadlocks-combat` | Engine multiplayer-transition defect | Fixed; regression verified | A departed active player leaves a declaration step that no living player can legally perform. |
| `player-loss-leaves-owned-objects-in-game` | Engine multiplayer-zone defect | Fixed; regression verified | A player leaves a continuing multiplayer game but retains hand, library, battlefield, and stack objects. |
| `token-sba-has-no-terminal-lifecycle-event` | Event/zone-accounting defect | Fixed; regression verified | A token is removed by SBA without a `TokenCeasedToExist` receipt. |
| `transmute-shuffle-is-not-auditable` | Event/audit defect | Fixed; regression verified | Transmute changes a library's order but logs no `LibraryShuffled` receipt. |
| `terminal-game-has-no-end-event` | Event terminal-state defect | Fixed; regression verified | Logs have `PlayerLost` but no canonical terminal winner/draw event. |
| `game-ended-precedes-terminal-policy-receipt` | Event chronology defect | Fixed; regression verified | A passing policy move causes automatic loss, yet its acceptance receipt follows `GameEnded`. |
| `policy-cannot-choose-draw-replacement` | Policy/state-machine coverage defect | Fixed; regression verified | A full policy game could not choose Dredge at its draw-replacement boundary, so `Dredged` was absent from the event corpus despite direct API coverage. |
| `valid-draw-misclassified-as-matrix-failure` | Policy-harness result-classification defect | Fixed; regression verified | Two Char resolutions in the eight-seed corpus reduced both players to zero in one SBA pass; the engine correctly logged `GameEnded { winner: None }`, but the matrix treated the valid completed draw as a failure. |
| `reference-dimir-convoke-is-64-cards` | Deck-fixture defect | Fixed; regression verified | A declared 60-card probe contains 64 cards, weakening matrix comparability. |
| `pending-draw-replacement-allows-priority-interleaving` | Engine transition defect | Fixed; regression verified | While a mandatory Dredge-or-draw decision was pending, a policy could cast an instant, mutate the stack, and leave the stale replacement marker to be caught only later by invariants. Priority-bearing actions now reject atomically until `PolicyAction::Draw` resolves the decision. |
| `event-log-external-mutation-not-detected` | Engine audit defect | Fixed; regression verified | A caller could append, reorder, remove, or rewrite valid-looking public events and still pass the prior semantic event checks. The engine now seals its canonical event sequence and detects edits; `clear_event_log` is the explicit synchronized reset. |
| `public-seat-id-corruption-not-rejected` | Engine audit defect | Fixed; regression verified | Mutating player IDs or extending the public seat vector could evade the former indirect seat lookup. The audit now requires the original fixed seat count and exact `PlayerId(n)`/index correspondence. |
| `active-eliminated-seat-accepted-by-invariant-audit` | Engine audit defect | Fixed; regression verified | A continuing game whose active player had already lost could pass the former audit if priority had been reassigned. The invariant now rejects an eliminated active seat. |
| `zero-timestamp-continuous-effect-accepted` | Engine audit defect | Fixed; regression verified | A fabricated continuous effect with timestamp zero evaded the uniqueness/future-timestamp test. Effects now require positive timestamps as well as monotonicity. |
| `muddle-counterspell-front-face-unmodeled` | Engine/card-coverage defect | Fixed; regression verified | The prior honest fallback rejected Muddle's unsupported front face. The narrow executable RAV slice now targets an instant or sorcery card on the stack, removes it during resolution, and emits `SpellCountered`; its transmute behavior remains intact. |
| `public-game-fields-can-bypass-transition-machine` | Engine API encapsulation weakness | Open; explicitly bounded | Several authored-fixture fields are public; a hostile caller can construct a shape-valid state without using a legal transition. `validate_invariants` detects invalid shapes but cannot establish transition provenance. |

## Non-engine result retained for policy work

`policy-matrix-incomplete-run` is not classified as an engine defect. The
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

After the initial corrective batch, the repeated audit completed with
`finding_count=0` over its public API probes and 64 shuffled policy matches
(four deck/policy pairings × 16 seeds). The original 16-seed fail-closed
tournament reported `failure_count=0`; `cargo test --workspace`, strict
Clippy, and Harbor's public 11-scenario engine verifier also passed.

## Expansion-round evidence

The broader six-deck policy matrix discovered and fixed the token-combat view
defect above. Its repeated public campaign ran 90 ordered full-deck games
(six fixtures × five opponents × three deterministic seeds) with
`finding_count=0`. That historical campaign is retained as baseline evidence;
the current fifteen-deck corpus is recorded with its own canonical event-log
manifest rather than overwriting that result.

## Invariant-expansion evidence

The invariant expansion now includes twelve broad public-API contract tests,
seven event-log contracts, five turn-state-machine contracts, seven zone/SBA
contracts, and three deterministic stateful property tests. The property tests
execute 64 three-player traces, each with 199 accepted and 33 intentionally
rejected policy actions, checking the invariant audit and every player view
after every attempt. Rules review of the initial 210-log corpus surfaced the
fixed turn, event, token, and multiplayer-transition defects above; the new
tests make those transitions fail closed rather than merely documenting them.
The draw-replacement ABI is additionally exercised through a real three-player
policy submission: its canonical trace contains `Dredged` followed by a
`PolicyMoveSubmitted { kind: Draw }` receipt. The open public-field boundary
remains a deliberate limitation of the fixture-oriented API, not a waived
invariant failure.

## Transition-hardening evidence

The follow-up adversarial tranche adds four public-state mutation tests (18
deliberate corruptions), four draw-replacement/multiplayer tests, two RAV
stack-target tests, and a fixed-digest public Muddle counterspell scenario. It
specifically exercises visible-stack countering, response LIFO order,
all-targets-illegal rules counters, mandatory replacement decisions,
three-player elimination handoff, and combat elimination. The engine suite
contains 53 tests, including a 64-seed stateful policy campaign; all pass after
the fixes above. The public field API remains intentionally
fixture-oriented, so a shape-valid externally fabricated state remains the
open provenance boundary rather than a claim that every state arose through a
legal transition.

The fresh post-hardening replay is retained at
`artifacts/rav-reference-deck-matrix/round7-transition-hardening/` (ignored
run output): 1,680 ordered games, 1,213,213 accepted moves, and 3,417,010
events. It has `failure_count=0`, 794 seat-zero wins, 884 seat-one wins, and
two valid draws. The manifest contains exactly 1,680 uniquely named logs;
manual review found zero event-count/header mismatches, zero unexpected
termination values, and a terminal `GameEnded` receipt in every trace.
Representative logs confirmed the ordered Dredge and Transmute receipts and
the simultaneous-loss cleanup ordering.

This natural policy corpus has 232 `Dredged` and 773 `Transmuted` events but
zero `SpellCountered` and zero `SpellCounteredByRules` events. That is a
coverage observation, not a clean bill of health for either counter path. The
persisted `rav_muddle_counterspell` scenario and RAV stack-target contracts
therefore remain mandatory targeted evidence for countering, LIFO response
resolution, and rules-based target failure.

## Eight-seed event-log review

After the draw-replacement and valid-draw corrections, the full public matrix
ran all fifteen ordered-deck policies against one another for seeds 0 through
7: 1,680 games, 1,213,213 accepted policy moves, and 3,417,010 logged
events. It completed with `failure_count=0`; 794 games were won by seat zero,
884 by seat one, and two were rules-valid simultaneous-loss draws. The corpus
contains 232 `Dredged` events in 102 games, 773 `Transmuted` events, 2,315
`TokenCeasedToExist` events, and 1,509 `ContinuousEffectExpired` events.

Manifest/header counts matched every log, every trace ended in exactly one
`GameEnded` record, and aggregate event checks found zero priority passes in
automatic Untap/Cleanup steps and zero empty-attacker combats that entered
blockers or damage. The corpus did not naturally produce
`SpellCounteredByRules` or `EngineWeaknessRevealed`; those are retained as
targeted public invariant/reporting tests rather than misrepresented as
whole-deck coverage.
