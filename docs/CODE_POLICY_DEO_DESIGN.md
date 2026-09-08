# CardBench code-policy DEO — deck + pilot against hidden opponents

Date: 2026-08-23. Status: design sketch. Branch: `integration/pokemon-magic`.

Companion: `evals/docs/CODE_POLICY_TASK_STANDARD.md` is the validity standard
this must satisfy; `evals/docs/handoffs/HANDOFF_GAMEBENCH_CODE_POLICY_DEO.md`
records why each rule in it exists.

---

## 0. What the merge established

`integration/pokemon-magic` = `feat/code-policy-train-heldout-split` + `dev`.
Before it, neither branch had both halves:

| | Pokémon code-policy roster | Magic engine |
| --- | --- | --- |
| `feat/code-policy-train-heldout-split` | yes | no |
| `dev` | no | yes |
| **`integration/pokemon-magic`** | **yes** | **yes** |

Two conflicts, both resolved rather than taken from one side:

* `.gitignore` — union. Each side added different ignores.
* `run_harbor.py` — a *semantic* conflict. `dev` added a `variety` parameter
  (magic vs pokemon dispatch); the Pokémon branch added a `split` parameter
  (train diagnostic vs held-out authority). Both are load-bearing, so
  `score_command` now takes both, and every call site passes `variety` —
  including one in `run_reference` that neither side's diff touched and that
  would have raised `TypeError` on the first reference run.

`.sealed/` is gitignored by design. The sealed held-out data — `heldout_v1.json`,
`server_heldout.sqlite`, and the `opponents/` directory holding
`codex_run01.rs … codex_run10.rs` — lives on disk only and does not travel with a
branch. **A fresh clone has no held-out split.** Provisioning it is a step, not an
assumption.

---

## 1. What already exists (do not rebuild)

### Pokémon
* Submission ABI: `tcg_ai::traits::AiController`, Rust.
* Reference: `candidates/reference/baseline_policy.rs`; `algorithm_template.rs`
  is the agent-facing starting point, documented at ~25% win rate.
* Roster `rosters/code_policy_v1.json`: 5 train decks × 5 train opponents
  (`random_ai_v1..v4`, `simple_heuristic_v1`).
* Sealed held-out: 10 opponents that are **previous Codex runs**.
* Cell = `(candidate deck, opponent deck, opponent, seat, seed)`.
* `score_metric: cell_win_rate_delta` — already a delta, not an absolute.
* Paired bootstrap over identical cells; fail-closed coverage (a sweep that does
  not reproduce the roster's cell set exactly scores zero).
* Decks are JSON: `{name, source, cards:[{card_def_id, count}]}`.

### Magic
* Submission ABI: `pub trait CodePolicy` — `propose_move`,
  `propose_pending_decision`, `propose_optional_triggered_ability`, the latter
  two with conservative defaults. Documented as *"Submission ABI for
  `cardbench/magic/code_policy`"*, so the family was always intended.
* 19 archetype policies across the Ravnica guilds, plus generations `v1..v5` in
  `policies/src/archetypes/` — a real ladder with documented deltas.
* `policies/src/ladder.rs`: mirror decks, every deck (not one), paired seats,
  Wilson intervals.
* Binaries: `rav_deck_hillclimb`, `rav_policy_ladder`, `rav_policy_match`,
  `rav_archetype_matrix`, `rav_engine_tournament`, `rav_ratings`.
* Decks are TOML fixtures; `load_constructed_decks()` is the frozen control group
  and `load_hillclimb_decks()` is a deliberately separate index the ladder and
  public matrix never see.
* **No held-out concept.** Zero hits for `heldout`/`roster` across `policies`,
  `arena`, `engine`, `session`. This is the gap.

---

## 2. Reference code policies

A reference policy has a harder job here than in a single-player game: it is the
ranking origin *and* the worked example. The standard requires it to be
competent, or every candidate beats it and the benchmark measures nothing.

### 2.1 Requirements

1. **Self-contained under the submission ABI.** One compilation unit, nothing
   from the workspace beyond the engine view types the ABI already exposes.
2. **Beats the weak opponents decisively, loses to the strong ones.** If it beats
   everything the ceiling is invisible; if it loses to everything the floor is.
   Target ≈ 60-70% against `random_ai_*`, near 50% against `simple_heuristic_v1`.
3. **No memorised lines.** Rogue's baseline replayed a fixed action sequence keyed
   on task id and looked competent for exactly one map. A card baseline must not
   branch on deck id or opponent id.
4. **Deterministic given (seed, seat).** Two runs of a cell must agree, or the
   paired comparison is comparing noise.

### 2.2 Pokémon reference — sketch

Layered, each layer independently testable:

```
priority ladder (first match wins)
  1. lethal check      — attack that KOs and wins the prize race now
  2. forced response   — active is about to be KO'd: retreat or evolve
  3. board development — bench a basic if bench < 3 and a basic is in hand
  4. evolution         — evolve any benched basic whose stage-1 is in hand
  5. energy attachment — attach to the attacker that reaches an attack soonest
  6. supporter         — draw supporter if hand < 4, else search
  7. best attack       — highest expected damage after weakness/resistance
  8. pass
```

Rungs 1 and 7 are the whole difference between ~25% and a real baseline. The rest
keeps it from bricking.

### 2.3 Magic reference — sketch

Magic already has `v1..v5`; the reference should be a **named freeze** of one
generation, not a new policy:

```
reference_v1 := archetypes::v5   (frozen, versioned, never edited in place)
```

`v5` documents itself as carrying "v2's whole-set attack planning, v3's valued
blocking, v4's land sequencing, and adds activated abilities" — the same layered
structure as above, already built and already laddered. Freezing a generation
gives a reference that is competent *and* whose competence is documented as a
diff against its predecessor.

New generations then become candidates rather than baseline edits, which is what
`ladder.rs` was written to measure.

---

## 3. The DEO task: deck + pilot

The agent submits **two artifacts**, graded together:

```
submission/
  decklist.json     (Pokémon)  or  decklist.toml  (Magic)
  policy.rs         the pilot, under the variety's submission ABI
```

This is the interesting part of the design. A deck is only good *with* a pilot
that can play it, and a pilot is only good *with* a deck that supports its plan.
Grading them jointly is the honest formulation and is strictly harder than
either alone: a strong pilot cannot rescue an incoherent deck, and a tuned deck
cannot rescue a pilot that never attacks.

### 3.1 The opponent surface: 5 visible, 5 hidden

```
train  (visible in the workspace, feedback only)
  opp_t1 … opp_t5    decks + pilots the agent can read and play against

heldout (sealed, authority)
  opp_h1 … opp_h5    never present in the workspace at any point
```

The held-out five are what the reward is computed from. The visible five exist so
the agent can iterate without a blind budget.

**Provenance of the hidden five.** Pokémon already does the right thing —
`codex_run01..10.rs` are *previous agents' submissions*, sealed. That gives an
opponent pool that is (a) not hand-authored to be beatable, (b) improves as the
benchmark is used, and (c) cannot be reverse-engineered from the reference. Magic
should adopt the same: seal a generation of archetype pilots plus their decks.

### 3.2 Cells and pairing

```
cell = (candidate deck, candidate pilot, opponent, opponent deck, seat, seed)
```

* **Both seats.** Every pairing played twice with seats swapped, so play advantage
  cancels. `ladder.rs` already does this.
* **Identical cells for baseline and candidate.** The lift is a paired difference
  then a bootstrap — never a difference of two blended win rates.
* **Per-opponent reporting, not just the aggregate.** Magic's ladder already
  refuses to let "helps Boros, hurts Golgari" hide inside a mean; the same rule
  applies per opponent here.
* **Wilson intervals** on each rate; a step counts only when the interval clears.
* **Fail-closed coverage.** Missing cells score zero, not a partial result.

### 3.3 Reward

```
reward = mean over heldout opponents of (candidate_win_rate - baseline_win_rate)
gate   = lower bound of the bootstrap CI > 0
```

Delta, not absolute — the thing still open for rogue (§4.6 of the handoff).
Pokémon's `cell_win_rate_delta` is already exactly this.

---

## 4. Task format

Harbor bundle, the shape the gamebench DEO lane was certified against:

```
adapters/harbor/bundles/code_policy_cards/
  task.toml                     schema_version 1.1; [task] [metadata] [agent]
                                [verifier] [environment]
  instruction.md                cwd, artifacts, contract, iterate loop
  environment/Dockerfile        rust toolchain + engine + docker-cli
  environment/setup_workspace.sh
  tests/test.sh                 -> score_code_policy.py
  tests/score_code_policy.py    runs the sweep from /task, writes reward.txt
  solution/solve.sh             reference submission, for the oracle lane
```

Baked per variety with build args, exactly as the gamebench bundle is:

```
--build-arg CARDBENCH_VARIETY=pokemon   -> cb-cpo-pokemon
--build-arg CARDBENCH_VARIETY=magic     -> cb-cpo-magic
```

Registered in
`evals/containers/images/harbor-code-policy/harbor_code_policy/trials.py` as
`TrialImage(..., candidate_sandbox_docker=True)`, digest-pinned.

Non-negotiables inherited from the standard: graded code and suites served from
`/task` (never the agent's workspace), held-out data stripped from the workspace
copy at bake time, reward gated on verifier exit code, isolation reported from the
receipt rather than a constant.

### 4.1 The one genuinely new problem: Rust in the sandbox

Every rule transfers except the sandbox contract. Gamebench copies **one Python
file** into a container with a bare interpreter. A Rust submission has to be
compiled, and compilation is the part that wants a network, a crate registry, and
minutes of CPU.

Resolution — compile **outside**, run **inside**:

```
verifier (has /task, the engine, a warm vendored cargo registry)
   └─ cargo build --offline   candidate policy.rs against a frozen shim crate
        └─ self-contained binary
             └─ THAT binary goes into the per-episode sandbox container
                (network=none, read-only, uid 65534, pids-limit, memory cap)
```

The candidate's *code* is untrusted at compile time and its *binary* is untrusted
at run time. Different threats:

* **Compile** is bounded by `--offline` against a vendored registry (no network,
  no arbitrary dependency), a wall-clock cap, and an outright **rejection of
  `build.rs`** — a build script is arbitrary code execution at build time and must
  be refused, not sandboxed.
* **Run** is the existing container sandbox, unchanged.

A compile failure is a `candidate_policy_failure` — the candidate's fault, never
retried, scored zero, and explicitly *not* an infra abort.

`policies/src/bin/rav_policy_match.rs` and the Pokémon sweep's existing
`subprocess` compile path are the starting points; neither is sandboxed today.

### 4.2 Verifier budget

Magic games are slow and cells multiply fast:

```
5 heldout opponents × 5 opponent decks × 2 seats × N seeds × 2 policies
```

At N=10 that is 1000 games per grade. The gamebench lane needed `parallel=8` and a
2700 s budget for 400 episodes. **Measure per-game cost first, then choose N** —
gamebench learned this the expensive way when a 600 s client timeout tore the
platform down mid-grade and reported it as a lane failure.

---

## 5. Order of work

1. **Provision the sealed split for Magic.** Pokémon has one; Magic has none, and
   nothing downstream is meaningful without it.
2. **Freeze the references.** Magic: name `archetypes::v5` as `reference_v1`.
   Pokémon: raise the template from ~25% to a real baseline (§2.2).
3. **Rust-in-sandbox** (§4.1). Shared prerequisite; nothing ships before it.
4. **Bake `cb-cpo-pokemon`** — design is furthest along, cheapest first proof.
5. **Bake `cb-cpo-magic`.**
6. **Back-port to gamebench**: paired comparison, Wilson intervals, per-opponent
   regression reporting, delta metric. Rogue has none of these.

Independent of the above: **craftax full** (generator confirmed 120/120 unique,
100-seed suite already ships, image built, baseline and isolation already fixed),
then **crafter classic**, then **craftax partial**, then **dungeongrid** (needs a
scenario generator; its composite scoring design is already the best in gamebench
and worth copying regardless).
