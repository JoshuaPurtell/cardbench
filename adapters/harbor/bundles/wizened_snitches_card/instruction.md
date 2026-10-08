Implement the RAV Wizened Snitches candidate module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/wizened_snitches.rs`.

Preserve its printed `{3}{U}` cost, 1/3 creature characteristics, Flying, and global static effect that reveals the top card of each player's library to every player while Wizened Snitches is on the battlefield. The provided offline scorer runs a shuffled two-player game, verifies both views, removes the source with Last Gasp, checks visibility revocation, and requires the terminal game, replay, and mutation gates to pass.
