# Implement Sadistic Augermage

Complete the candidate-owned card module for Ravnica: City of Guilds #103, Sadistic Augermage.

Implement the black `{2}{B}` 3/1 Creature — Human Wizard. Its required trigger comes from Sadistic Augermage moving from the battlefield to the graveyard. During resolution, open private hand selections for the two players in APNAP order, then move the chosen objects to the tops of their respective owners' libraries. Treat this as a library placement, not a discard.

Keep the implementation declarative in the existing card-module API. The task authority checks the printed characteristics, trigger event, mandatory status, both players' private choice boundaries, ownership, library ordering, and semantic mutations.
