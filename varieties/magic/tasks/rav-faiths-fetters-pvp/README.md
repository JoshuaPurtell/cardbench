# Faith's Fetters full-game card task

Implement the candidate-owned definition and mandatory enters-the-battlefield ability in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/faiths_fetters.rs`. Faith's Fetters is a white `{3}{W}` Aura that enchants a permanent, stops that permanent from attacking or blocking, suppresses its nonmana activated abilities, and gains its controller four life when it enters.

The isolated authority casts it on an opposing Selesnya Guildmage in a shuffled 60-card two-player game. It checks the exact `{3}{W}` payment and attachment, live restrictions, atomic rejection of the Guildmage activation plus attack and block declarations, the mandatory four-life trigger, deterministic replay, and a terminal P0 combat win. The task reports one identity among 291 RAV names and 306 printings with no card credit or reward.
