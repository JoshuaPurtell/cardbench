## Candidate contract

A candidate is exactly ONE file: `candidate/policy.rs`, exporting one function:

```rust
pub fn build_policy(
    player: PlayerId,
    archetype: Archetype,
    index: Arc<CardIndex>,
) -> Box<dyn CodePolicy>
```

That is the only symbol the grader looks for. Define whatever structs you like
around it. The file is compiled into the sweep, so it may use
`cardbench_magic_policies` and `cardbench_magic_engine`; nothing else in this
workspace is on its path.

`build_policy` is called **once per seat**, so one instance plays one seat of one
game and any state you keep is scoped to that game.

**`CodePolicy` has one required method, `propose_move`.** Pending decisions,
optional triggers, draw replacement and library choices all have conservative
defaults you can leave alone.

**Start by wrapping, not by writing from scratch.**
`candidates/algorithm_template.rs` delegates every decision to the reference
generation, which means your first submission is already a legal, working policy
scoring a delta of zero — and every point after that is attributable to a
decision you actually changed. An empty `propose_move` means reimplementing mana
payment, combat and targeting before you can finish a single game.
`candidates/reference/reference_policy_v1.rs` is the same idea in its shortest
form and is exactly the ranking origin.

### Your deck

You are graded on a deck from the roster's `candidate_deck_pool`, and **deck and
pilot are graded together** — a strong pilot cannot rescue an incoherent deck and
a tuned deck cannot rescue a pilot that never attacks.

Write a deck id into `candidate/deck.txt` to choose one. With no `deck.txt` you
play `rav_selesnya_midrange`, which is the deck the ranking origin plays, and
which is the only choice that isolates your pilot from your deck.

### What fails your run outright

The sweep is **fail-closed on coverage**. A missing cell, an extra cell, an
unpaired cell, or a stall rate above 25% on either arm scores zero rather than
reporting the mean of the cells it managed to play. A policy that hangs the game
is therefore scored worse than one that plays badly, and `rejected_moves` in the
report counts actions the engine refused — a rising count means you are proposing
illegal moves and falling back.

The held-out split is 5 opponents you cannot see, over 300 cells.
