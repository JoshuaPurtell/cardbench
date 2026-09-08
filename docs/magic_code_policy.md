# `cardbench/magic/code_policy` — reference freeze, sealed split, and sweep

Date: 2026-08-23. Branch: `integration/pokemon-magic`.

Implements §2.3, §3.1, §3.2 and §3.3 of `docs/CODE_POLICY_DEO_DESIGN.md` for the
Magic variety, against the validity standard in
`evals/docs/CODE_POLICY_TASK_STANDARD.md`.

Out of scope here, and still owed by someone: the Harbor bundle, the Docker
image, and compiling an untrusted candidate `policy.rs` in a sandbox. Until that
exists a "candidate" is a `(deck, pilot generation)` pair named on the command
line, not agent-authored Rust.

---

## 1. What was missing

Magic had a real ladder — mirror decks, paired seats, Wilson intervals,
per-deck reporting, eight documented policy generations — and **no held-out
concept at all**. `grep -r heldout` across `policies`, `arena`, `engine` and
`session` returned nothing. Every number the variety could produce was a train
number.

It also had no frozen origin. `pilot_for` seats `PolicyVersion::latest()`, so the
thing a new policy was implicitly compared against moved every time a generation
was added.

Three pieces close that.

---

## 2. `reference_v1` — the frozen origin

`policies/src/code_policy/reference.rs`.

```text
reference_v1 := archetypes::v5     (frozen, versioned, never edited in place)
```

v5 is the generation that carries v2's whole-set attack planning, v3's valued
blocking, v4's land sequencing, and adds activated abilities — six of the nine
stack-using activated abilities in the constructed card pool, with the three
sacrifice-cost ones reported unsupported rather than silently skipped.

**Why v5 and not `latest()`.** A reference that tracks the newest generation is
not a reference. v6, v7 and v8 already exist in the tree; freezing at v5 makes
them candidates, which is what `ladder.rs` was written to measure, and it means
a v9 does not silently redefine what every earlier delta was measured against.

**Two guards, both tests:**

| guard | what it catches |
| --- | --- |
| `reference_v1_is_the_v5_generation` | the alias being repointed; a v5 policy id being renamed |
| `reference_v1_does_not_track_latest` | someone "helpfully" wiring the reference to `latest()` |
| `reference_v1_play_is_frozen_to_a_recorded_transcript` | v5, **or any shared planner v5 routes through**, changing behaviour |

The third is the one that makes this a freeze rather than a naming convention.
It replays a fixed mirror at a fixed seed and asserts the engine's event digest
is still `fnv1a64:5df7b930a827f3c3`. If that fails after an intentional change,
the correct response is to publish `reference_v2` and re-measure — **not** to
update the constant, because every previously recorded delta was computed
against the old transcript.

---

## 3. Roster and sealed held-out split

`rosters/code_policy_v1.json` (committed) and `.sealed/code_policy/` (gitignored).

```text
train    5 opponents, visible.  v1..v5 generations on the constructed control decks.
heldout  5 opponents, sealed.   Composition published only as counts and a sha256.
```

Structurally the same shape Pokémon uses (`varieties/pokemon/rosters/code_policy_v1.json`):
a committed roster carrying the train surface in full and the held-out surface as
`{manifest, sha256, opponent_count, seeds, cell_count}`, plus a build script that
regenerates the sealed manifest from a hand-authored input.

**Opponents are `(archetype pilot generation, deck fixture)` pairs.** The 15
hand-written per-deck policies in `reference_decks.toml` were deliberately *not*
used: those fixtures run 44 to 52 lands in a sixty-card list across four to seven
distinct cards. They exist to exercise the rules engine. An opponent surface
built from them would measure who can beat a deck that cannot function.

**Disjoint by construction.** Train seats generations v1..v5 on the four
constructed decks; held-out seats v6..v8 on the six hillclimb decks. No pilot
generation and no deck fixture appears in both. `build_sealed_heldout.py`
enforces this, and also refuses to seat the frozen reference as an opponent —
that would guarantee a `0.0` delta cell and dilute the reward toward zero.

**Fail-closed on the pin.** The sweep recomputes the manifest's sha256 before it
will score a held-out run, using a dependency-free SHA-256
(`code_policy/sha256.rs`, verified against the NIST vectors). Verified live:

```
$ ./rav-code-policy-sweep --split heldout ...        # after editing the manifest
cannot resolve the heldout surface: sealed heldout manifest sha256 4545c556… does
not match the committed roster pin 10d654d1…; the heldout surface moved and no
score from it is comparable
exit=1
```

Exit `1`, not a zero score — infrastructure failure and candidate failure are
different facts (standard §2.8).

`build_sealed_heldout.py --verify` **cross-checks the Rust digest against
Python's `hashlib`** by shelling out to the sweep binary, rather than assuming
the two canonical JSON forms agree. They agree because `serde_json::Value` orders
object keys and the compact writer emits Python's `separators=(",", ":")`; the
manifest is required to be pure ASCII because Python escapes non-ASCII by default
and `serde_json` does not.

### What sealing does and does not cover

Both halves of every held-out pair — the generation and the deck fixture — are
files in this public repository. **Their source is not secret.** The seal covers
the *composition*: which pilot flies which deck, in what order, over which seats
and seeds. This is the same honesty Pokémon's `.sealed/README.md` records about
its own opponent pool, and receipts must not overclaim it.

The residual risk is real and worth naming: an agent can enumerate the candidate
*set* the split was drawn from even though it cannot read the draw. Narrowing
that further means stripping the hillclimb index and the newer generations from
the workspace copy at bake time, which belongs to the Harbor bundle.

---

## 4. The sweep

`policies/src/code_policy/sweep.rs`, driven by `rav-code-policy-sweep`.

```text
cell     = (candidate deck, candidate pilot, opponent, opponent deck, seat, seed)
pair key = (opponent, opponent deck, seat, seed)
reward   = mean over opponents of (candidate win rate - reference win rate)
gate     = paired-bootstrap lower bound > 0  AND  no per-opponent regression
```

**Two identifiers, because this is a deck-*and*-pilot task.** The candidate
submits its own deck, so the candidate deck is not a shared axis: the reference
arm plays the roster's fixed `reference.deck`, the candidate arm plays whatever
it brought. Coverage is checked on the full cell id per arm; the paired
comparison joins on the shared coordinate. Joining on the full id would pair
nothing.

That asymmetry is the point of the DEO framing. A candidate that wins by picking
a better deck has genuinely won — a deck is only good with a pilot that can play
it, and grading either alone measures the wrong thing.

**Reused, not rebuilt** (`ladder.rs`, `matchup.rs`):

* paired seats — every `(opponent, deck, seed)` played from both seats;
* `Interval::wilson` on every reported rate;
* per-opponent reporting with an explicit regression list, so "helps Boros,
  hurts Golgari" cannot hide inside a mean;
* contaminated cells counted and surfaced rather than averaged in.

**Fail-closed coverage.** A missing cell, an extra cell, an unpaired cell, or a
stall rate above 25% on either arm ⇒ `scored=false` and `reward=0.0`. Not the
mean of the cells that ran: the cells that fail are not missing at random, they
are the hard ones.

**Deterministic gate.** The bootstrap uses a fixed xorshift seed, so two runs of
the same data give the same verdict.

**No `--seeds` flag.** Seed count is part of the pinned surface. A caller who
could widen it could buy significance with compute, and two runs would not be
comparable.

---

## 5. Measured: the frozen reference

`reference_v1` = v5 on `rav_selesnya_midrange`, 30 seeds × 2 seats × 5 opponents
= 300 cells per arm, `DeckMatchConfig::default()`.

### Train (visible)

| opponent | pilot | deck | reference win rate | 95% Wilson | n |
| --- | --- | --- | ---: | --- | ---: |
| `opp_t1` | v1 | `rav_boros_aggro` | **78.3%** | [66.4, 86.9] | 60 |
| `opp_t2` | v2 | `rav_golgari_midrange` | **65.0%** | [52.4, 75.8] | 60 |
| `opp_t3` | v3 | `rav_boros_burn` | **76.7%** | [64.6, 85.6] | 60 |
| `opp_t4` | v4 | `rav_selesnya_midrange` | **50.0%** | [37.7, 62.3] | 60 |
| `opp_t5` | v5 | `rav_boros_aggro` | **77.6%** | [65.3, 86.4] | 58 |
| **overall** | | | **69.5%** | | 298 |

`opp_t4` is the true mirror — same deck, one generation back — and lands on 50.0%
exactly, which is the sanity check the whole apparatus rests on. `opp_t5` seats
the *same generation* as the reference on a different deck and loses 77.6/22.4:
that gap is pure deck strength, and it is the DEO signal the task is built to
reward.

### Held-out (sealed)

| opponent | reference win rate | 95% Wilson | n |
| --- | ---: | --- | ---: |
| `opp_h1` | 70.6% | [57.0, 81.3] | 51 |
| `opp_h2` | 65.0% | [52.4, 75.8] | 60 |
| `opp_h3` | **23.3%** | [14.4, 35.4] | 60 |
| `opp_h4` | 86.7% | [75.8, 93.1] | 60 |
| `opp_h5` | 70.0% | [57.5, 80.1] | 60 |
| **overall** | **62.9%** | | 291 |

Against standard §2.6 and design §2.1 — "beats the weak decisively, loses to the
strong ones" — the surface behaves: the reference wins 86.7% against the weakest
held-out opponent and **loses 23.3/76.7** against the strongest. The ceiling and
the floor are both visible.

### The benchmark discriminates

v8 on the same deck, scored against the frozen reference over the train split:

```
reward = +2.0pt   ci [-0.7, +4.7]   scored=true   passes=false
```

Non-degenerate (the number moves) and not trivially passable (three generations
of improvement do not clear a 95% gate at 300 cells). Per opponent the candidate
is +3.3, −6.7, +3.3, +10.0, +0.0 — an aggregate gain containing a per-opponent
loss, which is exactly the pattern the regression condition exists to refuse.

**The deck axis is load-bearing.** The same newest-generation pilot on a weaker
deck, train split:

```
candidate = v8 on rav_boros_aggro
reward = -19.5pt   ci [-25.7, -12.3]   scored=true   passes=false
regressions: opp_t1 -25.0pt, opp_t4 -26.7pt, opp_t5 -29.3pt
```

Three generations of pilot improvement do not come close to covering the deck
gap. That is the DEO claim made measurable: a strong pilot cannot rescue a weaker
deck, so grading the pilot alone would have measured the wrong thing.

---

## 6. Defect found in existing Magic code

Not introduced here; surfaced by the sweep's rejection accounting.

```
PolicyMoveRejected { policy: "rav.archetype-burn.v6", kind: ActivateAbility,
                     error: "this permanent's nonmana activated abilities are suppressed" }
```

`planner::ability::best_activation` — the entry point v5 and v6 use — does not
consult `nonmana_activated_abilities_suppressed`. A suppression-respecting
variant exists (`best_activation_respecting_suppression`) and v7/v8 use it; v5
and v6 were left wired to the blind one, with a comment saying older generations
must not acquire a hidden dependency on a newer view field.

That freeze decision is defensible. Its cost is not zero, and the sweep now
measures it: when the engine suppresses an ability, a v5 or v6 pilot proposes an
illegal activation, the engine refuses, and the match **terminates without a
winner**. Observed:

* held-out `opp_h1` (v6 burn): **9 of 60 cells** lost, `n` 60 → 51
* train `opp_t5`: **2 of 60 cells** lost, `n` 60 → 58

In both cases the refused proposal came from the *opponent's* pilot
(`rav.archetype-burn.v6`, `rav.archetype-aggro.v5`), not the reference's — but
`reference_v1` is the same generation on the same code path, so it is exposed
identically on any deck whose abilities can be suppressed. Both cells are
reported as `contaminated` with the rejection reason, and both runs stayed well
under the 25% stall ceiling, so they still scored. This is a live upper bound on
how many cells any measurement involving a v5 or v6 pilot can lose. Two ways
forward, neither taken here because both change frozen behaviour:

1. teach the older generations to check suppression — which moves the freeze
   digest and requires a `reference_v2`;
2. treat a suppressed-ability rejection as a skip rather than a termination in
   the match runner — which changes `deck_match.rs`, outside this scope.

---

## 7. Running it

```bash
cd ~/GitHub/cardbench/varieties/magic

# regenerate + verify the sealed split (needs .sealed/code_policy/inputs/)
python3 scripts/build_sealed_heldout.py --verify

cargo test -p cardbench-magic-policies --lib code_policy::

cargo build --release -p cardbench-magic-policies --bin rav-code-policy-sweep

# feedback
./target/release/rav-code-policy-sweep --split train \
    --candidate-deck rav_selesnya_midrange --candidate-pilot v8 --json out.json

# authority
./target/release/rav-code-policy-sweep --split heldout \
    --candidate-deck rav_selesnya_midrange --candidate-pilot v8 --json out.json
```

Roughly 3 minutes per sweep (600 games, single-threaded).

---

## 8. Standard checklist — what this covers and what it does not

| standard §4 item | state |
| --- | --- |
| Held-out scenarios the agent cannot read | **partial** — sealed by composition, not by source; see §3 |
| Train and held-out disjoint by construction | **yes** — enforced by the build script |
| Ranking on train, reported score from held-out | **yes** — `--split` selects, held-out is the authority |
| Scenarios distinct, not one scenario resampled | **yes** — 5 opponents × 2 seats × 30 shuffle seeds, distinct decks |
| True reward non-degenerate for the reference | **yes** — reference spans 23.3%–86.7% across opponents |
| Baseline genuinely competent, no memorised lines | **yes** — v5 routes through shared planners; contains no card ids and no deck/opponent branch |
| Benchmark discriminates | **yes** — v5 vs v8 differ and the gate does not fire on the difference |
| Reward is a delta, not an absolute | **yes** |
| Fail-closed coverage | **yes** — tested |
| Infrastructure failure is never a score | **yes** — exit 1, `scored=false` |
| Per-episode OS sandbox receipt | **NOT DONE** — no sandbox; "Rust-in-sandbox" is unowned |
| Graded code not the agent's to edit | **NOT DONE** — needs the Harbor bundle |
| Harbor packaging, manifest, image pins | **NOT DONE** — out of scope |
