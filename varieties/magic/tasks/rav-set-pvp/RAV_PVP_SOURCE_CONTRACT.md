# RAV per-card PvP source contract

## Rules baseline

Per-card PvP traces use the official [*Magic: The Gathering Comprehensive
Rules* effective August 7, 2026](https://media.wizards.com/2026/downloads/MagicCompRules%2020260807.pdf),
the latest effective edition as of this task's September 23, 2026 cutoff. Check
the official [rules page](https://magic.wizards.com/en/rules) for the current
document. Apply only rules changes effective by the cutoff; future-dated update
bulletins are not part of this baseline. Rules questions must be resolved
against the official Comprehensive Rules, not inferred from engine behavior.

## Card semantics

The compatibility target is each gameplay identity as printed in **Ravnica:
City of Guilds**. Use the RAV printing for its public identity and functional
branches. Current Oracle wording may be consulted to interpret rules questions,
but it does not silently replace the original printing; any reconciliation or
excluded later erratum must be recorded in the card's source note. A set
printing that shares a gameplay identity with another printing contributes to
the printing denominator but does not create another card identity.

Retain only the public identity facts and the CardBench-authored semantic
operations needed by the tested trace. Do not commit card rules prose, scans,
art, flavor text, upstream response payloads, or a full card-text corpus.
`DATA_PROVENANCE.md` records the source checks and retained fields.

## Coverage and credit

The inventory denominator is 291 distinct RAV gameplay identities and 306
printings. For a card to pass a PvP trace, a legal two-player constructed game
must reach a terminal result, actually use the card, and assert its relevant
ordered public events and post-event state through the typed session
projection. Add separate traces where one game cannot exercise meaningful
choices, response windows, random outcomes, invalid actions, or cross-card
interactions. A catalog row, a deck inclusion, a focused fixture, or another
card's trace is not per-card PvP evidence.

Keep card, mechanic-family, set-engine, and whole-set results separate. A
reference trace is diagnostic until its card-specific semantic mutation,
compile-valid stub rejection, independent review, and real candidate trial
have all passed and their evidence is retained. Missing cards stay in the
denominator; set coverage remains incomplete until every required identity is
credited.

Each task's independent-review manifest must bind the catalog denominator,
the engine and session files that produce its event and state facts, the card
definition, task feature summary, materializer, scorer, and Harbor bundle. The
candidate workspace must omit per-card task directories, scorer and
materializer scripts, and trusted PvP authority code; registered authority
binaries in that workspace are fail-closed stubs. The trusted trace and mutation
oracles remain in Harbor's reference lane.

Every task-owned mechanic must live in the candidate-owned card module. This
includes card-specific activated, triggered, static, replacement, and other
runtime bindings, not only the `CardDefinition` literal. The set registry must
delegate those bindings to the card module so materialization can replace the
whole card implementation with a compile-valid stub. Leaving a target card's
working binding inline in the shared set registry exposes its answer in the
candidate workspace and invalidates the task boundary.
