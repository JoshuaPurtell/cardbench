# RAV Plains single-card task

This isolated PvP task qualifies Plains as one RAV gameplay identity across collector numbers 289–290. The authority checks a sixty-card constructed deck with 52 Plains, records a Plains entering from its hand, taps that exact object for white mana, spends white to cast a RAV creature, and verifies the spell's resolution and battlefield entry before a clean seat-zero combat win. The opposing sixty-card Plains deck follows a deterministic pass-only policy.

The rules snapshot is the Wizards Comprehensive Rules effective August 7, 2026, the last effective release at the September 23, 2026 authority cutoff. The trace covers deck composition, basic land type, intrinsic mana activation, casting and payment, turn priority, and a full-game terminal result. The official rules PDF is <https://media.wizards.com/2026/downloads/MagicCompRules%2020260807.pdf>.

Candidate-owned source is `src/cards/plains.rs`. Its pure declarative interface exports both the `CardDefinition` and the typed `BasicLandTypeBinding`; the RAV registry delegates Plains's definition and type binding to this module. The authority rejects any implementation that changes Plains identity, land type, mana color, or characteristics. The RAV denominator remains 291 unique names and 306 printings. This diagnostic pass does not credit the card, reward, or set coverage; independent review, receipt retention, and a real candidate trial remain pending.

Run the materializer with `--workspace PATH --manifest PATH`; verify the result with those arguments plus `--verify`. Run the local authority with `cargo run --locked --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin rav-plains-pvp`. The Harbor scorer and trusted authority remain in the reference lane.
