# RAV Blazing Archon (#4)

Implement the candidate-owned module `definition() -> CardDefinition` and
`static_attack_restriction_binding() -> StaticAttackRestrictionBinding` for
the white 5/6 Creature with Flying and `{6}{W}{W}{W}`. The static restriction
must keep opponents from declaring attacks against its controller while the
Archon is on the battlefield. Preserve the function names and signatures; edit
only this card module.

The full-game diagnostic verifies exact payment, an opposing attack before the
Archon enters, a typed empty opposing attacker declaration while it is live,
and a terminal win from Archon combat. The report is diagnostic-only with zero
card credit and reward.
