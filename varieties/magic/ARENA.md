# The arena

A container that seats **code policies** and **LLM agents** against each other
on the same Ravnica constructed decks, under the same seeds, reported the same
way.

```sh
cd varieties/magic

rav-arena probe 3                       # how many decisions are decisions
rav-arena code-vs-code   3 v7 v5        # two policy generations
rav-arena react-vs-code  3 --model M    # a model against a policy
rav-arena react-vs-react 3 --model M    # a model against itself (calibration)
rav-arena react-vs-react 3 --model M --model-b N   # two different models
```

---

## 1. Why it is shaped like this

### The measurement that decides everything

A real 38-turn match takes **1025 policy decisions**. Magic has priority;
Pokémon does not. A seat that consults a model on every decision costs a
thousand calls per game and is not affordable at any model price.

But most of those decisions are not decisions. Measured over twelve real games
with `rav-arena probe`:

```
seat_decisions=2342
open_decisions=599      open_share=25.6%
open_priority=509  open_attacks=75  open_blocks=15
widest_menu=24
model_calls_per_game_estimate=49.9
```

**74.4% of decisions have exactly one legal reply.** The model is consulted
only on the rest. That is what makes a model seat affordable.

`model_calls_per_game_estimate` is an **upper bound, and a loose one**. Measured
against real model seats it overstates by about 2.5×:

| | probe estimate | measured |
| --- | --- | --- |
| consultations per game | 49.9 | **19.0** (455 over 24 games) |

The gap is plan-following. The probe counts each mana-tap decision as its own
open menu, because the code policy it wraps really does decide them one at a
time; the agent seat chooses a cast once and plays the taps out itself. In the
same 24 games those became 302 `plan_steps` and cost nothing.

Re-run `probe` after any change to what `menu.rs` enumerates. The number is
load-bearing and it will move.

### An agent seat is an ordinary `CodePolicy`

`ReactPolicy` implements the same trait the archetype policies do, so
`run_deck_matchup_with` seats it without knowing what it is. Paired seeds, seat
swapping, Wilson intervals, the seat split, per-deck cells, and contamination
reporting all apply to a model with no new statistics code. This is the main
reason the crate is Rust rather than a Python driver.

### The model picks from a menu, it does not compose actions

The engine exposes **no legal-move enumerator** — `GameView` is data with no
methods, and code policies construct an action the engine then accepts or
refuses. `menu.rs` builds the missing enumerator, and the model returns an
index.

The payoff is that an agent seat cannot make an illegal move, so it never lands
in the `rejected_policy_moves` path that `HANDOFF.md` §5 calls "never data".
The price is a ceiling: **an agent can only be as good as what `menu.rs`
enumerates.** Three bounds are known and unfixed —

- blocks are none / the planner's assignment / every single attacker-blocker
  pair, so no multi-blocks and no blocking several attackers at once;
- attacks are the three aggression levels plus none and all, not arbitrary
  subsets;
- activated abilities are the planner's single best activation.

Widening any of these changes what every agent seat can express. Treat it as a
version bump and re-measure.

---

## 2. Reading a result

```
cell deck=rav_boros_burn archetype=burn rate=66.7% ci=[30.0,90.3] n=6 mean_turns=12.5 rejected=0 truncated=0
  seats deck=rav_boros_burn on_play=100.0% on_draw=33.3%
overall seat_a=v7 rate=54.2% ci=[35.1,72.1] n=24 a_stronger=false
seat_a decisions=612 open=143 consulted=118 plan_steps=61 delegated=9
seat_a fallbacks=25 parse_failures=4 provider_failures=0 budget_exhausted=21 agency=82.5%
valid=true
```

Four things decide whether the rate means anything:

1. **`valid=true`.** Any `rejected` move means those games ended by an engine
   refusal rather than by play.
2. **`agency`.** The share of open decisions the model actually decided. A seat
   that fell back on most of them measured the *fallback policy* wearing the
   model's name. Watch `provider_failures` and `budget_exhausted` for why.
3. **The seat split.** The paired rate cancels anything seat-dependent
   *exactly*. A model strong on the play and weak on the draw reads as 50%.
   This is the mistake the policy ladder made for four generations; see
   `POLICY_LADDER.md`.
4. **`n`.** At `pairs = 3` the interval is ±20 points. It is a smoke test, not
   a measurement.

`delegated` counts no-priority rules decisions — library searches, trigger
targets, dredge — which always go to the fallback and are never offered to the
model. They are reported separately so they cannot inflate `fallbacks`.

---

## 3. Measured so far

`react-vs-code` against **v7**, 24 games each, `pairs=3`, `reasoning_effort=low`.
Both seats play the same deck; v7 is also each model seat's fallback.

| Model | Win rate vs v7 | Agency | Valid |
| --- | --- | --- | --- |
| `openai/gpt-oss-120b` | 33.3% [18.0, 53.3] | 99.4% | ✓ |
| `openai/gpt-oss-20b` | 29.2% [14.9, 49.2] | 100.0% | ✓ |

Both lose to v7, and the two are not distinguishable from each other at this
sample size. Agency near 100% is what makes the rates mean anything: no
fallbacks, no engine refusals, no truncated replies.

### The models collapse on the draw

Pooling both models' seat splits — exact counts, not estimates:

| | wins | rate |
| --- | --- | --- |
| on the play | 12 / 24 | **50.0%** [31.4, 68.6] |
| on the draw | 3 / 24 | **12.5%** [4.3, 31.0] |

On the play these models hold parity with v7. On the draw they win one game in
eight, and the intervals do not overlap. Pooling two models is informal, but
the pattern is consistent across all four decks.

The obvious hypothesis is not "the models are worse". Playing from behind is
mostly defence, and defence is the weakest part of the menu: no multi-blocks,
no blocking several attackers at once. **It may be that the menu does not offer
the moves rather than that the model cannot find them**, which makes widening
the block enumeration the first thing to test.

---

## 4. Configuration

`arena.toml`, next to this file. Model, endpoint, temperature, budgets, and
pair count live there; **only the API key comes from the environment**
(`OPENROUTER_API_KEY` by default). An unknown key in the file is an error
rather than a setting that silently did nothing.

The provider is any OpenAI-compatible `/chat/completions` endpoint. HTTP is
done by shelling out to `curl` rather than by adding a TLS stack to a workspace
that is otherwise nearly dependency-free; `LlmProvider` is the seam, and
swapping in a real client changes one file.

The key is written to a mode-0600 scratch file and passed with `curl --config`
rather than `-H`, because process arguments are world-readable.

---

## 5. What this is for

Same goal as `HANDOFF.md` §1 — policies strong enough that games between them
are productive gameplay — with the two downstream uses now runnable:

- **Ask a model to write a better policy.** Score it with `rav-policy-ladder N
  <candidate> <baseline>`. Unchanged; the arena does not touch that lane.
- **Have a model play against the policies.** `rav-arena react-vs-code N
  --model M --code v7`. The baseline has to be strong for this to measure
  anything, which is why the policy work comes first.

`react-vs-react` with one model on both seats is the calibration case: it must
land at 50%. If it does not, something is seat-dependent and the split will say
where.

---

## 6. Things that will bite you

**A fallback is not a pass.** When the model errors or replies unreadably the
seat delegates to a code policy, so the game always finishes. That is correct
and it is also how a run silently stops being about the model. Read `agency`
before reading the rate.

**The parser refuses rather than repairs.** An out-of-range index is an error,
not a clamp — clamping would play option 4 when the model asked for 9 and the
transcript would record a move nobody chose. A bare integer is accepted only
when it is the *entire* reply, so "it deals 3 damage" is not read as choice 3.

**The system prompt and the parser travel together.** A prompt asking for
`{"action": ...}` would produce replies the parser reads as no choice at all,
and every decision would fall back. There is a test pinning them together.

**A reasoning model can silently spend its whole budget thinking.** At
`max_tokens=512` both gpt-oss builds return `finish_reason=length` with an
empty message, having spent ~490 tokens reasoning and written nothing. Every
consultation then fails and the seat plays its fallback while the output still
says the model's name. This is not hypothetical -- it is how the first gpt-oss
run was wasted. The defaults are now 2048 tokens and `reasoning_effort = "low"`,
and an empty message is reported as itself rather than as "no choices in
response".

Raising effort raises the floor with it. Measured: `medium` needs more than
2048 on gpt-oss-20b, and `high` on gpt-oss-120b spent 1922 tokens and truncated
at 2048 while taking 95-122s per call. **Raise `max_tokens` and `timeout`
together with `reasoning_effort`, or the higher setting produces worse data
than the lower one.** `--reasoning-effort`, `--max-tokens`, and `--timeout` are
flags for exactly this reason, and every result prints the `provider` line that
produced it.

**Cost scales with `pairs` times ~19.** `pairs = 3` is 24 games and roughly
460 calls per model seat. The ladder's 60 pairs would be 480 games and about
9,000. Widen deliberately.

**The probe is not free either** — it plays real games with code policies, so a
large `probe` count costs minutes, not money.
