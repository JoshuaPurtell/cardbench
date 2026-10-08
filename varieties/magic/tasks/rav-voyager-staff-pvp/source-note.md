# Voyager Staff source note

Ravnica: City of Guilds #274 is a colorless `{1}` Artifact. Its printed activated ability costs `{2}` and sacrificing Voyager Staff, exiles target creature, and returns that creature to play under its owner's control at end of turn. Current Oracle uses “Exile target creature” and returns the exiled card to the battlefield under its owner's control at the beginning of the next end step. The compatibility trace preserves the RAV target, owner, and next end-step behavior; it records the wording modernization rather than substituting a different effect. The engine represents the linked target and delayed return as a typed, exact-incarnation group. Tokens are outside this task trace.

Public card reference: https://mtg.wtf/card/rav/274/Voyager-Staff

Retained facts only: public card name, set and collector number, colorless `{1}` cost, Artifact type, `{2}` activation, sacrifice cost, creature target, exile zone, owner-controlled return timing, and identity of the exact linked exile incarnation. No artwork, scan, flavor text, or bulk card-text corpus is retained.
