# CardBench Magic single-card task: Twilight Drover

Implement the RAV Twilight Drover card module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/twilight_drover.rs`. Provide the candidate-owned card definition, mandatory triggered-ability binding, activated-ability binding, and generalized activated-cost binding.

The printed card is a white `{2}{W}` 1/1 Spirit. A creature token leaving the battlefield puts one +1/+1 counter on Twilight Drover. Its `{2}{W}` activated ability removes a +1/+1 counter from this source as a cost and creates two 1/1 white Spirit creature tokens with flying. A simultaneous nontoken creature departure must not trigger the counter ability.

The public verifier uses a legal 60-card PvP game that casts Twilight Drover, Scatter the Seeds, and Peel from Reality. Peel returns one controlled Saproling token and an opponent-controlled Watchwolf together; only the token departure triggers Drover. The source counter is then paid to activate Drover, and both generated tokens must remain live white 1/1 Flying Spirits. The game reaches a terminal win and must replay deterministically.

The printed body subtype Spirit is documented but is not asserted through `CardDefinition`, whose schema lacks an ordinary creature subtype field. The token subtype is represented and checked. Current Oracle terminology differs from the 2005 phrasing as described in the task source note.

This is diagnostic-only: the report preserves the 291-name / 306-printing RAV denominator and awards zero card credit and reward.
