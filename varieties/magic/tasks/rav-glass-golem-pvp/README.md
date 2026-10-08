# RAV Glass Golem single-card PvP pilot

This isolated card task qualifies Glass Golem (RAV collector #261) through a
normal two-player constructed match. The shown Selesnya midrange deck replaces
its Watchwolf slot with Glass Golem and faces the shown Boros Aggro 60-card
deck piloted by a deterministic pass-only baseline. The authority requires an
exact five-generic-mana cast, stack resolution, battlefield entry, and live
6/2 artifact-creature characteristics before the game reaches a clean terminal
result.

The rules snapshot is the Wizards Comprehensive Rules effective August 7,
2026, the last effective release at the September 23, 2026 authority cutoff.
The trace checks CR 105.2 and 202.1–202.2 for color and generic costs, 117 for
timing and priority, 208.1 and 302.4c for printed creature power and toughness,
301, 302.1–302.2 and 601.2 for casting and artifact creature resolution, 608.3a for
permanent entry, and 613 for continuous effects. The official rules PDF is
<https://media.wizards.com/2026/downloads/MagicCompRules%2020260807.pdf>.

The task source is the isolated `src/cards/glass_golem.rs` definition. Before it
is evaluated, the authority restricts the source AST to a pure declarative card
literal and requires that definition to equal the card definition used by the
RAV game registry. The task report retains the complete 291-name / 306-printing
RAV denominator. Glass Golem is the only pilot pass; credited cards, Harbor
reward, and whole-set coverage stay at zero or false. Independent review and a
real candidate trial remain pending.

Run the materializer with `--workspace PATH --manifest PATH`; verify with the
same arguments plus `--verify`. Run the scorer with `--workspace PATH
--manifest PATH --output-root PATH --candidate-kind reference|agent` after the
authority bin is registered as `rav-glass-golem-pvp` in the session Cargo
manifest. The scorer mounts that verifier read-only after submission and
validates its report outside the candidate process.
