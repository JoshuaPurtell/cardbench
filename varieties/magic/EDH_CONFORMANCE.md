# EDH conformance work — not a certification

Current requested scope: [RAV Commander dynamics](RAV_COMMANDER_SCOPE.md).
The arbitrary-card blockers below are not prerequisites for mechanics absent
from that explicitly admitted pool.

Status: partial implementation. `Commander` remains the historical M5 fixture
capability; `CommanderEdh` is explicitly absent. Neither a green M5 test nor a
deterministic replay establishes arbitrary-deck EDH conformance.

Rules authority: [Wizards Comprehensive Rules](https://magic.wizards.com/en/rules),
particularly 103, 704, 800 and 903. Card legality additionally requires a
versioned Oracle/ban-list snapshot. Do not derive color identity from mana cost
alone, or accept legality metadata from a submitted deck/policy.

## Implemented in this change

- Native graveyard/exile commander arrivals retain their real destination.
  At an SBA boundary the owner receives a typed optional `CommanderReturn`
  decision. Acceptance and decline are recorded by the existing decision
  receipts; decision ids and zone incarnations reject stale responses.
- Newly arrived commanders are eligible from any represented zone, including
  spell-stack termination. Blink that returns before the SBA does not create
  a stale return opportunity. Declining consumes that arrival's opportunity.
- Commander choices are collected in APNAP owner order before applying the
  selected returns. Death triggers survive the choice boundary and are placed
  after the SBA choices, not before them.
- `CommanderDeck::validate` checks exactly 100 cards including commanders,
  no sideboard, nonzero entries, singleton/copy exceptions aggregated across
  canonical names, full supplied color identity, basic-land-type restrictions,
  legal and implemented flags, and mutually permitted commander pairs.
- `Game::load_commander_deck` validates before mutation, requires executable
  definitions, seats commanders in the command zone and the remaining cards in
  a shuffled library, and sets that seat to 40 life with threshold 21.
- The catalog policy explicitly elects command-zone return. That is a policy
  choice, not a forced engine replacement.
- `LondonPregame` owns an unstarted full-deck game through all-seat London
  declarations, simultaneous redraws, multiplayer's free first mulligan, and
  private bottom-card choices. Round nonces reject stale answers. Strict
  Commander/constructed-2HG constructors recheck loaded decks; opening-hand
  special abilities remain outside this entry point's declared pool.
- Explicit per-attacker player targets support split-defender combat. Each
  independent defender declares blocks in APNAP order without intervening
  priority; unblocked damage retains its individual recipient. Shared-team
  combat allows cross-teammate blocks and uses the attacked player's lands
  for landwalk. Legacy Team-only declarations remain fixture compatibility.
- Policy views expose public command zones, commander taxes, individual
  combat targets and collected block declarations. Teammates are separate
  from opponents; owned commanders enter casting menus without becoming hand
  cards or changing printed mana value.
- Shared-turn sorcery stack invariants recognize either active teammate.
  Cleanup collects both active teammates' private discard choices before
  discarding either hand, then resumes the shared cleanup boundary.
- Typed `CommanderZoneReplacement` decisions now suspend supported stack
  instructions before a hand/library move. The owner can accept or decline;
  instruction cursors, exact incarnations, and one-use choices preserve the
  enclosing spell's prefix and suffix without granting priority. Covered
  paths include targeted bounce/recursion/top/bottom, source bounce/shuffle,
  attached-source bounce, current-turn creature recursion, graveyard shuffle,
  selected public graveyard returns, selected single-card library search,
  linked-exile hand returns, and selected Cloudstone Curio returns.
  Linked returns collect the owner's choice before any linked card moves;
  return receipts exclude commanders whose destination was replaced. Curio
  retains the controller's selection through the separate owner choice.
  Hidden-zone choices are private. Search receipts recognize the actual chosen
  destination, and a replaced bottom move cannot remove another library card.
  Unsupported live commander hand/library moves now reject atomically rather
  than silently omitting the optional replacement. This is not full coverage.
- Further continuations cover ordinary draw steps, multi-card and conditional
  draws, Dredge's post-mill return, group draw/discard, reveal-to-hand life loss,
  paid library looks, multi-card searches, private library partitions, hand
  recycling, and Warp World's initial shuffle-out. Draw counts, revealed cards,
  paid life and completed sacrifices/retargeting survive the choice boundary.
  Activation return/hand-to-library costs collect owner choices before paying
  mana, life or counters; the saved request is resumed without charging twice.
  Library reordering within the same zone does not offer a zone replacement.
  The historical immediate Transmute helper is rejected in live Commander;
  the real stack-backed `activate_transmute` path remains available.
- Real full-deck Commander runs exposed a historical Szadek receipt audit
  consulting the victim's current lost status. It now validates departure
  order at the event boundary, preserving earlier damage while rejecting
  post-departure receipts. Positive and replay-corruption tests cover both.
- Disembowel now has an exact-X target primitive matching its Oracle text,
  checked at casting and at the resolution target-legality boundary, including
  copied mana values. The previous at-most-X primitive remains available for
  generic fixtures but no longer defines Disembowel. Brightflame's X damage
  quantity no longer accidentally imposes a creature mana-value restriction.
- Warp World now retains its shuffle/reveal prefix while explicit typed
  decisions collect shockland life payments, Aura endpoints and owner-selected
  bottom order. Artifact/creature/land entry and enchantment entry are separate
  simultaneous batches; intervening decisions do not grant priority or run
  SBAs. Unattachable Auras remain in the library, tokens count toward reveals,
  and the selected bottom order has a public replay receipt. These changes
  cover the RAV entry paths; they do not certify arbitrary registered entry
  copy/replacement combinations outside that pool.
- Non-targeting Aura entry and attachment SBAs now use enchant/protection
  legality without incorrectly applying shroud. Aura spell targets retain
  the ordinary targeting check. Focused tests distinguish shroud from protection.
- Greater Mossdog's unrelated capability label is removed and its exact trace
  contracts now include the engine's incarnation receipts (no event filtering).
  These three existing regressions previously required a VisualsBench overlay.
- Scatter the Seeds' exact trace now includes its two incarnation receipts.
  The passive-seat critique fixture uses an explicit nonattacking pilot rather
  than assuming the evolving latest policy will never attack.

`CommanderCatalog` is a trusted host API, **not a supplied full Oracle catalog**.
Pair eligibility and full identities must be reviewed and versioned by the
host. The old configurable setup/designation methods remain fixture seams;
they must not be used as submitted-deck legality checks.

## Remaining release blockers

1. Audit the expanded hand/library continuation coverage against all caller
   paths, especially competing replacements, draw-replacement interactions,
   and simultaneous SBAs across team draws. Warp World's RAV two-wave entry
   choices are implemented; arbitrary entry-copy/replacement combinations
   beyond that declared pool remain unaudited. The only existing
   additional spell-cost kind is creature sacrifice; new cost kinds must gain
   their own continuation support before admission.
2. Opening-hand abilities and production integration of the new pregame
   controller, including replay receipts and supported-pool admission.
3. Versioned, reviewed card metadata covering the declared playable pool;
   per-card executable behavior coverage. The new metadata validator is not
   evidence that every legal Magic card is implemented.
4. Independent multiplayer combat/protocol authority, including departure
   during partially collected declarations, team choices/trigger ordering,
   and non-player defender types. The new split-combat tests do not authorize
   a blanket protocol capability upgrade.
5. Broader commander/tax/copy/control/merged-object and multiplayer departure,
   concession, alternative win/loss, replacement and trigger interactions.
6. An independent rule-indexed conformance corpus and full green engine/set/
   policy/protocol regression gate, then native Cybernetics evaluator migration.

## Evidence commands

The expanded continuation batch passed all 799 binaries in
`artifacts/magic-regression-vpkc72sv/receipt.json`. That receipt predates the
subsequent Szadek historical-validation and exact-X fixes; it must not be used
as final-source evidence for those changes. The post-fix engine library has
57 passing unit tests; focused real-card tests cover Szadek, Disembowel and
Brightflame. Cybernetics' native runner is staged, not a promoted evaluator.

Prior-batch verification: all **797 Rust test binaries passed** in
`artifacts/magic-regression-k6_vvoui/receipt.json` (locked offline build, eight
parallel processes). The four Cybernetics capability/multiplayer tests passed.
The earlier `magic-regression-zgo7cdse` receipt records the two failures fixed
before this final run; the sequential `edh-*.log` files are preliminary runs,
not the final authority. Full EDH remains unimplemented despite this green gate.

The follow-up adds engine unit tests in `pregame.rs` and
`multiplayer_combat_tests.rs`, plus real-card admission/projection tests in
`arena/tests/native_commander_pregame.rs`. The native 2HG full-game test starts
four legal 60-card constructed decks with empty battlefields and reaches real
terminal outcomes for shown seeds 73 and 192, with no rejected policy move.
These are development regressions, not sealed benchmark workloads or scores.
The Commander three-card setup also reaches a native terminal outcome using
an explicitly weak attack-all/no-block smoke pilot. The general-purpose pilot
run was stopped after six minutes without completion; it did not earn a pass.
The smoke proves setup/casting/combat/termination plumbing, not strategic
difficulty, policy strength, or arbitrary-card EDH conformance.
Do not use a regression receipt built before the last source edit as final
evidence. The regression runner now records source hashes and fails on source
or executable drift during verification.

From `varieties/magic`:

```sh
cargo test -p cardbench-magic-engine --lib
cargo test -p cardbench-magic-engine --test formats_m5
cargo test -p cardbench-magic-protocol --test commander_scope
cargo test --workspace --tests --jobs 4
```

New tests are in `engine/src/commander.rs`, `engine/src/commander_tests.rs`, and
`protocol/tests/commander_scope.rs`. Broad regression logs are local artifacts
under `artifacts/edh-*-regression.log`; inspect their exit status and failures,
not just the number of passing tests.

For bounded parallel verification, run `python3 scripts/check_magic_workspace.py
--workers 4` from the repository root. It builds with locked offline dependencies,
then runs every cargo-emitted test executable from its owning package directory.
Each unique `artifacts/magic-regression-*/receipt.json` records binary hashes,
exit codes, timings and per-binary logs. Any failure or timeout fails the gate;
the receipt explicitly does not certify full EDH.

## Downstream boundary

Cybernetics' existing `magic-multiplayer-commander-v1` still grades the legacy
Python mini-Commander engine. It is not migrated by this patch. Its separate
`commander_edh` capability is fail-closed, with a regression test preventing
legacy fixture support from authorizing full EDH. Do not regenerate or promote
EDH packages, scores, traces or engine pins until the native migration gate is
actually green. Existing VisualsBench frozen artifacts are unchanged.
