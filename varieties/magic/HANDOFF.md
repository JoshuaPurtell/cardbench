# Magic variety — engineer handoff

Written 2026-08-04. Branch `dev`, worktree clean.

Read this, then `ARCHITECTURE_CONTRACT.md` for boundaries and
`POLICY_LADDER.md` for how policy strength is measured. `ENGINE_BUG_LEDGER.md`
is the defect record and has no OPEN entries.

---

## 1. What exists now

Six crates. `engine`, `policies`, and `sets/ravnica_city_of_guilds` have **zero
external dependencies** and must keep them; `serde` lives only in `protocol` and
`session`. That rule is load-bearing, not stylistic — see §4.

| Crate | Role |
| --- | --- |
| `cardbench-magic-engine` | Rules. Knows nothing of transport, policy, or sessions. |
| `cardbench-magic-rav` | Ravnica card definitions, bindings, decks. |
| `cardbench-magic-policies` | Shared planners, archetype policies v1–v5, campaign runners. |
| `cardbench-magic-protocol` | Versioned owned transport schema. **No engine dependency, deliberately.** |
| `cardbench-magic-session` | Engine→protocol projection, transcripts, critique, campaign statistics. |

### Tooling

```sh
cd varieties/magic
./scripts/check-batch.sh list          # every gate
./scripts/core-check.sh                # ~5-15s, the edit-loop gate
./scripts/check-batch.sh ladder 120    # policy generation N vs N-1
./scripts/check-batch.sh matrix 60     # deck matchup structure
./scripts/check-batch.sh stats 30      # what pilots actually did, per deck

cargo run --release -p cardbench-magic-session --bin rav-match-review -- \
  rav_boros_aggro rav_selesnya_midrange 3 --jsonl match.jsonl
cargo run --release -p cardbench-magic-policies --bin rav-card-pool
```

---

## 2. Where the work stands

**Policy generations.** Each is a frozen file under
`policies/src/archetypes/`. All pilot any deck; the archetype supplies weights,
not code.

| Rung | Clean win rate | Verdict |
| --- | --- | --- |
| v2 vs v1 — whole-set attack planning | 50.6% [46.9, 54.2] | no change |
| v3 vs v2 — valued blocking | 51.0% [47.3, 54.6] | not established |
| v4 vs v3 — land sequencing | **58.2% [54.6, 61.7]** | **the only real gain** |
| v5 vs v4 — activated abilities | 49.1% [44.6, 53.5] | no change |

**Deck matchups** under v5, every mirror calibrating at exactly 50.0%:
Selesnya midrange 65.3%, Golgari 58.9%, Boros aggro 44.2%, Boros burn 31.7%.

**Engine defects found and fixed** this stretch, each red-green with separate
commits: a departed player's aura breaking the historical-receipt audit; missing
live declaration facts on `CardView`; and a survivor's aura orphaned by CR
800.4a departure. The last one was the important one — it was refusing 51 of 240
games in one ladder cell, and fixing it took the whole ladder to
`invalid_rungs=0` for the first time.

---

## 3. What I would do next, in order

### a. Nothing in the current card pool is a big policy win

This is the main thing to absorb before spending effort. Four hypotheses were
tested and three failed, each for a different reason:

- **Mana development** looked broken over six games. Over 1,000+ turns it was
  variance in the *opposite* direction.
- **Activated abilities** looked like the largest gap: 9 of 38 distinct cards
  carry one. Worth exactly nothing — three cost a sacrifice, the rest sit on
  creatures that rarely reach the board. `abils` is 0.00 across 300 games.
- **Aggro never blocking** looked like a weight miscalibration. The arithmetic
  shows the weight flips no decision; aggro blocks rarely because its creatures
  are small, which is correct play.

Only v4's land sequencing moved the needle. My honest read: the shared planners
now play this pool close to the ceiling that one-ply reasoning allows, and the
next real gain needs either **lookahead** or **a bigger card pool** (Guildpact
adds Izzet, Orzhov, Gruul and far more ability-dense cards).

### b. The unexplained result worth chasing

**Every mirror shows a negative play advantage** — the seat on the play wins
34–50% where real Magic predicts ~53%. Two candidate mechanisms, unseparated:
the skipped first draw in long games (CR 103.8a, correctly implemented), and
one-ply policies favouring the reactive seat. Separating them would say
something real about either the engine or the whole policy family. It is the
most interesting open question here.

### c. Cheap, concrete, unblocked

- **Parallelise the ladder.** The matrix runner is threaded; the ladder is not.
  A 120-pair ladder is ~8 minutes that should be under one.
- **Convoke.** 15 cards in the set have it; the planner cannot reduce a cost, so
  Siege Wurm and friends are simply never cast.
- **Sacrifice-cost abilities.** The three v5 skips need explicit payment
  selection through `ActivateAbilityWithGeneralizedCosts`.
- **Mulligans.** No generation mulligans; every game keeps its seven.

### d. Milestones 2–5 of the original handoff

M1 (protocol and contracts) is done. M4's projection half has landed in
`session`. Still open: the policy v2 ABI over `ActionRequest`, controllers, the
orchestration loop, and multiplayer. Nothing above depends on them.

---

## 4. Things that will bite you

**Do not add serde to `engine`, `policies`, or `rav`.** The zero-dependency
posture is why `protocol` has no engine dependency, which is in turn what makes
"transport cannot grow a second rules implementation" a compiler guarantee
rather than a code-review promise. Project, don't derive — `session/src/project.rs`
is the pattern.

**Do not change the four constructed decks.** Every ladder number is against
them. Deck changes are a separate axis and invalidate the baseline.

**Freeze a generation once measured.** Editing v4 in place destroys the baseline
v5 is compared against. New generation, new file.

**A rejected policy move is never data.** It means the game ended by an engine
refusal rather than by play. The ladder reports contaminated cells separately
and `is_improvement()` demands every cell be clean.

**Watch out for substring matching on event kinds.** I reported "43 ability
activations, v5 works" from `grep -c AbilityActivated`, which also matches
`ManaAbilityActivated`. The true count was zero. The transcript carries a typed
`kind` field precisely so nobody needs substring matching — use it. The tell was
the ladder returning byte-identical numbers across a supposed behaviour change.

**Object identities must be captured at setup, not at the end.** CR 800.4a
removes a departing player's objects, so an end-of-match scan silently loses
every card the loser owned — half the seats in every decisive game. This is
already fixed and a fidelity test guards it, but the same trap applies anywhere
you resolve an id after a game ends.

**Mean-of-ratios is not ratio-of-means.** Creature conversion read 0.36 when the
true figure was 0.95, because seats with a zero denominator contributed zeros.

---

## 5. Measurement discipline that earned its keep

Three checks caught real errors and are worth preserving:

1. **Mirror calibration.** A deck against itself must be exactly 50%. The first
   run reported 100% and exposed a real attribution bug — in a mirror both deck
   ids are the same string, so wins must be attributed by *seat*.
2. **Identical numbers are a red flag, not a result.** Twice, a byte-identical
   ladder meant the change was a no-op, not that it was neutral.
3. **Wilson intervals, per-deck.** At 100 games a 55% rate cannot be told from
   50%. Per-deck reporting is what made v3's single-deck effect visible where an
   aggregate would have read as noise — and equally what stopped v6 being
   claimed on a correlation.

The general lesson, stated plainly because it cost the most time: **static card
counts and single games both generated confident, wrong leads.** Only
`check-batch.sh stats` over hundreds of games produced a hypothesis worth
testing, and even that one turned out to have its causation backwards. Count
first.
