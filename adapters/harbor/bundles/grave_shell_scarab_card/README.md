# Grave-Shell Scarab card bundle

This isolated diagnostic task covers only RAV #211, Grave-Shell Scarab. The submission target is `varieties/magic/sets/ravnica_city_of_guilds/src/cards/grave_shell_scarab.rs`.

The trusted PvP authority checks the printed 4/4 black-green creature, `{2}{B}{G}{G}` cost, Dredge 1, and `{1}, Sacrifice this creature: Draw a card.` It exercises both using the sacrificed Scarab's own Dredge 1 and declining Dredge for the normal draw, then requires a deterministic terminal PvP game. This diagnostic grants no card credit or reward.
