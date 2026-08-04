# Magic architecture contract

Status: **Milestone 1 (architecture and contracts)**. This document is the
authority for crate boundaries, the two-player assumption inventory, and the
mapping from each non-negotiable invariant to the test that defends it.

Nothing in this document claims that Commander, Two-Headed Giant, draft, or
complete Magic rules are supported. The supported boundary is stated in
[Supported boundary](#supported-boundary) and is machine-readable through
`ProtocolCapabilities::current()`.

---

## 1. Layers and crate boundaries

Four layers, with a strict dependency direction. An arrow means "may depend
on"; anything not drawn is forbidden.

```
      transports / clients          (layer D — does not exist yet)
                 |
                 v
   cardbench-magic-session          (layer C — does not exist yet)
          |                \
          v                 v
cardbench-magic-engine   cardbench-magic-protocol
      (layer A)                (layer B)
```

| Crate | Layer | Exists | External deps | Purpose |
| --- | --- | --- | --- | --- |
| `cardbench-magic-engine` | A | yes | none | Rules. Knows nothing about transport, UI, policy, or sessions. |
| `cardbench-magic-rav` | A | yes | none | One expansion's card definitions and fixtures. |
| `cardbench-magic-policies` | — | yes | none | Reference policies and campaign runners. Migrates to layer C's controller interface in M2. |
| `cardbench-magic-protocol` | B | **yes (this milestone)** | `serde` | Owned, versioned transport schema. |
| `cardbench-magic-session` | C | no (M4) | engine + protocol | Owns one `Game`, one controller per seat, projection, and submission. |

### 1.1 The edge that is deliberately absent

**`cardbench-magic-protocol` does not depend on `cardbench-magic-engine`, and
must not.**

This is the structural enforcement of the invariant "transport code cannot
create a second rules or legality implementation": there is nothing in the
protocol crate to check legality *with*. The compiler enforces it; no reviewer
has to notice.

Consequences, all intended:

- Protocol types are **owned mirrors**, not re-exports. `CardName(String)`
  replaces the engine's `&'static str`; `SeatId(u16)` is not `PlayerId(usize)`;
  `TargetDto::Seat` replaces `Target::Player`.
- The engine is free to add an internal variant without a schema bump. The
  schema changes only when the wire contract changes.
- Staleness validation and event redaction are total functions over owned data
  (`protocol/src/validate.rs`, `protocol/src/scope.rs`), so they are
  exhaustively testable without constructing a game — see `tests/staleness.rs`
  and `tests/redaction.rs`.
- Translation cost is real and lands in exactly one place: the session crate's
  projection. That is the point. One reviewable file is where a hidden-zone
  leak can be introduced, rather than anywhere a `GameView` is passed.

### 1.2 The hermetic-core rule

`engine`, `policies`, and `sets/ravnica_city_of_guilds` have **zero external
dependencies** and keep them. `serde` is confined to `protocol` (and, later,
`session`). Adding a dependency to a layer-A crate is a contract change, not a
routine edit.

### 1.3 What the session crate will and will not do (M4)

Will: own the `Game`; assign a controller per seat; issue one `ActionRequest`
at a time; project `Game` → `MatchObservation` per `ViewerScope`; validate and
submit through `Game::submit_policy_move`; maintain the `CommandLedger`; own
the seeded RNG.

Will not: mutate `Game` or `PlayerState` fields; re-derive legality; hold a
second copy of rules state. **Controllers receive an `ActionRequest` and return
a `GameCommand`. They never receive `&mut Game`.**

---

## 2. Two-player assumption inventory

Required by Milestone 1. Every entry is a live citation at the head this
document was written against. Each is classified by whether it is a *bug*, a
*limitation*, or *correct as written*.

### 2.1 Interface shape — the observation flattens the table

| # | Site | Assumption | Class |
| --- | --- | --- | --- |
| A1 | `engine/src/game.rs:639` — `GameView::opponent_life: Vec<(PlayerId, i64)>` | Already a list, but paired with A2 it invites `.first()`. | limitation |
| A2 | `engine/src/game.rs:679` — `GameView::opponent_battlefield: Vec<CardView>` | Every opponent's permanents flattened into one list with no seat partition. A pod consumer cannot tell whose creature is whose except through `CardView::controller`. | **limitation** |
| A3 | `engine/src/game.rs:5280`–`5365` — the view builder | Constructs A2 by concatenation. | limitation |
| A4 | `GameView` splits `own_*` from `opponent_*` throughout | Encodes "me and the other one" as the shape of the world. | limitation |

Resolved in the protocol schema: `MatchObservation.seats: Vec<SeatObservation>`
is seat-indexed with no own/opponent split, and
`MatchObservation::living_opponents_of` returns identified seats rather than an
aggregate. `redacted_for` decides entitlement per seat. **The engine's
`GameView` is unchanged by this milestone**; the session crate projects from it.

### 2.2 Combat — one defender per combat, not per attacker

| # | Site | Assumption | Class |
| --- | --- | --- | --- |
| B1 | `engine/src/game.rs:8044` — `let defending_player = self.next_player(self.active_player);` | The defender is *derived*, never chosen. With three seats this silently attacks the next living seat in turn order. | **limitation** (correct for a duel, wrong for a pod) |
| B2 | `engine/src/game.rs:760` — `CombatState::defending_player: Option<PlayerId>` | One defender for the whole combat, not one per attacker. | **limitation** |
| B3 | `engine/src/model.rs:5073` — `CombatBlock { attacker, blocker }` | At most one blocker per attacker. Already documented in-place as a declared capability gap. | limitation (documented) |

Modelled in the protocol schema ahead of the rules work:
`AttackDeclaration { attacker, controller, defender: DefenderDto }` is
per-attacker, and `DefenderDto` admits a seat or a team. This is reported as
`FeatureCapability::ChosenAttackDefender = Modelled`, **not** `Supported`, and
`MultipleBlockers = Absent`. A client must not infer support from the schema's
shape — that is what the manifest is for.

### 2.3 Policies — the `PlayerId(0)` fallback

| # | Site | Assumption | Class |
| --- | --- | --- | --- |
| C1 | Nine policies use `view.opponent_life.first().map_or(PlayerId(0), \|(player, _)\| *player)` | Takes "the" opponent as the first entry, and falls back to `PlayerId(0)` — **which in a pod is frequently the policy's own seat**. | **latent bug at >2 seats** |

Files: `boros_tempo.rs:59`, `boros_char_control.rs:64`, `boros_convoke_burn.rs:242`,
`boros_radiance_assault.rs:180`, `boros_token_rally.rs:224`,
`dimir_transmute_convoke.rs:212`, `dimir_transmute_attrition.rs:166`,
`dimir_transmute_helix.rs:219`, `radiance_convoke_assault.rs:143`.

Harmless today (a duel has exactly one opponent, and it is never seat 0 when
the fallback fires) and therefore **not** an engine defect — it is a policy
weakness under the handoff's classification. It must be fixed by threat
selection over identified opponents in M2/M3, not by patching the fallback
constant. No test is added for it here, because a test asserting correct
targeting in a pod would need pod fixtures that do not exist.

### 2.4 Orchestration — fixed arity

| # | Site | Assumption | Class |
| --- | --- | --- | --- |
| D1 | `policies/src/deck_match.rs:260` — `let mut policies: [Box<dyn CodePolicy>; 2]` | Array arity two. | limitation |
| D2 | `policies/src/deck_match.rs:727` — `life: [game.players[0].life, game.players[1].life]` | Indexes seats 0 and 1 directly; panics at one seat, truncates at three. | **limitation** |
| D3 | `run_rav_deck_matchup` dispatches the seven policy hooks with an if/else chain | Not arity-bound, but is the de-facto orchestrator that M4 replaces. | limitation |

Resolved in the protocol schema: `TerminalResult` carries
`winning_seats: Vec<SeatId>` **and** `winning_teams: Vec<TeamId>`, plus
`eliminated_seats` in elimination order. A duel is the degenerate case.

### 2.5 Correct as written — do not "fix" these

| # | Site | Why it is right |
| --- | --- | --- |
| E1 | `engine/src/game.rs:32885` — `if self.players.len() != 2 \|\| self.turn != 1 \|\| self.active_player != PlayerId(0)` | CR 103.8a/103.8c: the starting player skips their first draw **only in a two-player game**. The arity check is the rule. |
| E2 | `engine/src/game.rs:41497` — `next_player` skips `lost` seats | Already living-seat order, not modulo-2. |
| E3 | `engine/src/game.rs:41511` — `remaining_player_count` | Already a count, not a boolean. |
| E4 | `engine/src/game.rs:41539` — `normalize_priority_after_elimination` | Already handles elimination generally. |
| E5 | `engine/src/game.rs:15703` — `winner()` returns `Some` only when exactly one seat remains | Already multiplayer-correct. |
| E6 | `engine/src/game.rs:15729` — `validate_invariants` requires `players.len() >= 2` | A lower bound, not an equality. |

**Conclusion of the audit:** the engine's *turn and priority substrate* is
already seat-general. The two-player assumptions are concentrated in (a) the
shape of `GameView`, (b) combat defender selection, and (c) the policy and
runner layers. That is why this milestone starts with the interface rather than
with the rules.

---

## 3. Invariant → defending test

Every non-negotiable invariant, and what currently defends it. "M4"/"M2" means
the invariant is stated but not yet mechanically defended, because the layer
that could violate it does not exist yet. Those rows are commitments, not
claims.

| Invariant | Defended by |
| --- | --- |
| No live `Game`/`PlayerState` field mutation by policies, UI, transport, or orchestration | Structural: `protocol` has no engine dependency, so it cannot hold a `Game`. Controller-side enforcement lands with the session crate (M4). |
| Every live command uses the ordinary public submission boundary | M4. `GameCommand` adds no action the engine cannot perform (`command.rs`), which is the precondition. |
| No hidden information crosses viewer boundaries | `protocol/tests/redaction.rs::a_seat_never_sees_an_opponents_hidden_zones`, `a_spectator_sees_no_private_information_at_all`, `a_redacted_card_discloses_no_characteristics`, `a_private_event_is_blanked_rather_than_dropped`, `a_judge_only_event_never_reaches_a_seat`, `re_projecting_a_narrow_view_cannot_widen_it`, `redaction_is_idempotent`. Against real games: M4. |
| No stale request can mutate a later state | `protocol/tests/staleness.rs::a_command_answering_a_closed_request_is_refused`, `a_command_observing_an_older_revision_is_refused`, `a_command_observing_a_divergent_state_at_the_same_sequence_is_refused`, `a_command_echoing_the_wrong_decision_is_refused`, `decision_presence_must_match_the_request`. |
| Accepted commands are attributable to seat/controller/request/revision | `CommandEnvelope` and `CommandReceipt` make every field mandatory; `protocol/tests/schema_roundtrip.rs::envelopes_round_trip_and_carry_full_attribution`. |
| Rejected commands produce no partial mutation | `protocol/src/validate.rs::admit` returns a finished result *before* the engine is reachable; `protocol/tests/staleness.rs::rejection_reports_the_current_revision_and_nothing_else`. Engine side: `engine/src/game.rs` unit test `a_rejected_action_leaves_the_public_state_digest_unchanged`. |
| Out-of-turn and duplicate commands fail without mutation | `staleness.rs::a_command_from_the_wrong_seat_is_refused`, `a_command_from_a_seat_that_does_not_exist_is_refused`, `a_duplicate_client_command_id_replays_the_original_receipt`, `a_duplicate_is_detected_before_staleness`. |
| Canonical replay is deterministic | Pre-existing: `Game::canonical_event_log` + digest comparison in `run_rav_deck_matchup_verified`, and the all-card gauntlet. New: `engine/src/game.rs` unit test `public_state_digest_identifies_the_observed_state`. Wire stability: `schema_roundtrip.rs::round_trip` asserts byte-stable re-encoding. |
| Event redaction is deterministic | `redaction.rs::redaction_is_idempotent`, `a_private_event_is_blanked_rather_than_dropped`. |
| Terminal games have complete receipts | Pre-existing engine receipt invariants; `TerminalResult` carries seats, teams, and elimination order, and `truncation_is_not_a_rules_outcome` stops a move-limit stop being reported as a win. |
| Local policy, scripted human, and eventual HTTP clients use one protocol and orchestrator | M4. `ActionRequest::reply` is already the single envelope-assembly path; `staleness.rs::replies_built_from_a_request_always_validate`. |
| Transport code cannot create a second rules or legality implementation | Structural: the absent dependency edge (§1.1). |
| Never claim correctness beyond the tested boundary | `schema_roundtrip.rs::capability_manifest_claims_only_what_is_implemented`. |

---

## 4. Supported boundary

Machine-readable via `ProtocolCapabilities::current()`; the test above pins it.

- **Verified seat counts:** `[2]`. The engine constructs more, but no
  deterministic fixture covers more, so more is not claimed.
- **Formats:** Duel `Supported`. Free-for-all `Modelled`. Commander,
  Two-Headed Giant, booster draft `Absent`.
- **Features:** scoped redaction `Supported`; canonical replay `Supported`;
  chosen attack defender `Modelled`; teams `Modelled`; idempotent
  resubmission `Modelled` (the ledger exists; no orchestrator drives it yet);
  multiple blockers `Absent`; exhaustive legal actions `Absent`.

`Modelled` means the schema can express it and the rules cannot. A command that
depends on a `Modelled` or `Absent` capability must be refused with
`ProtocolError::Unsupported`.

### 4.1 Known gaps recorded rather than hidden

- `LegalActionSurface.exhaustive` is `false` and there is no producer yet. A
  controller must not treat an absent option as illegal.
- `DecisionKindLabel` is an owned string, not a mirror of the engine's ~35
  decision kinds. Rationale is in `request.rs`: the taxonomy grows per card
  mechanic, and the structure a client needs to *answer* a decision is carried
  by the typed candidate lists, which are mirrored exactly. Revisit if a client
  ever needs to dispatch exhaustively on kind.
- `StateRevision.sequence` is owned by the orchestrator, not the engine. The
  engine supplies only `Game::public_state_digest()`, because the public event
  log is resettable for measured scenario suffixes and so cannot serve as a
  monotonic counter.
- `TeamObservation` has no shared-life field. Adding one before 2HG rules exist
  would read as a claim. "Do not approximate 2HG by summing two life totals."

---

## 5. Milestone status

| Milestone | State |
| --- | --- |
| M1 architecture and contracts | **this change** — boundaries documented, two-player inventory complete, protocol types with owned serde representations, schema/version constants, round-trip tests, stale revision/request rejection tests, visibility/redaction tests. No networking. |
| M2 policy v2 | not started |
| M3 stronger policies | not started |
| M4 in-process match orchestrator | not started |
| M5 multiplayer foundation | not started |
| HTTP/WebSocket, Commander, 2HG, draft | gated behind M1–M5 |

---

## 6. Gates

```sh
cd varieties/magic
./scripts/core-check.sh
./scripts/check-batch.sh policies
./scripts/check-batch.sh protocol
./scripts/check-batch.sh protocol clippy
./scripts/check-batch.sh rav 1/48
./scripts/check-batch.sh engine 1/32
cargo clippy -p <changed-package> --all-targets -- -D warnings
```

Do not run a monolithic workspace suite after every edit. The exhaustive
release gate is for deliberate milestones.
