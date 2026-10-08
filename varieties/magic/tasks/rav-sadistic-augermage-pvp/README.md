# Sadistic Augermage RAV card task

Implement RAV #103, Sadistic Augermage: a black `{2}{B}` 3/1 Creature — Human Wizard. Its mandatory death trigger asks both players for a private selection from their own hands, then places each chosen card atop its owner's library without discarding it.

The isolated two-player authority makes Augermage die to a state-based action, then resolves its mandatory trigger. It checks private hand choices in APNAP order, verifies that each chooser sees only their own eligible hand cards, confirms the first selected card stays in hand until the second player has chosen, and checks both selected cards become their owners' library tops without being discarded. The fixture also preserves an earlier library card and an unchosen hand card for each player. Mutation checks cover identity and trigger semantics. This diagnostic awards no card credit or reward.
