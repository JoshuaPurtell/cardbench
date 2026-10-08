# Votary of the Conclave task

This diagnostic RAV single-card task evaluates Votary of the Conclave (#35), a white 1/1 creature that costs `{W}` and has `{G}: Regenerate target creature`.

The reference authority plays one shuffled, legal 60-card game per replay. It casts Votary for exactly one white mana, lets Brainspoil target the untapped Votary, responds with its targeted `{G}` ability, and checks that the regeneration shield is created and used, Votary becomes tapped but remains on the battlefield, and Brainspoil resolves without destroying it. Votary then attacks to a terminal win. The authority checks live characteristics and ordered typed payment, cast, resolution, battlefield-entry, activation, shield, and game-end events. It also checks deterministic replay and a compiled wrong-color regeneration mutant.

The report is diagnostic-only: 1 of 291 unique RAV names from 306 printings is trace-passed, zero cards are credited, and reward is zero. This package is not a provider trial.

Implement only `definition()` and `activated_ability()` in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/votary_of_the_conclave.rs`.
