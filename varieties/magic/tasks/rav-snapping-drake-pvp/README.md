# RAV Snapping Drake single-card PvP pilot

This isolated card task qualifies Snapping Drake (RAV collector #64) through a
normal two-player constructed match. Seat zero uses a task-local 60-card deck
with four Snapping Drakes and 56 Islands, facing the public Boros Aggro
60-card deck with a deterministic pass-only opponent policy. The authority
requires the legal `{3}{U}` cast, exact blue mana payment, stack resolution,
battlefield entry, live 3/2 Flying characteristics, and a later legal attack
after summoning sickness expires before the match reaches a clean terminal
result. It replays the same seed and runs cost, power,
toughness, and Flying mutations through the same game and trace oracle.

The task source is the isolated
`src/cards/snapping_drake.rs` definition. Before it is evaluated, the
authority restricts the source AST to a pure declarative card literal and
checks card identity. Printed characteristics are checked after the submitted
definition is substituted into the complete RAV rules registry and the full
game is played. The report retains the complete 291-name / 306-printing RAV
denominator. Snapping Drake is the only pilot pass; credited cards, Harbor
reward, and whole-set coverage stay at zero or false. Independent review and a
real candidate trial remain pending.

Run the materializer with `--workspace PATH --manifest PATH`; verify with the
same arguments plus `--verify`. Run the scorer with `--workspace PATH
--manifest PATH --output-root PATH --candidate-kind reference|agent` after the
authority bin is registered as `rav-snapping-drake-pvp` in the session Cargo
manifest. The scorer mounts that verifier read-only after submission and
validates its report outside the candidate process.
