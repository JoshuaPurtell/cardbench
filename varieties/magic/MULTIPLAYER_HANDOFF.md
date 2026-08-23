# Multiplayer handoff — free-for-all, Two-Headed Giant, Commander, and table talk

Written 2026-08-04 against `dev` at `5b546bb0`. Every line citation below was
re-verified at that head, not copied from `ARCHITECTURE_CONTRACT.md`, whose
numbers have drifted.

Read `ARCHITECTURE_CONTRACT.md` §2 first — it is the audit this plan builds on
and it is still accurate in substance. Read `ARENA.md` for how an LLM seat is
measured, because that is how this work gets proved.

---

## 1. What you are building, and what "done" means

Three formats and one communication channel:

| Deliverable | Depends on | Status today |
| --- | --- | --- |
| **Free-for-all** (3+ seats, no teams) | — | `Modelled` |
| **Commander** (FFA + command zone) | free-for-all | `Absent` |
| **Two-Headed Giant** (2v2, shared life) | teams | `Absent` |
| **Public table talk** | nothing | does not exist |

"Done" is not "it runs". This project's standard is that a capability is
claimed only where a deterministic test defends it, enforced by
`protocol/tests/schema_roundtrip.rs::capability_manifest_claims_only_what_is_implemented`.
Flipping a `SupportLevel` without a fixture will fail that test, and it should.

The proof obligation is in §7: each format must produce a **calibrated mirror**
(identical policies in every seat land on the uniform rate, within interval),
and each must be played by both a code policy and a gpt-oss ReAct seat with
`agency` near 100% and `valid=true`.

---

## 2. What already exists — do not rebuild it

The protocol crate was designed for this and is ahead of the engine. Check
before you add a type.

- `SeatId` and `TeamId` are distinct (`protocol/src/ids.rs:39,53`), with the
  comment "a seat is deliberately not a team".
- `MatchObservation` (`observation.rs:168`) is **already seat-indexed**:
  `seats: Vec<SeatObservation>`, `teams: Vec<TeamObservation>`, and separate
  `active_seat` / `priority_seat` / `decision_seat`. There is no own/opponent
  split to undo.
- `AttackDeclaration { attacker, controller, defender: DefenderDto }` is
  **already per-attacker**, and `DefenderDto` admits a seat or a team.
- `TerminalResult` already carries `winning_seats`, `winning_teams`, and
  `eliminated_seats` in elimination order.
- `EventVisibility` (`event.rs`) is already `Public | Seats(set) | Team(TeamId)
  | JudgeOnly`, and `ViewerScope`/`ScopeMembership` (`scope.rs`) already decide
  entitlement per seat. **`Team` visibility exists and nothing emits it yet.**

The engine's turn/priority substrate is likewise already seat-general —
`next_player` skips eliminated seats, `remaining_player_count` is a count,
`normalize_priority_after_elimination` is general, `winner()` requires exactly
one survivor, `validate_invariants` requires `players.len() >= 2` as a lower
bound, and `new_rav_game(player_count)` takes any count.

So this is not a rewrite. It is four specific holes.

---

## 3. The blocking sites, verified at `5b546bb0`

| # | Site | What breaks at >2 seats |
| --- | --- | --- |
| **B1** | `engine/src/game.rs:8105` — `let defending_player = self.next_player(self.active_player);` | The defender is *derived*, never chosen. In a pod every attack silently hits the next living seat in turn order. |
| **B2** | `engine/src/game.rs:785` — `CombatState::defending_player: Option<PlayerId>` | One defender for the whole combat, not one per attacker. |
| **A2** | `engine/src/game.rs:704` — `GameView::opponent_battlefield: Vec<CardView>` | Every opponent's permanents concatenated with no seat partition. Recoverable only through `CardView::controller`. |
| **C1** | 9 policy files use `map_or(PlayerId(0), ...)` on `opponent_life.first()` | **Latent bug.** In a pod the fallback frequently names the policy's *own* seat. |
| **D1/D2** | `policies/src/deck_match.rs:242,264,285` — `[Box<dyn CodePolicy>; 2]`; `:963` — `life: [game.players[0].life, game.players[1].life]` | Arity two throughout. `:963` panics at one seat and silently truncates at three. |

One thing already helps you: `planner/board.rs:660` `primary_opponent()` picks
the **lowest-life** opponent rather than the first, which is a defensible pod
heuristic. `Board.opponents` is already a `Vec<Opponent>` carrying seats.

### Do not "fix" these

`ARCHITECTURE_CONTRACT.md` §2.5 lists six sites that are two-player *by rule*.
The important one: `game.rs` checks `players.len() != 2` before skipping the
starting player's first draw, because CR 103.8a applies that rule **only in a
two-player game**. The arity check *is* the rule. Deleting it to "generalise"
is a rules bug.

---

## 4. Order of work

```
M5a free-for-all ──┬── M5b teams → Two-Headed Giant
                   └── M5c Commander
M5d table talk ────── independent; land it early, it is the cheapest
```

Commander does **not** need teams. 2HG does. Do free-for-all first regardless:
it is the only one that unblocks both, and it is where the latent C1 bug lives.

---

## 5. Format design

### 5.1 Free-for-all (M5a)

The smallest change that makes a pod real.

1. **Per-attacker defender.** Replace B2's single `defending_player` with a
   per-attacker map, and make `PolicyAction::DeclareAttackers` carry
   `Vec<(ObjectId, DefenderChoice)>`. `DefenderDto` already models this on the
   wire; mirror it in the engine rather than inventing a second shape.
   Keep B1's derivation as the **two-seat default** so every existing fixture
   and replay digest is unchanged — this is load-bearing, see §9.
2. **Seat-partitioned view.** Add `GameView::battlefields_by_seat` alongside
   A2. *Add, do not replace.* Every frozen policy generation v1–v8 reads
   `opponent_battlefield`, and rewriting it rewrites every ladder number ever
   measured.
3. **Fix C1 properly.** Threat selection over identified opponents, not a
   patched constant. `primary_opponent()` is the seam; give it a weights-aware
   sibling that scores opponents by clock, board, and life.
4. **Arity-general orchestration.** `run_deck_matchup_with` takes
   `Vec<Box<dyn CodePolicy>>`; `life` becomes a `Vec<i64>`.

**Calibration gate:** a 3-seat mirror of identical policies must land at
**33.3%** per seat within interval, and a 4-seat mirror at 25%. If it does not,
seat order is leaking — which is exactly the class of bug the ladder's seat
split caught in the duel lane.

### 5.2 Two-Headed Giant (M5b)

The hard one, and it is hard for one reason: **the turn is shared**.

- **Shared life.** Life moves from `PlayerState` to a team-level total (30 to
  start). Every damage, gain, and loss path must route through the team.
  Team loses at 0. This is the single most invasive change in this document —
  grep every `players[..].life` write before you start.
- **Shared turn.** Both teammates untap and have their turn together; each
  draws (the team does not share a draw); the *starting team* skips its first
  draw entirely, which is the 2HG analogue of CR 103.8a. `active_player`
  becomes `active_team` with an APNAP ordering of seats inside it.
- **Combat.** Attackers attack the opposing *team*; damage lands on shared
  life. `DefenderDto::Team` already exists.
- **Poison is 15, not 10** — irrelevant for Ravnica, but record it as
  deliberately unimplemented rather than forgotten.
- **Teammates may not see each other's hands.** They coordinate by talking,
  which is §6 and is the point of the whole exercise.

**Calibration gate:** a team mirror (all four seats identical policy) must land
at **50%** per team.

### 5.3 Commander (M5c)

Free-for-all plus four mechanics. None interact with the shared-turn problem,
which is why this is independent of 2HG.

- **Command zone.** A new `Zone` variant, plus a castable-from-command-zone
  path.
- **Commander tax.** +{2} generic per prior cast *of that commander from the
  command zone*. Per-commander counter on the player.
- **Commander damage.** Track combat damage per `(damaged_seat, commander
  object identity)`. 21 eliminates. This needs the **object identity** to
  survive zone changes — `ObjectId` is already stable across zones and
  `ObjectIncarnationAdvanced` already tracks incarnations, so use those rather
  than card definition strings.
- **Zone-change replacement.** On death/exile/bounce, the controller may send
  the commander to the command zone instead. This is a replacement effect, and
  the engine already has a replacement layer.
- **Deck rules.** 100-card singleton, colour identity. Enforce in `rav` deck
  validation, not in the engine.
- **Starting life 40.**

Ravnica alone has no legendary creature that makes a good commander pool —
check `sets/ravnica_city_of_guilds` before designing decks. Guildpact and
Dissension manifests exist but are `in_development`. **Confirm the card pool
can seat four legal commanders before committing to this milestone.**

---

## 6. Public table talk (M5d)

The user-facing goal: seats communicate, publicly, and in 2HG that is how
teammates coordinate at all.

### Contract

```rust
/// A seat-authored message. Never affects rules state.
GameCommand::Say { seat: SeatId, body: String }
```

Five rules, each of which exists to stop a specific failure:

1. **Public only.** `EventVisibility::Public`. No whispers in v1. A private
   channel is a hidden-information surface and this project's redaction tests
   are its crown jewels — do not put a hole in them for a feature nobody asked
   for. (`EventVisibility::Team` exists and is tempting; leave it for later and
   note that 2HG teammates in paper *may* talk privately. v1 is public.)
2. **Rules-neutral.** A `Say` can never be a legal-action prerequisite, can
   never change priority, and can never be required to advance the game. If the
   engine ever blocks waiting for a message, the design is wrong.
3. **Out of the rules digest, inside the transcript.** Messages must **not**
   enter `canonical_event_log`, because the replay digest is how every campaign
   proves determinism and messages would make an LLM run non-replayable.
   They belong in the `session` transcript alongside the timeline. Write this
   down in `ARCHITECTURE_CONTRACT.md` when you land it; it is exactly the kind
   of thing a future reader will assume the other way.
4. **Bounded.** Cap length (256 chars is plenty) and messages per priority
   window (1). An LLM given an unbounded channel will fill it, and every
   message is prompt tokens for every other seat, on every subsequent decision.
5. **Attributable.** Same envelope discipline as every other command — seat,
   revision, client command id — so a duplicate replays rather than repeats.

### How an LLM seat says something

Do **not** add a menu entry. The menu-index contract is what makes an agent
seat incapable of an illegal move (`ARENA.md` §1), and mixing free text into
the choice would reopen the parse surface. Instead extend the reply object:

```json
{"reasoning": "...", "choice": 3, "say": "I have removal for the flier, take the ground"}
```

`say` is optional, ignored when absent, and validated separately from `choice`.
A malformed `say` must not invalidate a valid `choice` — degrade to silence and
count it, the way `parse_failures` is counted today.

Incoming messages render into the prompt as a bounded, most-recent-first
`=== TABLE TALK ===` section. Bound it hard (last 10 messages): the prompt is
re-sent on every consultation and this is the one section that grows without
limit.

---

## 7. How to prove it out

This is the part that makes the work real, and it is why the arena exists.

### 7.1 Code policies first

Write **v9: pod-aware**, as a new frozen generation under
`policies/src/archetypes/` — never by editing v1–v8. It needs exactly two new
behaviours: threat selection over identified opponents (fixing C1 properly),
and choosing a defender per attacker. Everything else inherits.

Score it with the existing ladder in a duel first: **v9 must be a no-op against
v8 at two seats.** If it is not, you changed duel behaviour while adding pod
behaviour, and the pod result will be uninterpretable.

### 7.2 The calibration gates

Run these before believing any result. Each is a mirror of identical policies:

| Format | Seats | Expected per-seat rate |
| --- | --- | --- |
| Duel (regression) | 2 | 50.0% |
| Free-for-all | 3 | 33.3% |
| Free-for-all | 4 | 25.0% |
| Two-Headed Giant | 2v2 | 50.0% per team |

A mirror that misses its uniform rate outside the interval means seat or team
order is leaking. Fix that before measuring anything else. The duel lane learned
this the expensive way: `POLICY_LADDER.md` records a "negative play advantage"
that survived as the project's headline open question until it turned out to be
n=60 noise on a statistic reported without an error bar.

### 7.3 The gpt-oss lane

The arena seats a model as an ordinary `CodePolicy`, so once `run_deck_matchup_with`
is arity-general, `rav-arena` should need only the menu work. Two changes:

- `menu.rs` gains a **"which opponent"** dimension on targeting and attacks.
  This is the same enumeration surface that is currently the ceiling on
  blocking — expect the menu to widen sharply and re-run `rav-arena probe`,
  because `MENU_LIMIT = 24` will start truncating.
- `render.rs` gains per-seat battlefields and the table-talk section.

Use `openai/gpt-oss-120b` and `openai/gpt-oss-20b` — they are cheap
($0.037 and $0.030 per M input), they work today, and there is a duel baseline
to compare against:

| Run (duel, vs v7) | Rate | Agency |
| --- | --- | --- |
| gpt-oss-120b, low effort | 33.3% [18.0, 53.3] | 99.4% |
| gpt-oss-120b, high effort | 41.7% [24.5, 61.2] | 96.2% |
| gpt-oss-20b, low effort | 29.2% [14.9, 49.2] | 100.0% |
| 20b vs 120b head to head | 33.3% / 66.7% | 100% / 98.7% |

**Read `agency` before the rate, every time.** A seat that fell back is the
fallback policy wearing the model's name.

### 7.4 The experiment worth running

Everything above is scaffolding for one measurement:

> **In Two-Headed Giant, do two LLM seats that can talk beat two that cannot?**

Same decks, same seeds, same models, table talk on versus off. It is a clean
A/B with an obvious null hypothesis, it is the only result here that could not
have been obtained in a duel, and it is a genuine capability question rather
than a rules-coverage one.

Predict before you run: the duel work found these models hold ~50% on the play
and ~15% on the draw across three independent measurements, and that more
reasoning did not move it. If coordination is similarly bottlenecked by what the
menu can express rather than by what the model can reason about, messaging will
do nothing — and that would itself be worth knowing.

---

## 8. Gates

```sh
cd varieties/magic
./scripts/core-check.sh                 # every edit
./scripts/check-batch.sh arena test     # agent seat, no network needed
./scripts/check-batch.sh arena probe 3  # re-measure open-decision share
./scripts/check-batch.sh protocol       # capability manifest honesty
cargo clippy -p <changed-package> --all-targets -- -D warnings
```

Add a `check-batch.sh multiplayer` entry with the §7.2 mirrors when M5a lands.

---

## 9. Traps

**Do not edit a frozen policy generation.** v1–v8 are what every ladder number
was measured against. Add v9.

**Do not replace `GameView::opponent_battlefield`.** Add the seat-partitioned
field beside it. The same rule that protects frozen policies protects the view
they read. `ARCHITECTURE_CONTRACT.md` §2.1 already commits to this: "the
engine's `GameView` is unchanged by this milestone".

**Keep the two-seat path byte-identical.** Every campaign in this repo compares
replay digests. If a duel's canonical event log changes because you generalised
combat, you have invalidated the entire measured history — v4's 58.2%, v7's
54.6%, v8's 57.3%. Make the pod path an addition that a two-seat game never
takes, and prove it with an unchanged digest.

**A rejected policy move is never data.** Pod policies will propose illegal
defenders while you are developing. The ladder and arena both report
`rejected_moves` separately for exactly this reason; a cell with any rejection
did not end by play.

**Shared life is the 2HG risk, not the shared turn.** The turn structure is
visible and you will remember it. Life is written from dozens of paths — damage,
drain, payment costs, replacement effects, `entry_life_payment` on lands. Grep
every write before you start, and consider making `PlayerState::life` private
so the compiler finds them for you.

**The capability manifest is a test, not a doc.** Flipping `FreeForAll` to
`Supported` without fixtures fails
`capability_manifest_claims_only_what_is_implemented`. That test is the
project's promise that it never claims correctness beyond its tested boundary.
Let it stop you.

**Messages are prompt tokens.** Every message is re-sent to every seat on every
subsequent consultation. Unbounded table talk will quietly multiply the cost of
a run and push long games into `budget_exhausted`.

**This worktree has concurrent agents.** `dev` is ~2,650 commits ahead of
`origin/main` and entirely unpushed. Check `git status` mtimes before assuming a
dirty file is yours, and never `git add -A` without checking `.gitignore`
coverage — that mistake swept 939 `node_modules` files into a commit here on
2026-08-04.
