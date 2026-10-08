# Gate Hound full-game card task

Implement the candidate-owned Gate Hound definition and static effect in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/gate_hound.rs`. Gate Hound is a white `{2}{W}` 1/1 creature. While Gate Hound is enchanted, creatures its controller controls have vigilance.

The isolated authority casts Gate Hound and attaches Fists of Ironwood in a shuffled 60-card two-player game. It checks that no creature receives vigilance before the Aura attaches, that Gate Hound and the Aura-created Saprolings all receive vigilance while it is attached, that they attack without tapping, and that the full game reaches a terminal P0 combat win. Replay and semantic mutations are checked in the trusted authority. The task reports one identity among 291 RAV names and 306 printings with zero credit and zero reward.
