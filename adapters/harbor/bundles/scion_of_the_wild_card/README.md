# Scion of the Wild task bundle

This bundle evaluates RAV Scion of the Wild (#182): a green 0/0 creature costing `{1}{G}{G}` whose power and toughness each equal the number of creatures you control. Harbor reads `varieties/magic/tasks/rav-scion-of-the-wild-pvp/harbor.toml` for the isolated route.

The trusted trace casts Scion as the only controlled creature, verifies 1/1, then casts Watchwolf and verifies the live update to 2/2. It preserves typed spell and combat events, replays deterministically, detects removal of the continuous effect, and remains diagnostic-only at 1/291 with zero reward.
