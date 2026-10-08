# Elvish Skysweeper card task

This single-card RAV task implements Elvish Skysweeper (#162) in its candidate-owned Rust module. The normal shuffled PvP authority checks its green `{G}` 1/1 body and `{4}{G}, Sacrifice a creature: Destroy target creature with flying` ability, then follows a real activation and target through the rest of the game to a terminal result.

The task exposes only the candidate module for editing. Its reference authority, task metadata, and scoring scripts remain trusted infrastructure. Reference diagnostics award no credit and no reward. The Harbor runner calls the isolated offline scorer and does not run provider-backed trials by itself.
