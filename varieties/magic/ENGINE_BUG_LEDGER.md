# Magic engine bug ledger

This is a public discovery ledger for the Rust RAV engine slice. It deliberately
separates engine defects from policy/harness outcomes. Each open engine item is
reproduced by `cargo run -p cardbench-magic-policies --bin rav-engine-audit`
from `varieties/magic`; the audit exits nonzero while an item remains open.

## Discovery batch 2026-07-30

| ID | Classification | Status | Reproducible observation |
| --- | --- | --- | --- |
| `damage-spell-targets-noncreature-permanent` | Engine target-legality defect — high severity, executable RAV direct-damage slice | Fixed; regression verified | `TargetRequirement::Any` accepted every battlefield permanent, so the executable Lightning Helix accepted an opponent's Plains and placed it on the stack as a legal target. That is not a legal damage target in this card slice (the engine has creatures, players, and no permanent-wide damage targeting semantics). The engine now represents the narrower player-or-creature target requirement and assigns it to Lightning Helix and Char. Exact red repro: `cargo test -p cardbench-magic-rav --test noncreature_damage_target_red -- --nocapture` from `varieties/magic`; it formerly failed with `Lightning Helix accepted a land target; result=Ok(()); events=["SpellCast { player: PlayerId(0), card: ObjectId(1) }"]`. |
| `life-total-overflow-panics-during-resolution` | Engine numeric/event-atomicity defect — high severity, ordinary RAV life-gain resolution | Fixed; regression verified | With the caster at `i16::MAX`, a legal Lightning Helix-equivalent RAV effect resolved its three player damage, then the controller-life addition overflowed and panicked before `LifeGained`, `SpellResolved`, and the zone move. The life-state, public view, scenario, and policy-result representation now use wide signed `i64` values, while effect amounts widen before arithmetic; the ordinary RAV resolution now completes its receipt and zone lifecycle beyond the former ceiling. Exact red repro: `cargo test -p cardbench-magic-engine --test life_total_overflow_red -- --nocapture` from `varieties/magic`; it formerly preserved only `[SpellCast, PriorityPassed(0), PriorityPassed(1), DamageDealtToPlayer(1, 3)]`. |
| `colored-mana-cost-overflow-underpays` | Engine mana-cost accounting / transaction defect — high severity, expansion-neutral casting and convoke | Open; red regression captured | Colored requirements are accumulated in a per-color `u8` with saturating addition. A legal model definition with 256 red symbols therefore accepts a 255-red pool, spends it, and writes `SpellCast` instead of rejecting atomically for missing mana. Exact red repro: `cargo test -p cardbench-magic-engine --test colored_mana_cost_overflow_red -- --nocapture` from `varieties/magic`; it fails with `256 required red mana was accepted with only 255 available; result: Ok(()); events: ["SpellCast { player: PlayerId(0), card: ObjectId(1) }"]`. |
| `continuous-pt-overflow-panics-mid-installation` | Engine continuous-layer arithmetic / transaction defect — high severity, public continuous-effect transition | Open; red regression captured | `characteristics` adds power and toughness modifiers in `i16`. Installing a representable `i16::MAX` power modifier on a 1-power creature therefore panics during the public post-installation SBA check after retaining the effect and writing `ContinuousEffectCreated`, rather than returning an engine-defined outcome atomically. Exact red repro: `cargo test -p cardbench-magic-engine --test continuous_pt_overflow_red -- --nocapture` from `varieties/magic`; it panics at `engine/src/game.rs:1003:81` with `attempt to add with overflow`, and the captured trace is `["ContinuousEffectCreated { source: ObjectId(1), target: ObjectId(1), layer: PowerToughness }"]`. |
| `damage-accumulator-overflow-panics-mid-resolution` | Engine damage accounting / resolution-transaction defect — high severity, multi-effect direct-damage substrate | Open; red regression captured | Marked permanent damage uses `i16` addition. Two individually representable damage effects (first `i16::MAX`, then `1`) targeting the same creature resolve before SBAs, so the second mark panics after the stack card has been popped and the first damage receipt has been written. Exact red repro: `cargo test -p cardbench-magic-engine --test damage_accumulator_overflow_red -- --nocapture` from `varieties/magic`; it panics at `engine/src/game.rs:2170:21` with `attempt to add with overflow` after `["SpellCast { player: PlayerId(0), card: ObjectId(1) }", "PriorityPassed { player: PlayerId(0) }", "PriorityPassed { player: PlayerId(1) }", "DamageDealtToPermanent { source: ObjectId(1), permanent: ObjectId(2), amount: 32767 }"]`. |
| `negative-damage-effect-leaves-invalid-transition` | Engine effect-definition validation / policy-transaction defect — high severity, direct-damage substrate | Open; red regression captured | `Effect::DealDamage` accepts a negative signed amount. A policy can cast that malformed executable definition; on resolution it writes `DamageDealtToPermanent { amount: -1 }`, leaves marked damage at `-1`, then the submitted pass returns an invariant error only after its state and event log have changed. The malformed effect must be rejected before it can make a transition. Exact red repro: `cargo test -p cardbench-magic-engine --test negative_damage_effect_transition_red -- --nocapture` from `varieties/magic`; the rejected pass leaves `damage=-1` and appends `["PriorityPassed { player: PlayerId(1) }", "DamageDealtToPermanent { source: ObjectId(1), permanent: ObjectId(2), amount: -1 }", "SpellResolved { card: ObjectId(1) }", "CardMoved { card: ObjectId(1), to: Graveyard }", "PolicyMoveSubmitted { player: PlayerId(1), policy: \"red-regression\", kind: PassPriority }"]`. |
| `radiance-colorless-target-omits-target` | Engine rules/effect-selection defect — medium severity, expansion-neutral radiance scope | Fixed; regression verified | A colorless creature is a legal radiance target, but the shared-color filter selected no objects because its color set was empty. The spell resolved with the target's base power unchanged (`Some(2)`, expected `Some(4)`). Red repro: `cargo test -p cardbench-magic-engine --test radiance_colorless_target_red -- --nocapture` from `varieties/magic`. The expansion-neutral selector now includes the legal target directly, then selects other creatures by shared color. |
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
| `defender-departure-retargets-declared-attackers` | Engine multiplayer/combat defect — high severity, 3+ player combat target identity | Fixed; regression verified | After `PlayerId(0)` declared attackers against next-seat `PlayerId(1)`, a resolving spell eliminated `PlayerId(1)`. The engine then made `PlayerId(2)` declare blockers and assigned the already-declared attacker's combat damage to `PlayerId(2)`, instead of removing attackers from combat when their original defender departed. Red repro: `cargo test -p cardbench-magic-engine --test multiplayer_combat_defender_departure_red -- --nocapture`. Combat now retains the original defender, clears attackers when that seat leaves, and skips blocker/damage steps instead of retargeting. |
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
| `negative-power-combat-damage-gains-life` | Engine combat/event defect | Fixed; regression verified | An unblocked creature reduced below zero power logged a negative `DamageDealtToPlayer` amount and increased the defending player's life; blocked creatures could likewise subtract marked damage. Nonpositive power now assigns no combat damage and emits no damage receipt. |
| `sorcery-cast-at-instant-speed` | Engine stack/timing defect | Fixed; regression verified | A nonactive player could cast a sorcery in response to an instant. The rejected cast is now atomic: the sorcery remains in hand and the stack and canonical event log are unchanged. |
| `casting-hands-priority-to-opponent` | Engine priority defect | Fixed; regression verified | Casting a spell immediately assigned priority to the next player. The caster now retains priority, the pass count resets, and an opponent's response is possible only after an auditable caster pass. |
| `public-mana-action-preserves-stale-pass-sequence` | Engine priority/event defect — high severity, public mana-action API | Fixed; regression verified | After `PlayerId(0)` passed in Upkeep, `PlayerId(1)` called `add_mana_from_action` and then passed. Because the mana action did not reset the prior pass, the engine advanced to Draw instead of returning priority to `PlayerId(0)`. The trace contains `PriorityPassed(0)`, `ManaAdded(1, Blue, 1)`, `PriorityPassed(1)`, then `StepBegan(Draw)`, skipping the response window. Red repro: `cargo test -p cardbench-magic-engine --test priority_mana_action_contract -- --nocapture`. The public mana-action seam now resets the pass sequence just like an intrinsic mana ability. |
| `zero-amount-mana-action-reopens-priority` | Engine action-validity/event defect — medium severity, public mana-action API | Fixed; regression verified | After `PlayerId(0)` passes in Upkeep, `add_mana_from_action(PlayerId(1), Blue, 0)` returned success even though the pool remained zero. It emitted `ManaAdded { player: PlayerId(1), color: Blue, amount: 0 }` and reset the pass sequence, letting a caller manufacture a priority-consuming non-action. Red repro: `cargo test -p cardbench-magic-engine --test zero_amount_mana_action_red -- --nocapture` from `varieties/magic`. The public seam now rejects zero amounts before mutating mana, event log, or pass state. |
| `weakness-report-preserves-stale-pass-sequence` | Engine priority/event defect — medium severity, policy reporting API | Fixed; regression verified | After `PlayerId(0)` passed in precombat main, `PlayerId(1)` submitted `ReportEngineWeakness` and then passed. The accepted non-pass report left the old pass count in place, so the engine advanced to Beginning of Combat instead of returning a response window to `PlayerId(0)`. Red repro: `cargo test -p cardbench-magic-engine --test report_priority_reset_red -- --nocapture` (trace: `PriorityPassed(0)`, `EngineWeaknessRevealed(1)`, `PolicyMoveSubmitted(ReportEngineWeakness)`, `PriorityPassed(1)`, `StepBegan(BeginningOfCombat)`). Reports now reset the pass sequence while remaining otherwise observational. |
| `stack-invariant-allows-fabricated-player-target` | Engine invariant/stack integrity defect — high severity, public-state mutation audit | Fixed; regression verified | A legal instant targeting `PlayerId(1)` was externally rewritten on the public stack to target unseated `PlayerId(99)`. `validate_invariants()` returned `Ok(())`, treating an impossible cast-time target as a plausible later-illegal target. Red repro: `cargo test -p cardbench-magic-engine --test stack_target_integrity_red -- --nocapture` (output: `fabricated stack-target audit result: Ok(()); ... targets: [Player(PlayerId(99))]`). The invariant now requires every stack player target to name a seated player while still allowing a later-lost seated target to be countered by the rules. |
| `setup-transitions-accepted-during-live-game` | Engine setup/turn-transition defect | Fixed; regression verified | A live game accepted `load_deck_into_library` for an empty seat and `draw_opening_hand` for a prepared seat, allowing hidden cards plus `DeckLoaded`, `LibraryShuffled`, or `OpeningHandDrawn` setup receipts to be injected mid-turn without priority. Both setup APIs now reject atomically after `begin_game`; a second opening hand also cannot be appended to a nonempty hand during setup. |
| `zone-change-silently-deletes-continuous-effect` | Engine event-log defect | Fixed; regression verified | When an effect source or target left the battlefield, the effect disappeared from state without `ContinuousEffectExpired`, leaving a creation-only lifecycle in canonical logs. Zone and player-departure cleanup now emits the expiration receipt after the corresponding move/leave event. |
| `public-continuous-effect-installation-skips-sbas` | Engine transition/SBA defect — high severity, public effect-installation API | Fixed; regression verified | Installing a legal permanent `ModifyPowerToughness { toughness: -1 }` effect on a one-toughness battlefield creature returns success but leaves it on the battlefield. The only receipt is `ContinuousEffectCreated`; no `StateBasedAction` or graveyard move follows, despite the completed transition having created a zero-toughness creature. Red repro: `cargo test -p cardbench-magic-engine --test continuous_effect_sba_public_transition_red -- --nocapture` from `varieties/magic` (trace: `zone=Some(Battlefield); events=["ContinuousEffectCreated { source: ObjectId(1), target: ObjectId(2), layer: PowerToughness }"]`). Public installation now reaches the SBA fixed point atomically, while an internal resolver primitive preserves the rule that SBAs wait until the entire spell finishes resolving. |
| `terminal-effect-installation-rejection-mutates-state` | Engine transaction/terminal-lifecycle defect — high severity, public effect-installation API | Fixed; regression verified | After `PlayerId(1)` lost and the terminal `GameEnded { winner: Some(PlayerId(0)) }` receipt was present, `add_continuous_effect` returned an error only after appending `ContinuousEffectCreated` and retaining the new effect. The rejected call therefore left the event log nonterminal and failed the invariant audit. Red repro: `cargo test -p cardbench-magic-engine --test terminal_continuous_effect_atomicity_red -- --nocapture` from `varieties/magic`. The public transition now rejects terminal games before it installs effects, changes timestamps, or writes lifecycle receipts. |
| `terminal-begin-game-rejection-appends-turn-event` | Engine transaction/terminal-lifecycle defect — high severity, public start-game API | Fixed; regression verified | A fixture reached a valid terminal state through `check_state_based_actions`, with `PlayerLost(1)` followed by `GameEnded { winner: Some(0) }`. `begin_game()` then returned `Err(IllegalAction("terminal game must end with exactly one matching GameEnded event"))` only after changing the live-game state and appending `StepBegan { turn: 1, active_player: PlayerId(0), step: Untap }` after the terminal receipt. The rejected call left an invalid event chronology instead of remaining atomic. Red repro: `cargo test -p cardbench-magic-engine --test terminal_begin_game_atomicity_red -- --nocapture` from `varieties/magic`. Start now rejects an already-terminal fixture before changing live-game fields or emitting a turn receipt. |
| `shown-scenario-count-understates-corpus` | Public RAV fixture defect | Fixed; regression verified | Severity low, fixture/audit scope: `scenarios/shown.toml` and an internal test declared 11 scenarios while the parity runner executed 12. A contract test now compares the public index with the executable corpus; parity reports all 12 fixed-digest scenarios. |
| `direct-draw-bypasses-turn-state-machine` | Engine transition defect — high severity, public gameplay API | Fixed; regression verified | After `begin_game` reached `Upkeep`, public `draw_card(PlayerId(0), None)` succeeded even though no draw instruction or pending Draw-step replacement existed. It moved `ObjectId(1)` from library to hand and emitted `CardMoved` without a policy receipt. Red repro: `cargo test -p cardbench-magic-engine --test direct_draw_timing_contract -- --nocapture` (output: `card zone: Some(Hand); new events: ["CardMoved { card: ObjectId(1), to: Hand }"]`). Live draws now require the active player's pending Draw-step marker and clear it atomically. |
| `unstarted-fixture-draw-leaves-stale-replacement-marker` | Engine transition defect — medium severity, authored-fixture turn-state scope | Fixed; regression verified | A fixture can publicly advance to player one's pending draw before `begin_game`; `resolve_pending_draw(PlayerId(1), None)` moved the card to hand but left `draw_replacement_pending` true. The following priority pass was rejected as an unresolved mandatory decision. Red repro: `cargo test -p cardbench-magic-engine --test direct_draw_timing_contract pending_draw_resolution_clears_its_marker_in_an_unstarted_fixture_turn -- --exact --nocapture`. Any existing pending marker is now consumed atomically when the ordinary draw resolves; the `started` guard remains limited to rejecting arbitrary live draws. |
| `transmute-hands-priority-to-opponent` | Engine priority defect — medium severity, activated-ability and RAV-policy scope | Fixed; regression verified | A legal Transmute activation recorded its search/shuffle events then set priority to `PlayerId(1)` rather than the activating `PlayerId(0)`. Red repro: `cargo test -p cardbench-magic-engine --test turn_lifecycle_adversarial transmute_activation_keeps_priority_with_its_controller -- --exact --nocapture`; the trace ends with `Transmuted(0, 1, 2)`, followed by `left: PlayerId(1)`, `right: PlayerId(0)`. The supported atomic activation now retains its controller's priority. |
| `priority-actions-bypass-mandatory-combat-declarations` | Engine priority/combat transition defect — high severity, all instant/activated action scope | Fixed; regression verified | At `DeclareAttackers` before attackers were declared, and at `DeclareBlockers` before blockers were declared, `cast_spell` accepted an instant. Each red trace logged `StepBegan` for the declaration step immediately followed by `SpellCast`, even though the required turn-based declaration must happen before either player receives priority. Red repro: `cargo test -p cardbench-magic-engine --test stack_priority_turn_based_action_red -- --nocapture`. All ordinary priority actions now reject until the appropriate combat declaration completes. |
| `eliminated-player-leaks-through-policy-view` | Engine policy-view/multiplayer defect — high severity, all multiplayer target-selection scope | Fixed; regression verified | In a continuing three-player game after `PlayerId(1)` lost, `view_for_player(PlayerId(0)).opponent_life` still returned `[(PlayerId(1), 0), (PlayerId(2), 20)]`. A policy that selects the first opponent could submit an illegal target for the departed seat even though the engine correctly rejects it. Red repro: `cargo test -p cardbench-magic-engine --test multiplayer_view_elimination_red -- --nocapture`. The view now filters departed seats from opponent life and battlefield projections. |
| `muddle-counter-policy-skips-available-mana` | Policy coverage defect — medium severity, natural RAV counterspell exercise | Fixed; regression verified | With Muddle in hand, two untapped Islands, priority, and an opposing Char on the stack, `rav.dimir-transmute-helix.v1` returned `PassPriority` instead of activating Blue mana toward its available counterspell. The engine's targeted counter scenario still passed, but natural policy logs contained zero `SpellCountered` events. Red repro: `cargo test -p cardbench-magic-policies dimir_transmute_helix::tests::activates_available_blue_mana_before_countering_a_stack_spell -- --exact --nocapture`. The policy now activates exactly the needed available Blue mana before submitting Muddle, and its regression records `SpellCountered`. |
| `public-game-fields-can-bypass-transition-machine` | Engine API encapsulation weakness | Open; explicitly bounded | Several authored-fixture fields are public; a hostile caller can construct a shape-valid state without using a legal transition. `validate_invariants` detects invalid shapes but cannot establish transition provenance. |
| `saturated-public-mana-action-logs-phantom-mana` | Engine mana/event-atomicity defect — high severity, public and intrinsic priority actions and audit receipts | Fixed; regressions verified | `add_mana_from_action` formerly used saturating pool addition but recorded the requested amount unconditionally. With a Red pool already at `u8::MAX`, a further Red action for `1` succeeded and wrote `ManaAdded { amount: 1 }` even though the observable pool remained `255`; the receipt therefore claimed mana that did not exist. The same unchecked saturation let an intrinsic land ability tap its source and write `ManaAbilityActivated` plus `ManaAdded` without increasing a full pool. Every direct mana producer now preflights pool capacity and rejects atomically before source, pass, pool, or event changes. Red repros: `cargo test -p cardbench-magic-engine --test mana_action_capacity_atomicity_red -- --nocapture` and `cargo test -p cardbench-magic-engine --test intrinsic_mana_capacity_atomicity_red -- --nocapture` from `varieties/magic`. |
| `mana-pool-total-overflows-during-bound-activation` | Engine mana-payment/state-machine defect — high severity, bound mana abilities | Fixed; regression verified | A player may hold the maximum valid `u8` amount of Blue and Red mana through the public mana-action API, then activate a legal `{1}` paid mana bundle. Generic-cost payment formerly totaled the five `u8` pool entries with ordinary addition and panicked on the 510-mana intermediate sum, rather than completing the activation. The payment path now uses a widened `u16` total; the legacy policy-facing `u8` total is explicitly saturated and cannot decide payment affordability. Red repro: `cargo test -p cardbench-magic-engine --test bound_mana_total_overflow_red -- --nocapture` from `varieties/magic`; its trace clears setup receipts before the panic so the recorded action list is empty. |

Severity for the newly fixed engine findings is high for
`negative-power-combat-damage-gains-life`, medium for
`setup-transitions-accepted-during-live-game` and
`zone-change-silently-deletes-continuous-effect`, and medium rules-conformance
severity for `sorcery-cast-at-instant-speed` and
`casting-hands-priority-to-opponent`.

## RAV set and mechanic coverage gaps (not engine defects)

The complete public inventory contains 306 printings / 291 unique names; the
executable compatibility slice contains 68 printings / 53 unique names.
Twenty executable printings are the four printings of each of five basic lands;
the other 48 names are deliberately bounded cards exercising generic casting,
base characteristics, mana abilities, targeted/global damage, temporary
modifiers, token creation, Convoke, Dredge, Radiance, and Transmute. The
remaining 238 unique names are explicitly catalog-only and fail closed with
`card-specific-rules-not-implemented`. This boundary is enforced by
`catalog_coverage::executable_slice_size_is_explicit_and_does_not_masquerade_as_set_coverage`.

The four advertised RAV mechanics have targeted compatibility examples
(Convoke, Dredge, Radiance, and Transmute), but those examples do not establish
set-wide mechanic or card-text fidelity. In particular, the executable
Brownscale definition claims only its draw-replacement/base-characteristic
slice, Muddle claims only its narrow counter/transmute slice, and the engine
does not infer any semantics for the other cataloged cards. The 48 public
scenarios are behavioral probes for the implemented slice, not coverage of all
291 names or all interactions among the four mechanics.

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

After the Muddle-response policy correction, a fresh exported one-seed matrix
ran 210 ordered games with `failure_count=0` and a complete 210-log manifest.
Manual event-log review still found zero natural `SpellCountered` receipts.
The new policy regression proves the policy activates two available Islands,
casts Muddle, and records `SpellCountered` in its controlled response window;
the zero-count matrix result remains a coverage gap rather than evidence that
the counter path is broadly exercised by stochastic full-deck play.

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
