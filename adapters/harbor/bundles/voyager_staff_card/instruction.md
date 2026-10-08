Implement the RAV Voyager Staff module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/voyager_staff.rs`.

Preserve RAV #274: colorless `{1}` Artifact. Its `{2}`, sacrifice-this-artifact ability exiles target creature and returns it to its owner's battlefield at the beginning of the next end step. The printed RAV wording says “remove from the game,” “play,” and “at end of turn”; current Oracle wording modernizes those zone/timing terms. The trusted trace uses a non-token creature and verifies the exact target incarnation and owner-controlled return.

Keep the definition and activated ability in the candidate-owned module, with no executable top-level side effects. The trusted source guard checks the pinned normalized literal module before calling either candidate function and rejects a compile-valid side-effect fixture. All rewards and credit remain zero.
