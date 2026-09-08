# Pokémon code-policy: the reference policy and what it measures

Date: 2026-08-23. Branch: `integration/pokemon-magic`.
Subject: `varieties/pokemon/candidates/reference/reference_policy_v2.rs`.

Companions: `docs/CODE_POLICY_DEO_DESIGN.md` §2 (what a reference must be),
`evals/docs/CODE_POLICY_TASK_STANDARD.md` §2.6 (why an incompetent reference
invalidates the lane).

Every number below was produced by running the sweep, not by reading code. The
train numbers are the full 400-cell surface from `rosters/code_policy_v1.json`
(5 decks × 4 opponent decks × 5 opponents × 2 seats × 2 seeds, 2 games per cell
= 800 games per policy). The held-out numbers are the sealed 384-cell surface.

---

## 1. The defect

`score_metric` is `cell_win_rate_delta`: reward is `candidate − baseline` over
identical cells. That makes the baseline the **ranking origin**, and the origin
was `candidates/reference/baseline_policy.rs` (TemplateAi), which lost to every
opponent on the roster:

| policy | vs `random_ai_v1` | `v2` | `v3` | `v4` | `simple_heuristic_v1` | overall |
| --- | --- | --- | --- | --- | --- | --- |
| `baseline_policy.rs` (old origin) | 33.1% | 33.1% | 19.4% | 16.2% | 42.5% | **28.9%** |
| `algorithm_template.rs` (agent skeleton) | 26.9% | 38.1% | 14.4% | 20.0% | 44.4% | 28.8% |
| `simple_heuristic_ai.rs` | 31.2% | 31.2% | 16.2% | 13.8% | 40.6% | 26.6% |
| `reference_policy_v1.rs` (Harbor oracle) | 31.2% | 51.2% | 18.8% | 21.2% | 55.6% | 35.6% |

An origin at 28.9% that loses every matchup is a floor, not an origin: any
candidate that attacks at all clears it, and the delta stops carrying
information. This is §2.6 of the standard — the same defect as rogue's
`_SCRIPTED_ROUTE` baseline, arrived at from the opposite direction.

## 2. The replacement

`reference_policy_v2.rs`, a one-ply greedy priority ladder. `propose_free_actions`
returns an ordered candidate list; the runner applies the first legal action and
calls again, so the list *is* the ladder and a rung already taken this turn
simply stops being legal.

```
1. lethal check      attack that KOs the defender now
2. forced response   active cannot attack at all: retreat to a readier bench mon
3. supporter / item  convert cards into resources before committing them
4. evolution         evolve the best target, active first
5. energy attachment onto whoever is closest to paying for an attack
6. board development bench basics toward a bench of three
7. best attack       highest damage AFTER weakness and resistance
8. pass
```

It obeys the three contract rules the design names:

* **Self-contained.** Two imports: `tcg_core::{PokemonView}` and
  `tcg_ai::traits::AiController`. Nothing else from the workspace.
* **No memorised lines.** Audited: zero occurrences of any deck name, opponent
  id, task id, or card def-id in the file. The only card text it reads is the
  engine's own canonical `ENERGY-<TYPE>` convention, which is how any policy
  must know what colour of energy it is holding.
* **Deterministic.** No RNG at all — zero `rand`, `shuffle`, `gen_`, clock or
  env reads. Ties break on a fixed mix of the seed and the card instance id.
  Verified empirically: two independent compile-and-sweep runs produced
  byte-identical per-cell telemetry across all 400 cells, and running the policy
  against itself through the sweep gives `delta = 0.0`, `ci = [0.0, 0.0]`.

## 3. Measured result — train surface (400 cells, 800 games)

| policy | `random_ai_v1` | `v2` | `v3` | `v4` | `simple_heuristic_v1` | overall | stall |
| --- | --- | --- | --- | --- | --- | --- | --- |
| old origin `baseline_policy.rs` | 33.1% | 33.1% | 19.4% | 16.2% | 42.5% | 28.9% | 9.8% |
| **new origin `reference_policy_v2.rs`** | **68.8%** | **70.6%** | **56.2%** | **51.9%** | **76.9%** | **64.9%** | 7.9% |

Against the design's target band (§2.2: ≈60-70% vs `random_ai_*`, near 50% vs
`simple_heuristic_v1`):

* `random_ai_v1` 68.8% and `random_ai_v2` 70.6% — **in band**.
* `random_ai_v3` 56.2% and `random_ai_v4` 51.9% — **below band**, and this is
  the target's fault rather than the policy's. Neither is random. `random_ai_v4`
  is 1 100 lines of typed energy attachment, evolution, trainer play, retreat
  scoring, matchup evaluation and full prompt handling; `random_ai_v3` is 918
  lines in the same style. They are the two *strongest* opponents on the roster.
* `simple_heuristic_v1` 76.9% — **above** the "near 50%" target, and again the
  ordering in the target is inverted: `simple_heuristic_v1` attaches energy to a
  uniformly random target, so most of its energy lands on the bench where it can
  never pay for an attack. It is the **weakest** opponent on the roster; the old
  origin's best matchup (42.5%) was against it.

So the roster's names do not track the roster's strengths. Read against the
*intent* of §2.2 — "beats the weak opponents decisively, loses to the strong
ones", so that both floor and ceiling are visible — the reference does what was
asked: 76.9% against the weakest, 51.9% against the strongest, 64.9% overall,
inside the intended overall band.

## 4. Measured result — sealed held-out surface (384 cells)

The held-out opponents are previous agent submissions (`codex_run*`,
`gepa_best_pl`), which is the provenance the design wants: not hand-authored to
be beatable.

| opponent | old origin | **new origin** |
| --- | --- | --- |
| `codex_run01` | 34.4% | **62.5%** |
| `codex_run02` | 46.9% | **83.3%** |
| `codex_run03` | 37.5% | **58.3%** |
| `codex_run04` | 28.1% | **59.4%** |
| `codex_run05` | 18.8% | **43.8%** |
| `codex_run08` | 42.7% | **51.0%** |
| `codex_run10` | 24.0% | **45.8%** |
| `gepa_best_pl` | 50.0% | **69.8%** |
| **overall** | **35.3%** | **59.2%** |

This is the shape the standard asks for. The reference beats five of the eight
held-out opponents decisively and is level with or behind three
(`codex_run05` 43.8%, `codex_run10` 45.8%, `codex_run08` 51.0%). The ceiling is
visible — real submissions already sit above it — and the floor is 24 points
below.

## 5. What actually moved the number

Ablations, each a full 400-cell sweep on the train surface:

| change | overall | Δ |
| --- | --- | --- |
| starting point: first draft of the ladder | 45.5% | — |
| **strict retreat** (only when the active cannot attack at all) | 63.8% | **+18.3** |
| **spend energy on optional `ChooseAttachedEnergy` prompts** instead of declining | 64.9% | **+1.1** |
| remove the trainer rung | 58.9% | −6.0 |
| bench target 4 instead of 3 | 63.1% | −0.7 |
| bench target 5 | 65.1% | +0.2 |
| evolution before trainers in the ladder | 65.0% | +0.1 |
| drop the "active is dying" attachment penalty | 62.6% | −1.2 |
| prefer the bench once the active can attack | 64.9% | 0.0 |
| all three near-neutral changes stacked | 63.9% | −1.0 |

Retreat was the whole story. The first draft retreated whenever the active
looked endangered; retreating pays its cost by *discarding* energy off the
active, so the policy churned actives, burned its energy on retreat costs and
stopped attacking. Its lost games ran 46 turns with 16 energy attached and only
8.7 attacks declared, against 23 turns and 2.76 prizes in its won games. Making
the rung fire only when the active has no payable attack at all recovered 18.3
points.

The last four rows are all inside the ±1.7-point standard error of an 800-game
sweep, and stacking them does not help, so the ladder is at a genuine local
optimum for a one-ply greedy policy. Getting past ~65% needs the things
deliberately left out: lookahead, opponent modelling, and card-specific trainer
lines.

## 6. Defects found in existing Pokémon code

1. **`reference_policy_v1.rs` computes remaining HP wrong.**
   `defender_remaining_hp` returns `hp − damage_counters`, but damage counters
   are worth 10 damage each — the engine knocks out at
   `damage_counters * 10 >= hp` (`tcg_core/src/combat.rs`,
   `check_knockout_for_with_cause`). A 60 HP Pokémon with 50 damage on it read
   as 55 HP remaining instead of 10, so the "prefer a lethal attack" rung it
   documents almost never fired. `random_ai_v4::remaining_hp` gets this right;
   the reference did not.

2. **`ChooseDefenderAttack` is answered with the wrong action, engine-wide.**
   `action_matches_prompt` pairs `Prompt::ChooseDefenderAttack` with
   `Action::ChooseDefenderAttack`. `random_ai_v4` and the other shipped
   opponents answer it with `Action::DeclareAttack`, which does not match, so
   the prompt goes unanswered and the game stalls. 25 of the 62 stalls in a
   full instrumented train sweep came from this one mismatch, all on the
   opponent side.

3. **A stall is charged to the candidate even when the opponent caused it.**
   Cell win rate divides wins by *attempted* games, so a game that nobody could
   finish scores as a non-win for the tracked player. Instrumenting the runner
   over all 800 train games showed 62 stalls, of which only 4 were the
   reference's own doing: 24 were opponents mishandling `ChooseDefenderAttack`,
   14 opponents with no `ChoosePokemonInPlay` handler, 12 opponents with no
   fallback on `ChooseStartingActive`, 7 opponents with no
   `ChooseCardsFromDiscard` handler. That is a ~7% ceiling on any candidate's
   win rate that no amount of play skill can recover. It is a common shift —
   baseline and candidate face the same opponents — so the *delta* is sound, but
   the absolute rate is compressed and should not be read as skill.

4. **The engine has no mulligan.** `game.rs` builds
   `Prompt::ChooseStartingActive { options }` by filtering the opening hand to
   basic Pokémon. An opening hand with no basic yields an empty option set, no
   legal `ChooseActive` exists, and the game deadlocks for both players. This
   accounts for the reference's own 4 remaining stalls and is not fixable from
   a policy: `Action::Concede` is the only legal reply and scores the same as
   the stall.

5. **`algorithm_template.rs` documented an ABI that does not exist.** It told
   agents `card: CardInstance (id, name, card_type, etc.)` — `CardInstance` is
   `{ id, def_id, owner }`, with no name and no card type — and advertised a
   `can_retreat` hint that `ActionHints` does not have. Both corrected in place,
   along with the counters-vs-damage trap.

6. **`benchmark_ai.py` deletes single-item `tcg_core` imports.**
   `filter_duplicate_imports` strips any line starting with `use tcg_core::`,
   and only the braced form goes through the item-level filter that preserves
   non-provided types. So `use tcg_core::PokemonView;` silently disappears and
   the candidate fails to compile, while `use tcg_core::{PokemonView};`
   survives. Not fixed — it is a shared file and the workaround is one brace —
   but it is a trap worth knowing about.

## 7. Wiring and what still needs doing

* `rosters/code_policy_v1.json` — `baseline_policy` now names
  `candidates/reference/reference_policy_v2.rs`; the old file is retained as
  `legacy_baseline_policy` for provenance.
* `scripts/run_policy_sweep.py` — the ranking origin is resolved from the roster
  (`baseline_policy_path()`) instead of being hardcoded, so the origin and the
  cell set it was measured on cannot drift apart. An unresolvable origin raises
  rather than silently scoring against whatever file sits at the old path.
* `scripts/run_policy_sweep.py` — per-opponent reporting now exists on **both**
  splits, not just held-out. `paired_per_opponent()` writes origin win rate,
  candidate win rate, delta and candidate stall fraction per opponent into
  `result.json`, `leaderboard.json` and the held-out scorecard. §3.2 of the
  design asks for per-opponent reporting so that "helps opponent A, hurts
  opponent B" cannot hide inside a mean; the train split had no such view, and
  the per-opponent table is also the only way to see whether the origin is
  competent rather than merely non-zero.

**Open, and outside this change's scope:**
`adapters/harbor/scripts/run_harbor.py` sets
`REFERENCE_POLICY = candidates/reference/reference_policy_v1.rs`, and
`adapters/harbor/bundles/code_policy/instruction.md` names the same file as the
reference solution. That policy measures 35.6% against a 64.9% origin — it loses
by −0.293 with a bootstrap CI of [−0.334, −0.251] — so the Harbor `verify` lane
will now fail until that pointer moves to a policy that actually beats the
origin. No such policy exists in the public tree today; the ladder plateaus at
~65% (§5), so the oracle needs a genuinely stronger design, not a tuned copy.

## 8. Sweep machinery re-verified against the new reference

| check | result |
| --- | --- |
| train sweep covers the roster cell set | 400/400 cells, both policies |
| held-out sweep covers the sealed cell set | 384/384 cells, `roster_sha256` matches the pin |
| paired bootstrap, candidate == origin | `delta 0.0`, `ci [0.0, 0.0]`, `ci_includes_zero`, rejected |
| paired bootstrap, weaker candidate | `baseline_policy` −0.360 `[−0.400, −0.320]`, `reference_policy_v1` −0.293 `[−0.334, −0.251]`, `simple_heuristic_ai` −0.383 `[−0.424, −0.341]`, all `ci_below_zero`, all rejected |
| paired bootstrap, stronger candidate | origin temporarily reverted to the old file: `+0.360 [0.320, 0.400]`, `ci_excludes_zero`, `passed: true`, reward 0.360 |
| fail-closed: one missing cell | raises `SweepError` |
| fail-closed: one unexpected cell | raises `SweepError` |
| fail-closed: stall fraction over 25% | raises `SweepError` |
| fail-closed: zero games attempted | raises `SweepError` |
| fail-closed: origin cannot be resolved | raises before any game is played |

The sweep's own scores reproduce the independent measurement harness exactly
(0.64875 / 0.28875 / 0.35625 / 0.26625), which cross-validates the two paths.
