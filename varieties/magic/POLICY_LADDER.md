# Policy ladder

How policy strength is improved and measured in this variety.

## The two questions, kept apart

"Is this deck better?" and "is this policy better?" are different questions.
Conflating them is how a search random-walks, so there are two harnesses.

| Question | Harness | What varies | What is held fixed |
| --- | --- | --- | --- |
| Is this policy stronger? | `rav-policy-ladder` | policy generation | deck list (both seats play the same one) |
| Which deck beats which? | `rav-archetype-matrix` | deck list | policy generation (latest for both) |

The ladder is the one that drives development. Both seats play the *same* deck
list and differ only in which generation is piloting, so a win rate is
attributable to the policy change and nothing else — there is no other
asymmetry left in the game.

## Why the harness looks the way it does

**Paired seats.** Each shuffle seed is played twice with the generations
swapped. A shuffle that hands one side a perfect curve hands it to both, so
most of the luck cancels, as does the play/draw advantage.

**Every deck, not one.** A change that helps Boros aggro and quietly hurts
Golgari midrange is not an improvement. Per-deck rates are always reported, and
a significant regression on any single deck blocks the rung regardless of the
aggregate.

**Wilson intervals.** A win rate without error bars is not a measurement. At
100 games a 55% result cannot be distinguished from 50%; resolving a genuine
3-point edge needs a few thousand. `is_improvement()` requires the pooled
interval to *exclude* 50%, so a point estimate on the right side of the line is
never enough on its own.

**Rejected moves invalidate a cell.** A game that ended because the engine
refused a proposal did not end by play. Those cells are reported and counted,
never averaged in as losses.

**Calibration.** With every generation identical, the ladder reports exactly
50.0% on every deck with zero rejections. That is the null result the harness
must produce before any non-null result from it means anything.

## Generations

Each generation is a separate file under `policies/src/archetypes/`, frozen
once measured. A policy improved in place destroys the baseline it would be
compared against, which is precisely what the ladder needs.

Every generation pilots *any* deck; the archetype supplies weights, not code.

| Version | Change | Rationale |
| --- | --- | --- |
| `v1` | Shared planners: mana payment, one-ply combat, target scoring. | Baseline. Replaces card-ID decision trees and the previous generic policy, which could cast only target-free permanents — excluding every instant and sorcery, so all removal and all burn. |
| `v2` | Whole-set attack planning. | v1 judged each attacker against a hypothetically free best blocker. That is wrong whenever attackers outnumber blockers: only as many attackers as there are blockers can be blocked, so a creature that looks bad alone is often free damage in a crowd. |
| `v3` | Valued blocking. | v1 and v2 scored a block purely as material. A 2/5 blocking a 3/3 kills nothing and loses nothing, so the delta is zero and the block is declined — the wall watches three damage go through every turn. Both midrange decks run 2/5 bodies. |
| `v4` | Land sequencing by what a land unlocks. | v1–v3 ranked lands by colour coverage, which prefers exactly the wrong land: a karoo makes two colours and sorts to the top, but enters tapped *and* bounces a land. Scoring a land by the best spell it makes castable this turn subsumes the problem rather than special-casing it. |

### Shared scale

Damage and material are priced on one scale — fractions of a life total,
`LIFE_TOTAL_VALUE` in `planner/combat.rs`. Without that, one of them dominates
by accident: the first version of v2 valued four damage at 0.56 and a 2/2 at
3.6, so it declined every attack that cost a creature no matter how close the
opponent was to dying.

The same scaling makes prevention life-sensitive, which is the behaviour a
constant could not express: at twenty life, eating a 1/1 outright beats
absorbing four damage once; at six life it inverts.

## Measured results

Definitive run, 150 seed pairs per rung for v2/v3 and 60 for v4 (300 and 120
games per deck), paired seats throughout. "Rate" is the challenger's win rate;
50% means the change did nothing.

| Rung | Boros aggro | Selesnya midrange | Golgari midrange | Boros burn | Pooled | Significant? |
| --- | --- | --- | --- | --- | --- | --- |
| v2 vs v1 | 49.7% | 54.9% | 51.0% | 51.0% | **51.6%** [48.8, 54.5] | no |
| v3 vs v2 | 51.3% | **59.4%** [53.7, 65.0] | 51.3% | 49.7% | **52.9%** [50.0, 55.7] | borderline |
| v4 vs v3 | 56.7% | 60.2% | 57.5% | 60.0% | **58.5%** [54.0, 62.9] | **yes** |

Reading these honestly:

- **v2 did essentially nothing.** Whole-set attack planning is correct — the
  unit tests pin a board where v1 declines three free attackers and v2 takes
  them — but the board state it fixes is rarer in these matchups than expected.
  A correct change is not automatically a valuable one, and the ladder is what
  distinguishes the two.
- **v3 helped exactly where it was predicted to.** The +9.4 point gain is on
  Selesnya midrange, the deck built around 2/5 bodies, and its interval
  excludes 50%. The pooled figure is borderline because the other three decks
  have almost no walls and were unaffected. That is the per-deck reporting
  earning its keep: an aggregate alone would have called this "noise".
- **v4 is the first unambiguous rung.** Positive on all four decks, pooled
  interval excludes 50%, no regressions. Land sequencing turns out to be worth
  more than either combat change.

One caveat, reported rather than hidden: v4's better mana development lengthens
the Selesnya midrange mirror from 26 to 44 mean turns, which triggers the open
engine defect `battlefield-attachment-illegal-target-invariant-false-positive`
in roughly 18% of that cell's games. Those games ended by an engine refusal,
not by play. The ladder reports `clean`, pooling only uncontaminated cells,
alongside the raw aggregate; `is_improvement()` still requires every cell to be
clean, so that rung is not claimed as a formal pass.

## Running it

```sh
cd varieties/magic
./scripts/check-batch.sh ladder        # 25 seed pairs, quick signal
./scripts/check-batch.sh ladder 150    # definitive; minutes, not seconds
./scripts/check-batch.sh matrix 50     # deck matchup structure
```

Both use the release profile: a debug build makes these campaigns roughly ten
times slower.

## Known limitations

- **The ladder runs serially.** The deck matrix runner is threaded; this one is
  not. A 150-pair ladder is minutes of wall clock that could be tens of
  seconds.
- **No mulligans.** Every game keeps its opening seven, which flattens the
  difference between decks with different curve sensitivity.
- **No activated abilities.** No generation activates a non-mana ability, so a
  repeatable damage source such as Viashino Fangtail is played as a vanilla
  body. This is the largest single gap remaining.
- **No Convoke.** Cards whose cost the planner cannot reduce are simply not
  cast when their printed cost is unpayable.
- **One open engine defect** touches this measurement:
  `battlefield-attachment-illegal-target-invariant-false-positive` in
  `ENGINE_BUG_LEDGER.md` produces a handful of rejected moves in long Selesnya
  midrange games. It is reported per cell rather than silently absorbed.
