# CardBench Magic single-card task: Thundersong Trumpeter

Implement the RAV Thundersong Trumpeter definition in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/thundersong_trumpeter.rs`. Preserve the existing literal-only declarative definition and activated-ability binding API.

The trusted verifier plays a shuffled 60-card two-player constructed match, checks the exact red-white casting payment and resolved 2/1 characteristics, then uses the tap activation on a legal opposing creature. The ordered typed events and live state must show the source tapped, a temporary combat restriction resolving on the target, and that target unavailable as a blocker. The game continues through combat to a terminal seat-zero win. This is a diagnostic trace against the 291-name/306-printing RAV catalog and awards no set coverage or reward.
