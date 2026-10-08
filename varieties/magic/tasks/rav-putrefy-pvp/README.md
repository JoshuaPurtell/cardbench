# Putrefy RAV card task

Implement candidate-owned Putrefy, RAV #221. Its `{1}{B}{G}` instant destroys target artifact or creature, and that permanent cannot be regenerated.

The reference authority runs a shuffled, legal 60-card duel. It casts Putrefy at an opposing Glass Golem after Votary of the Conclave has placed a regeneration shield on it. The trace checks actual black, green, and generic payment, target identity, spell resolution, destruction despite the shield, no shield consumption, continued combat, deterministic replay, and a terminal seat-zero win. Gameplay mutations that remove destruction, allow regeneration, or change the mana cost must fail. Diagnostic reports award no card credit or Harbor reward.
