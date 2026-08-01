# RAV worktree reconciliation

This is a public, source-to-`dev` reconciliation record for the temporary
RAV implementation worktrees. It preserves an honest distinction between code
that was safely ported and a source-lane approximation that was deliberately
not imported. Removing a clean worktree does not delete its branch; the branch
remains available for a later, audited red-to-green port.

## Black lane (`magic/okr-black`)

- The safe Moonlight Bargain coverage delta is integrated on `dev` as red
  `a3a89a2e`, green `b9082950`, and closure `03debeaf`. The dev implementation
  intentionally replaces the source lane's pre-resolution hidden-card choice
  with a stack-suspension, controller-private, no-priority boundary.
- Thoughtpicker Witch is now ported on `dev` as red `7f1a6526` and green
  `62eccace`. The current implementation preserves its generic activation
  cost, but replaces the source lane's obsolete binding/fixture path with an
  activated-ability stack suspension. Candidate identities are projected only
  to the activating controller; the public log records the opening metadata,
  legal exile result, and terminal ability receipt without disclosing the
  opponent-library snapshot.

## Green lane (`magic/okr-green`)

- Dowsing Shaman is ported on `dev` as red `752b565d` and green `53a21599`.
  The current implementation deliberately corrects the historical source
  branch's `{3}{G}` activation-cost drift to `{2}{G}`, and uses a typed,
  controller-owned enchantment-card graveyard target with current stack and
  invariant APIs.
- The dev branch already contains the safe Vinelasher Kudzu controller-land
  trigger, with controller-relative provenance. The source lane's generic
  land-entry trigger is not imported because it would also fire for an
  opponent's land.
- Ivy Dancer is ported on `dev` as red `fe25539b` and green `e1af2bb2`.
  The source controller-wide, green-mana approximation was not merged. The
  current implementation instead uses a zero-mana tap activation that targets
  exactly one creature, grants typed Forest landwalk until end of turn, and
  records declaration-time landwalk provenance for blocker legality.
- Chord of Calling and Doubling Season remain catalog-only on `dev`. Their
  source implementations rely on an older variable-search/replacement model
  that has not passed the current state-machine and information-boundary
  audit. They must be ported through new red-to-green milestones rather than
  cherry-picked.
- The other source-only literal definitions are represented on dev as bounded
  chassis where their source-lane full claims rely on obsolete trigger or
  choice behavior. Their individual printed ability gaps stay explicit in the
  catalog and coverage ledger.

## Multicolor lane (`magic/okr-multicolor`)

No literal RAV card definition exists only on this branch. Its safe
front-face, stack, and land work has already been reconciled into dev. The
branch remains as a source-history reference until the final branch-retention
decision; its clean worktree can be removed without losing commits.

## White lane (`magic/okr-white`)

- Seed Spark and Leave No Trace are ported on `dev` as red `b3e3f07e` and
  green `0a98be19`. The current implementation uses existing stack target
  legality for Seed Spark and adds the expansion-neutral typed-enchantment
  Radiance destruction substrate needed by Leave No Trace; it does not import
  the obsolete source engine wholesale.
- Remaining White source-only definitions are not silently considered merged:
  Auratouched Mage, Bathe in Light, Blazing Archon, Boros Fury-Shield,
  Caregiver, Chant of Vitu-Ghazi, Concerted Effort, Conclave's Blessing,
  Faith's Fetters, Festival of the Guildpact, Flickerform, Gate Hound, Ghosts
  of the Innocent, Hour of Reckoning, Hunted Lammasu, Light of Sanction,
  Loxodon Gatekeeper, Oathsworn Giant, Suppression Field, Three Dreams,
  Twilight Drover, Veteran Armorer, and Wojek Apothecary. Each requires its
  own current-engine red-to-green port; the legacy branch's positive markers
  are not evidence that its behavior is live on `dev`.

## Deletion rule

Before removing a temporary worktree, verify its `git status --short` is
empty, leave its named branch intact, and ensure `dev` has passed the focused
contract, full workspace suite, strict Clippy, formatting check, and RAV
parity gate applicable to the latest dev integration. A worktree removal is
then only disk cleanup; it is reversible with `git worktree add` from its
retained branch.
