# RAV Twilight Drover single-card diagnostic

This task covers Twilight Drover (RAV collector #33), a white `{2}{W}` 1/1 creature. Its mandatory ability puts a +1/+1 counter on it whenever a creature token leaves the battlefield. Its `{2}{W}` activated ability removes one +1/+1 counter from itself and creates two 1/1 white Spirit creature tokens with flying.

The reference is a legal constructed 60-card PvP game. It casts Twilight Drover, creates Saprolings, and returns one token and one opposing nontoken creature simultaneously. The event trace must show exactly one Drover trigger for the token departure. That counter is then paid as the activation cost, and the activation creates exactly two live white 1/1 Flying Spirit tokens. The match continues to a clean terminal result and is replayed deterministically.

The candidate boundary is `varieties/magic/sets/ravnica_city_of_guilds/src/cards/twilight_drover.rs`. Implement its definition, triggered ability, activated ability, and generalized activated-cost binding as literal factories. The public materializer replaces only that module. The candidate module can express the required token-only event condition once the shared RAV engine substrate is integrated; until then its compile gate is intentionally pending.

The RAV printing says “leaves play” and “into play”; current Oracle uses “leaves the battlefield” and “create.” These are terminology updates for the operations modeled here. The printed creature subtype Spirit is retained in this task note, but the current `CardDefinition` schema does not represent ordinary creature subtypes. The created Spirit tokens are typed and checked.

The report compares the RAV catalog's 291 unique names and 306 printings. This is an unreviewed diagnostic: card credit, Harbor reward, and pilot reward remain zero.
