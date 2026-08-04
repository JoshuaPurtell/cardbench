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
| `v5` | Activated abilities (tap- and mana-cost only). | Nine of thirty-eight distinct cards carry a stack-using activated ability and no earlier generation activated any. Measured at no change: the opportunity is much smaller in play than in the card list. Sacrifice-cost abilities are reported unsupported rather than silently skipped. |

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

Definitive run: 120 seed pairs per deck per rung, 240 games per cell, ~950
decisive games per rung, paired seats throughout. "Rate" is the challenger's
win rate; 50% means the change did nothing.

`clean` pools only the cells that measured without engine-refused proposals.
It is the number to trust, and it is not always the flattering one.

| Rung | Aggro | Selesnya | Golgari | Burn | Pooled | Clean | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- |
| v2 vs v1 | 49.6% | 56.0% | 51.2% | 50.8% | 51.9% [48.7, 55.0] | 50.6% [46.9, 54.2] | no change |
| v3 vs v2 | 50.4% | 60.0% | 52.9% | 49.6% | 53.2% [50.0, 56.3] | 51.0% [47.3, 54.6] | not established |
| v4 vs v3 | 57.1% | 60.8% | 57.5% | 60.0% | 58.7% [55.5, 61.9] | **58.2% [54.6, 61.7]** | **improvement** |
| v5 vs v4 | 47.5% | 50.0% | 50.0% | 48.7% | 49.1% [44.6, 53.5] | 49.1% [44.6, 53.5] | no change |

v5 vs v4 was measured at 60 seed pairs (120 games per cell); the earlier rungs
used 120 pairs (240 per cell).

Reading these honestly:

- **v2 did nothing measurable.** Whole-set attack planning is provably correct
  — a unit test pins a board where v1 declines three free attackers and v2
  takes them — and worth 0.6 points, well inside noise. A correct change is not
  automatically a valuable one. This is the entire reason the ladder exists.

- **v3 is not established.** Its only material gain is +10 points on Selesnya
  midrange, which is exactly the deck whose cell is contaminated by the open
  engine defect; on the three clean decks it is 51.0%, indistinguishable from
  no change. The valued-blocking argument is sound and the direction is right,
  but the evidence sits in the one cell that cannot currently be trusted. It is
  reported as suggestive, not as a win.

- **v4 is the one real rung.** 58.2% on clean cells with the interval excluding
  50%, positive on all four decks, no regressions. Land sequencing — the
  cheapest of the three changes — is worth more than both combat changes
  together.

- **v5 changed nothing measurable, and the reason is the interesting part.**
  Activated abilities looked like the largest remaining gap: 9 of the 38
  distinct cards across the four decks carry one, and no earlier generation
  ever activated any. The planner works, but the opportunity is far smaller
  than the card count implies. Three of the nine cost a sacrifice and are out
  of scope by design. Of the rest, Loxodon Hierarch is correctly skipped,
  Selesnya Sagittars is a 2/5 that is almost always tapped or summoning sick
  when the window opens, Ordruun Commando's damage shield has no value function
  yet and scores zero, and Viashino Fangtail — a five-mana 3/3 in a burn deck —
  rarely reaches play at all. Across a whole match the planner finds nothing
  worth doing.

  Worth recording how this was nearly mis-reported: an initial check counted
  "43 ability activations" and concluded v5 was working. That count came from
  `grep -c AbilityActivated`, which also matches `ManaAbilityActivated` and
  `BoundManaAbilityActivated`. The true count of real activations is zero, and
  the giveaway was that the ladder returned byte-identical numbers across a
  behavioural change — v5 was literally v4. Substring matching on event kinds
  is now something the transcript's typed `kind` field makes unnecessary.

The contamination is not incidental to v4: better mana development lengthens
the Selesnya mirror from 26 to 46 mean turns, and the open defect
`battlefield-attachment-illegal-target-invariant-false-positive` fires in 51 of
240 games there, up from 6 at v2. `is_improvement()` still demands every cell
be clean, so no rung is claimed as a formal pass; `is_clean_improvement()`
reports what the uncontaminated evidence supports.

## Deck matchup structure

With v4 piloting both sides, 60 seed pairs per cell (120 games), every mirror
calibrating at exactly 50.0%:

| Deck | Win rate vs the field | n |
| --- | --- | --- |
| Selesnya midrange | **65.3%** [60.2, 70.0] | 360 |
| Golgari midrange | **58.9%** [53.7, 63.9] | 360 |
| Boros aggro | 44.2% [39.1, 49.3] | 360 |
| Boros burn | 31.7% [27.1, 36.6] | 360 |

Notable individual cells: Selesnya beats aggro 76.7%, Selesnya beats burn
77.5%, aggro beats burn 68.3%, and Golgari edges Selesnya 58.3%. That is a
hierarchy with one inversion at the top rather than a clean
rock-paper-scissors, which is unsurprising given how thin the pool's burn is —
the set contains two burn spells that can be aimed at a player.

### An unexplained result worth flagging

Every mirror shows a **negative play advantage**: the player on the play wins
34-50% rather than the ~53% real Magic would predict. In a mirror the two
figures sum to 100% by construction, so Selesnya's 34.0% on the play is a
32-point swing toward the drawing seat.

Two mechanisms plausibly contribute, and this work has not separated them:

1. The engine correctly implements CR 103.8a — the starting player skips their
   first draw — and these games run long enough (13 to 44 turns) that a card
   is worth more than a turn of tempo.
2. Every generation so far is one-ply and reactive. The seat acting second sees
   a developed board before committing, which a deeper policy would exploit
   less asymmetrically.

Until it is separated, no claim is made that this reflects Magic rather than
this policy family. It is the clearest open question the harness has surfaced.

## Reviewing a single match

```sh
cargo run --release -p cardbench-magic-session --bin rav-match-review -- \
  rav_boros_aggro rav_selesnya_midrange 3 --jsonl match.jsonl
```

Prints a per-turn timeline (mana produced, spells cast, lands played,
attackers declared, damage by seat) and a critique, and optionally writes a
JSONL transcript whose events carry typed fields — no regex needed to ask
"how much damage did seat 1 take".

The critique flags suspicions, not verdicts: engine-refused proposals,
non-terminal stops, stalled development, mana produced and unspent, a seat that
never attacked, a match with no blocks. It found its first real result
immediately — the Selesnya pilot declares no attacker at all across a 13-turn
game it loses.

## Campaign statistics

```sh
./scripts/check-batch.sh stats 30      # 300 games, all pairings
```

Win rates say which deck is better; these say what the pilots actually did.
They exist because two improvement leads in a row came from inference rather
than counting — a six-game sample and a static card list, both wrong — and both
would have been caught here.

At v5, 300 games:

| deck | wins/150 | turns | drewCr | castCr | conv | lands | mana | atks | blks | abils |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| boros_aggro | 55 | 8.2 | 5.6 | 5.3 | 0.95 | 7.5 | 14.6 | 4.4 | **0.7** | 0.00 |
| boros_burn | 62 | 7.1 | 5.6 | 3.9 | **0.70** | **5.8** | 15.2 | 3.2 | **0.8** | 0.00 |
| golgari_midrange | 93 | 9.3 | 9.5 | 7.9 | 0.83 | 7.0 | 27.0 | 4.0 | 1.9 | 0.00 |
| selesnya_midrange | 87 | 14.4 | 11.0 | 10.2 | 0.93 | 10.0 | **35.6** | 9.3 | 6.8 | 0.00 |

What this says that no win rate does:

- **`abils` is 0.00 everywhere.** v5's ability planner fires zero times across
  300 games, confirming at campaign scale what one traced game suggested.
- **Aggro and burn essentially never block** (0.7 and 0.8 per game against
  Selesnya's 6.8), and they are the two losing decks. Aggro's archetype weights
  set `own_life` to 0.08, which makes almost every block score negative. That is
  a calibration hypothesis a win rate alone could never have pointed at.
- **Burn converts only 70% of the creatures it draws** and makes the fewest land
  drops per turn (5.8 over 7.1), so it strands its costlier bodies.
- **Midrange out-resources aggro roughly two to one** in mana produced, which
  frames the aggro deficit as much as a deck question as a policy one.

### The v6 hypothesis these numbers produced, and why it failed

The obvious read of the table above is that aggro and burn lose *because* they
never block, and that the cause is aggro's `own_life` weight of 0.08 making
almost every block score negative. A v6 was built that floored the life weight
used for blocking at 0.30, leaving the spending weight alone.

It measured 50.0% on all four decks, n=478 — behaviourally identical to v5 — and
the arithmetic says why. The floor changes the *magnitude* of a block's score
but flips no decision at any realistic board state:

| blocker | attacker | my life | v5 gain | v6 gain | decision |
| --- | --- | --- | --- | --- | --- |
| 2/1 | 3/3 | 20 | -2.86 | -2.07 | decline, both |
| 1/1 | 5/5 | 20 | -1.32 | -0.00 | decline, both |
| 2/5 | 3/3 | 20 | +0.29 | +1.08 | block, both |
| 2/1 | 2/2 | 20 | +0.64 | +1.17 | block, both |

So the correlation is real and the causation runs the other way: aggro blocks
rarely because its creatures are *small*, and a 2/1 trading itself to stop three
damage at twenty life is correctly declined. Small creatures both block badly
and lose to midrange; the blocking figure is a symptom, not the disease.

v6 was reverted rather than kept. Unlike v5, which left reusable machinery, it
was a constant that provably changed no decision, and a rung that cannot differ
still costs eight minutes on every ladder run.

The registry these numbers depend on was itself wrong on first attempt: object
identities were resolved at end of match, and CR 800.4a had already removed
every card owned by the loser, so half the seats reported drawing nothing.
Identities are now snapshotted at setup and merged with an end-of-game pass, and
a fidelity test asserts both seats appear.

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
- **Activated abilities are only half covered.** v5 handles tap- and mana-cost
  abilities; the three sacrifice-cost ones in these decks are reported
  unsupported. Ordruun Commando's damage shield reaches the scorer but has no
  value function, so it always scores zero.
- **No Convoke.** Cards whose cost the planner cannot reduce are simply not
  cast when their printed cost is unpayable.
- **No open engine defects.** `departed-player-orphans-survivor-aura` was the
  last one touching this measurement and is fixed; the full ladder now reports
  `invalid_rungs=0` with zero refused proposals in every cell.
