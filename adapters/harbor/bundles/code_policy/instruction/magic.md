# CardBench Magic code-policy task

Write `candidate/policy.rs`: one Rust file exporting

```rust
pub fn build_policy(
    player: PlayerId,
    archetype: Archetype,
    index: Arc<CardIndex>,
) -> Box<dyn CodePolicy>
```

It is compiled into the sweep, so it may use `cardbench_magic_policies` and
`cardbench_magic_engine`. It must compile, propose legal actions, and finish its
games.

`build_policy` is called once per seat. One instance plays one seat of one game.

## Start by wrapping the reference

`candidates/algorithm_template.rs` forwards every decision to the reference
generation. Copy it and you have a legal, working submission that scores a delta
of exactly zero before you have made a single decision of your own — and after
that, every point is attributable to a decision you actually changed.

`CodePolicy` has **one required method**, `propose_move`. Pending decisions,
optional triggers, draw replacement and library choices all have conservative
defaults. Override them when you want them, not before.

Writing `propose_move` from scratch means reimplementing mana payment, combat and
targeting before you can finish one game. That is not where the value is.

## How you are scored

Play is scored over a **fixed grid of cells**. One cell is:

```
opponent | seat (p0/p1) | seed
```

Your policy and the frozen reference (`reference_v1`, a named freeze of
`archetypes::v5`) are run over *exactly the same cells* and compared pairwise, so
a win only counts if it beats the reference on the same matchup. The lift must
clear a paired interval, and **no single opponent may regress significantly** —
a candidate that gains overall by collapsing against one opponent does not pass.

| Split | Opponents | Cells | Role |
|-------|-----------|-------|------|
| **train** | the 5 in `rosters/code_policy_v1.json` | visible | feedback — iterate against this |
| **heldout** | 5 you cannot see | 300 | **authority** — this is your score |

`rosters/code_policy_v1.json` is the contract. It names the whole train surface
and publishes the *shape* and the sha256 of the heldout split, never its
composition.

The heldout opponents are `(pilot generation, deck)` pairs whose halves are both
public. What is sealed is which pilot flies which deck, over which seats and
seeds. Tuning to the five specific train opponents will not transfer.

## Your deck

You are graded on a deck from the roster's `candidate_deck_pool`, and **deck and
pilot are graded together**: a strong pilot cannot rescue an incoherent deck and
a tuned deck cannot rescue a pilot that never attacks.

Write a deck id into `candidate/deck.txt` to choose. With no `deck.txt` you play
`rav_selesnya_midrange` — the deck the reference plays, and the only choice that
isolates your pilot from your deck.

## Iterating

```bash
./run_train_sweep.sh
```

Use the wrapper, not `run_policy_sweep.py` directly: the sweep compiles Rust, and
depending on where this workspace is mounted there may be no toolchain here. The
wrapper detects that and re-runs the same sweep where one exists.

It grades the **train split only**. No argument reaches the heldout split.

## Coverage is fail-closed

A missing cell, an extra cell, an unpaired cell, or a stall rate above 25% on
either arm **scores zero** rather than reporting the mean of the cells that did
run. A policy that hangs games is scored worse than one that plays badly.

Read `rejected_moves` in the report: it counts actions the engine refused. A
rising count means you are proposing illegal moves and silently falling back to
something else.

## Rules

- Do not read, write, or otherwise reach into `varieties/magic/.sealed/`. It is
  excluded from this workspace; attempting to reconstruct it is disqualifying.
- Do not modify the roster, the sweep, or the reference policy. The graded copies
  live outside this workspace — editing yours only makes your own feedback lie.
- Do not read or edit verifier output.
