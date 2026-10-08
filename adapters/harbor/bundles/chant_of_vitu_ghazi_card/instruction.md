# RAV Chant of Vitu-Ghazi (#7)

Implement the candidate-owned module `definition() -> CardDefinition` for the
white Instant with exact `{6}{W}{W}` cost. Its Convoke payment reduces generic
cost only. When it resolves, it prevents damage that would be dealt by
creatures for the rest of the turn; its controller gains life only as damage
is actually prevented by this effect. Preserve the public function signature
and edit only this card module.

The full-game diagnostic verifies the colored payment, Convoke contributors,
typed ordered cast/resolution/prevention/life-gain/damage/end events, and the
terminal match. It is diagnostic-only with zero card credit and zero reward.
