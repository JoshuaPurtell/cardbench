# RAV Commander dynamics

The requested scope is four-player free-for-all Commander over the executable
Ravnica: City of Guilds pool, not arbitrary-card EDH. The trusted catalog must
pin card legality and full color identity; submitted metadata cannot expand it.
Use legal 100-card singleton decks, one of the eight eligible RAV commanders,
40 starting life, normal shuffled libraries and opening hands. No pre-seated
combat puzzle is an admitted benchmark workload.

Required dynamics: command-zone casting and escalating additional tax;
owner-selected graveyard/exile SBA returns and hand/library replacements;
21 combat damage tracked by commander identity across control/zone changes;
London mulligans; multiplayer priority, combat, triggered abilities, state-based
actions and player departure; and interactions reachable through RAV cards.

The existing engine regression covers draw/dredge, search, bounce, recycling,
Warp World, exile/linked return, control/copy and simultaneous-action paths.
The full-deck reference harness must exercise both four-commander rosters, not
only Tolsimir/Savra/Szadek/Agrus Kos. Rules interaction tests may construct
specific executable-card states, but are not benchmark worlds or policy gold.

The audit found Remand incorrectly using ordinary graveyard countering. Its
dedicated counter-to-hand instruction now preserves the physical spell's owner
choice before the draw suffix. A stack-origin replacement is distinct from a
player zone. Tests cover accept/decline, stale and wrong-player responses,
single counter/draw receipts, and recasting from hand versus command zone.
Replay validation requires an explicit matching counter-to-hand marker; normal
counter receipts cannot silently change their terminal destination.

The expanded roster includes the entire 291-card pool across eight decks,
including nonbasic lands. Deck inclusion is not a claim that every card or
interaction executed in those games. It exposed a repeated trigger flush that
could overwrite a pending APNAP ordering choice; placement now waits for both
ordering and targeting continuations. Public target decisions also provide one
legal ordered completion, preserving Spawnbroker's controller and power
constraints instead of treating its flat candidate union as a legal pair.

Printed-characteristic verification against real RAV records caught and
corrected 36 entries (including Induce Paranoia, Sisters of Stone Death,
hybrid costs, and Chord of Calling's zero-generic base before X). Induce
Paranoia now costs 2UU and gates its mill suffix on black mana spent; tests
cover both payment outcomes. The native catalog snapshot fails closed on
cost/color/numeric-stat mismatches. This metadata gate does not prove every
Oracle-text interaction; dynamic star characteristics remain rules tests.

Card-type checks also corrected Blockbuster to an enchantment and Cleansing
Beam/Dryad's Caress to instants. Focused printed-effect regressions replace
Blockbuster's spurious repeatable all-creature activation with its 1R sacrifice
and tapped-creature damage; Dryad's Caress counts battlefield creatures and
conditionally untaps the controller's creatures on white spend. Root-Kin Ally
has its printed tap-two-creatures pump, not invented ETB contributor counters;
Conclave Phalanx counts all controlled creatures, not only white ones.
Simultaneous dies triggers remain captured until Commander-return SBA choices
finish, before APNAP ordering begins.

Rules source: Wizards' [Ravnica Remastered release notes](https://magic.wizards.com/en/news/feature/ravnica-remastered-release-notes),
Remand entry, and Comprehensive Rules 903.

Out of scope for this pool: meld/mutate, commander pairs, companions,
opening-hand special abilities, planeswalker/battle defenders, and other
mechanics absent from the admitted RAV definitions. Two-Headed Giant is a
separate format, not a prerequisite for this free-for-all lane.

This bounded scope does not grant `CommanderEdh` arbitrary-card certification
or automatically promote Cybernetics packages. Runtime/reference evidence and
workload authority remain separate; assisted-menu transport checks are not
strategic policy gold.
