# Induce Paranoia RAV card task

Implement the candidate-owned module for RAV #56, Induce Paranoia. It is a `{2}{U}{U}` blue instant that counters its target spell and conditionally mills that spell's controller when black mana was spent to cast it. The trace uses a real opposing Carven Caryatid spell with mana value 3.

The trusted authority runs two legal shuffled 60-card PvP games. The black-spent game proves the targeted spell is countered, its owner receives exactly the three top library objects in their graveyard, and the same Induce Paranoia reaches its normal terminal zone. The no-black game counters the same target while retaining the target controller's library cards. Both continue to a terminal seat-zero win with a later Snapping Drake, assert the ordered projected events, replay deterministically, and check semantic mutations. No provider trial, candidate run, Harbor run, or credit is included in this package.

The candidate workspace replaces the entire candidate-owned definition module with a compile-valid stub. Its task folder, scorer, materializer, and trusted authority are excluded from the workspace. The authority validates the normalized source and its literal AST shape before it calls the candidate definition, and records rejection of an injected compile-valid side-effect candidate.
