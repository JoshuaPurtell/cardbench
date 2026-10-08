# RAV Courier Hawk single-card PvP pilot

This isolated task qualifies Courier Hawk (RAV collector #12) through a normal
two-player, 60-card constructed match. The Selesnya midrange curve deck contains
four copies and faces the Boros Aggro Hawk deck, piloted by a deterministic
pass-only baseline. The authority requires a legal `{1}{W}` cast, stack
resolution, battlefield entry, payment of white plus one generic mana, and the
live 1/2 Flying/Vigilance characteristics. It then attacks with Courier Hawk
on a later turn and checks from the ordered `AttackersDeclared` event and live
permanent state that it remains untapped.

The candidate source is `src/cards/courier_hawk.rs`. Before evaluation, the
authority restricts the Rust AST to a declarative card literal and only allows
the card's known keyword names. It deliberately allows omission of either
keyword and characteristic-value changes so those mutants reach the game and
are detected in the resolved permanent's live state. The trace replays its
selected seed and reports the complete 291-name / 306-printing RAV denominator.
Courier Hawk is the sole pilot pass; credited cards, Harbor reward, and whole
set coverage remain zero or false. A real candidate trial and independent
review are pending.

Run `materialize_courier_hawk_task.py --workspace PATH --manifest PATH`, then
verify with the same arguments plus `--verify`. Run
`score_courier_hawk_pvp.py --workspace PATH --manifest PATH --output-root PATH
--candidate-kind reference|agent` after registering the authority binary
`rav-courier-hawk-pvp` in the session Cargo manifest. The pinned rules snapshot
is the Comprehensive Rules effective 2026-08-07.
