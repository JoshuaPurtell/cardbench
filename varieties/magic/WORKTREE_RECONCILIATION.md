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
- Thoughtpicker Witch remains the existing bounded creature chassis on `dev`.
  The source lane's proposed activation is not imported: it uses a generic-only
  cost representation, exposes opponent-library candidate identities through
  a public receipt, and its fixture injects mana after `begin_game`. Each
  conflicts with the current dev cost, hidden-information, and live-state
  transition contracts. A future port needs an opponent-library private-choice
  suspension, not deterministic exile or public candidate disclosure.

## Green lane (`magic/okr-green`)

- The dev branch already contains the safe Vinelasher Kudzu controller-land
  trigger, with controller-relative provenance. The source lane's generic
  land-entry trigger is not imported because it would also fire for an
  opponent's land.
- The source Ivy Dancer activation applies a controller-wide effect rather
  than preserving an explicit target. It is not a safe promotion of dev's
  bounded chassis.
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

## Deletion rule

Before removing a temporary worktree, verify its `git status --short` is
empty, leave its named branch intact, and ensure `dev` has passed the focused
contract, full workspace suite, strict Clippy, formatting check, and RAV
parity gate applicable to the latest dev integration. A worktree removal is
then only disk cleanup; it is reversible with `git worktree add` from its
retained branch.
