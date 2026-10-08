# RAV Loxodon Gatekeeper single-card task

This isolated PvP task qualifies Loxodon Gatekeeper as the RAV white 2/3 creature at collector number 25. The authority plays a legal sixty-card constructed match, pays `{2}{W}{W}`, resolves the Gatekeeper, and records an opponent Plains entering tapped with a receipt tied to the exact live Gatekeeper object. The pilot then wins through creature combat; the opponent follows a deterministic policy that plays its Plains and otherwise passes.

The rules snapshot is the Wizards Comprehensive Rules effective August 7, 2026, the last effective release at the September 23, 2026 authority cutoff. The trace covers colored casting costs, priority, permanent spell resolution, and replacement effects that change how an opposing permanent enters the battlefield. The official rules PDF is <https://media.wizards.com/2026/downloads/MagicCompRules%2020260807.pdf>.

Candidate-owned source is `src/cards/loxodon_gatekeeper.rs`. Its declarative API exports the `CardDefinition` and the source-bound `StaticEntryRestrictionBinding`; the RAV registry delegates both to this module. The authority checks the printed identity and the live entry-tapped witness. This diagnostic pass does not credit the card, reward, or set coverage; independent review, receipt retention, and a real candidate trial remain pending.

Run the materializer with `--workspace PATH --manifest PATH`; verify the result with those arguments plus `--verify`. Run the local authority with `cargo run --locked --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin rav-loxodon-gatekeeper-pvp`. The Harbor scorer and trusted authority remain in the reference lane.
