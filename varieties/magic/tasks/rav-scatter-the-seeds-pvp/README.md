# RAV Scatter the Seeds single-card PvP diagnostic

This task evaluates Scatter the Seeds (RAV #181) in a complete, legal shuffled
two-player game. The casting player pays `{3}{G}{G}` with three green mana and
two distinct creatures tapped for generic Convoke. The resolving instant must
create exactly three green 1/1 Saproling creature tokens. The authority checks
their live token specifications and characteristics, then attacks with them
and requires their combat damage to finish the duel.

The trace validates candidate/registry identity, ordered Convoke and mana
payment, cast, resolution, graveyard movement, token creation, attack, damage,
terminal winner, and deterministic replay. Compiled mutants remove Convoke,
increase the generic cost, reduce token quantity, or alter the Saproling body;
all must be detected. This is diagnostic-only: one of 291 unique RAV names is
checked against 306 printings, with zero card credit, zero reward, and no full
set coverage.

The rules snapshot is effective August 7, 2026, at the September 24, 2026
authority cutoff. No provider-backed trial is part of this package.

Materialize with `materialize_scatter_the_seeds_task.py --workspace PATH
--manifest PATH`; verify using the same arguments and `--verify`. The only
candidate-editable file is `src/cards/scatter_the_seeds.rs`.
