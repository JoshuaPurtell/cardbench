# Magic variety — engineer handoff

Updated 2026-08-04. Branch `dev`, worktree clean, nothing pushed.

Read this, then `POLICY_LADDER.md` for how policy strength is measured,
`ARCHITECTURE_CONTRACT.md` for crate boundaries, and `ENGINE_BUG_LEDGER.md` for
the defect record.

---

## 1. What this is for

**The goal is a family of Magic policies strong enough that games between them
are productive gameplay, on Ravnica: City of Guilds constructed 60-card decks.**

Two words carry weight.

**Strong** — not "legal". A policy that passes priority correctly and never
blocks produces games that tell you nothing. The policies exist to be a *bar*:
something a new policy can be measured against, and something an opponent can be
asked to beat.

**Productive** — the games have to be worth watching. A game that ends on turn 4
to an unanswered curve, or grinds to turn 50 because neither side can convert a
board, exercises the rules engine without exercising judgement. The interesting
region is games decided by decisions: what to trade, when to spend an answer,
whether to race. Turn count is the cheapest proxy and it is on the dashboard.

This makes two things downstream possible, and they are the point:

1. **Ask a model to write a better policy.** Everything a candidate needs is a
   fixed contract: implement `CodePolicy`, add a `PolicyVersion`, and score it
   with `rav-policy-ladder N <candidate> <baseline>`. Same decks, paired seeds,
   Wilson intervals, per-deck regressions, contamination reported separately.
   The bar is not opinion, and it is not the author's.
2. **Have a model play against the policies.** The same `CodePolicy` seat takes
   an external agent, so the ladder becomes a benchmark rather than a
   development tool. That is why the policies must be strong: a weak baseline
   measures nothing about whatever is beating it.

Both depend on the harness being honest about what it can and cannot see, which
is most of what §3 is about.

---

## 2. What exists now

Six crates. `engine`, `policies`, and `sets/ravnica_city_of_guilds` have **zero
external dependencies** and must keep them; `serde` lives only in `protocol` and
`session`. That rule is load-bearing — see §5.

| Crate | Role |
| --- | --- |
| `cardbench-magic-engine` | Rules. Knows nothing of transport, policy, or sessions. |
| `cardbench-magic-rav` | Ravnica card definitions, bindings, decks. |
| `cardbench-magic-policies` | Shared planners, archetype policies v1–v7, campaign runners. |
| `cardbench-magic-protocol` | Versioned owned transport schema. **No engine dependency, deliberately.** |
| `cardbench-magic-session` | Engine→protocol projection, transcripts, critique, campaign statistics. |

```sh
cd varieties/magic
./scripts/core-check.sh                     # ~6s, the edit-loop gate
./scripts/check-batch.sh list               # every gate
./scripts/check-batch.sh matrix 60          # deck matchup structure
./scripts/check-batch.sh stats 30           # what pilots actually did, per deck

rav-policy-ladder 60                        # every successive rung
rav-policy-ladder 60 v7 v5                  # one named pair  <- new
rav-match-review DECK_A DECK_B SEED --jsonl out.jsonl
```

### Policy generations

Each is a frozen file under `policies/src/archetypes/`. All pilot any deck; the
archetype supplies weights, not code.

| Rung | Clean win rate | Verdict |
| --- | --- | --- |
| v2 vs v1 — whole-set attack planning | 50.6% [46.9, 54.2] | no change |
| v3 vs v2 — valued blocking | 51.0% [47.3, 54.6] | not established |
| v4 vs v3 — land sequencing | **58.2% [54.6, 61.7]** | improvement |
| v5 vs v4 — activated abilities | 49.1% [44.6, 53.5] | no change |
| v6 vs v5 — instant timing | 52.7% [48.2, 57.1] | not established |
| v7 vs v6 — timing split by target | 52.4% [47.9, 56.8] | not established |
| **v7 vs v5 — both timing changes** | **54.6% [50.1, 59.0]** | **improvement** |

Two formal passes in the project's history: v4, and now v7-over-v5. The second
is marginal (lower bound 50.1%) and should be confirmed at 120 pairs. Nearly all
of it is the burn deck at 66.7% [57.8, 74.5].

---

## 3. The two measurement repairs, and why they matter more than the rungs

### The ladder could not see seat-dependent play

`run_step` plays each seed twice with the generations swapped across seats. That
is what makes a rate attributable — and it cancels anything seat-dependent
*exactly*. A change that wins on the play and loses on the draw pools to 50.0%
and is indistinguishable from a no-op.

Every verdict now also reports the challenger's rate by seat, with an interval
on the difference. No extra games; the split was already being computed and
discarded.

It fired on the first rung it was pointed at. v7 vs v6, Boros burn:

```
rate=50.4% ci=[41.6,59.2] n=119
seats on_play=68.3% on_draw=32.2% asymmetry=+36.1pt ci=[+19.4,+52.9] established=true
```

Read the pooled number and the change did nothing. It did a great deal, in
opposite directions by seat. **Three of the four earlier rungs were verdicted
"no change" by an instrument with this blind spot.** Re-running v2, v3 and v5
with the split is cheap and is the first thing I would do.

### The play advantage was noise, and is retracted

`POLICY_LADDER.md` called the negative play advantage "the clearest open
question the harness has surfaced". At 60 games per mirror it reproduces and
looks enormous — the aggro mirror measured **-60.0pt**. At 180 games per mirror
three of four cells change sign and the mean lands near +4pt, which is roughly
what Magic predicts.

`play_edge` was the only statistic in the harness reported without an error bar.
It now carries one, and within a mirror it reduces exactly to `2p - 1` on the
seat-on-the-play rate, so the bound is exact rather than an approximation of a
difference of dependent proportions.

The lesson generalises the one already in §5 of the old handoff: **a number
without an error bar is not a result, including one derived from two numbers
that had them.**

---

## 4. Where policy strength actually is

### Measured, fixed: instants were cast at the worst legal moment

Across eight traced games, 141 of 143 spells were cast on the caster's own turn,
18 of those in the **upkeep or draw step** — before the caster had even drawn —
and not one at an opponent's end step or in any combat step. Every piece of
instant-speed interaction these decks own (Putrefy, Char, Lightning Helix) was
spent with nothing to respond to and no information gained.

v6 holds instants for a window worth something. v7 splits that by target: reach
loses nothing by waiting, while removal held to the end step has already conceded
the creature one attack. Together, +4.6 points.

### Found, not yet exploited: the attack planner models the wrong opponent

`simulate_defence` prices a block as pure material — attacker's value if it dies
minus blocker's if it does. That is the **v1** blocking model. v3 replaced it in
the *real* defender precisely because it is wrong: a 2/4 in front of a 3/3 kills
nothing and loses nothing, scores zero, and declines, so three damage goes
through every turn.

Since v3, then, the attack planner has been predicting a defender the codebase
itself stopped using — expecting free damage from attackers a valued defender
will in fact block. `simulate_defence_valued` now exists alongside it and is used
by the new damage projection, but **no generation uses it for attack planning
yet.** That is v8, it is a handful of lines, and it is the strongest lead I know
of. It is also exactly the kind of change the seat split was built to judge,
because it changes what the attacking seat does.

### Still open, ranked

1. **v8: attack planning against the valued defender.** Above.
2. **Re-measure v2/v3/v5 with the seat split.** Cheap; may reclassify three
   "no change" verdicts.
3. **Turn-level mana allocation.** `spell_to_cast` greedily takes the single
   best spell per priority window, so with four mana it takes the best four-drop
   even when two two-drops are strictly better. Rough measurement put land-mana
   utilisation at 38% for aggro and 41% for Selesnya — heavily caveated (an
   empty hand makes unused mana correct), which is why the metric should be
   built properly first: `SeatStats.mana_produced` promises a comparison against
   spend that is never computed.
4. **Confirm v7 over v5 at 120 pairs.** The pass is marginal.
5. **Give `Board` the stack's contents.** v6/v7 deliberately do not respond to
   anything, because `Board` exposes only `stack_depth` and acting on depth
   alone spends removal at random. A real response rule needs to see what is on
   the stack.
6. **Parallelise the ladder.** The matrix runner is threaded; this one is not. A
   60-pair named-pair run is several minutes.

### Productive gameplay, as its own axis

Strength and watchability are not the same number, and only one of them is
currently measured. The Selesnya mirror runs **43.8 mean turns**, and the
Selesnya ladder cell 50.3. Those games are not being decided by judgement; they
are two pilots unable to convert. Aggro-vs-burn at 13.7 turns is much closer to
the interesting region.

Nothing gates on this yet. A turn-count band — call it 8 to 25 — reported
alongside win rate would make it visible, and a policy change that improves win
rate while pushing the Selesnya mirror to turn 60 should not read as unqualified
progress.

---

## 5. Things that will bite you

**Do not add serde to `engine`, `policies`, or `rav`.** The zero-dependency
posture is why `protocol` has no engine dependency, which is what makes
"transport cannot grow a second rules implementation" a compiler guarantee
rather than a code-review promise. Project, don't derive — `session/src/project.rs`
is the pattern.

**Do not edit a shared planner in place if a frozen generation calls it.** This
is the trap that nearly caught the defence-model fix: `simulate_defence` is
called by every measured generation, so correcting it would silently rewrite the
baseline every ladder number was measured against. Add the corrected function
alongside it and let the new generation opt in.

**Do not change the four constructed decks,** and freeze a generation once
measured. Same reason.

**A rejected policy move is never data.** It means the game ended by an engine
refusal rather than by play. The ladder reports contaminated cells separately.
Note the scope: the *ladder* is clean at `invalid_rungs=0`, but the deck matrix
still reports `rejected_policy_moves=16, failure_count=1` at 90 pairs,
concentrated in the two Selesnya non-mirror cells. "No open defects" is a claim
about the mirror lane only.

**Watch out for substring matching on typed data.** "43 ability activations" once
came from `grep -c AbilityActivated`, which also matches `ManaAbilityActivated`;
the true count was zero. The same pattern is still live in production:
`board.rs:role_for` classifies cards with `format!("{effect:?}")` and
`names.contains("Destroy")`. One enum rename and a card silently becomes
`Role::Other`.

**Object identities must be captured at setup, not at the end.** CR 800.4a
removes a departing player's objects, so an end-of-match scan silently loses
every card the loser owned.

**Mean-of-ratios is not ratio-of-means.** Creature conversion read 0.36 when the
true figure was 0.95.

---

## 6. If you are wiring up an external agent

The seat contract is `CodePolicy`: `propose_move(&GameView) -> PolicyAction` and
`propose_pending_decision`. `seat_policy()` in `archetypes/mod.rs` is where a
version becomes a seat, and `run_versioned_matchup` seats two of them on the same
deck. An agent-backed policy slots in at exactly that point.

Three things to know before trusting a number that comes out of it:

- **Score against a named baseline**, not the newest thing: `rav-policy-ladder N
  <candidate> v7`. What "latest" means changes.
- **Read the seat split, not only the pooled rate.** A candidate that is 50.0%
  paired may be a large effect in both directions.
- **A candidate that produces refused proposals has not been measured.** Those
  games ended by an engine refusal rather than by play, and the ladder reports
  them separately for exactly that reason.

The card pool is larger than the current decks suggest — 291 implemented cards
against the 38 distinct cards the four constructed decks use, plus 15 more
constructed decks already written and sitting on the legacy per-deck-policy lane.
Widening the field is a deck-axis question and does not block any of §4.
