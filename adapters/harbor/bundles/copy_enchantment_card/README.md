# Copy Enchantment card task

Implement the RAV #42 Copy Enchantment definition in the only writable card module. Preserve its blue `{2}{U}` cost and Enchantment type. Its typed entry-copy binding is already supplied by the RAV set integration.

The authority runs a normal shuffled two-player duel. It requires one Copy Enchantment to copy an existing Fists of Ironwood, the copied Aura to attach to Watchwolf, a second copy choice to be declined, and the copied Aura's own ETB trigger to resolve. It checks ordered public events, object incarnations, and a full-game win. The task report remains uncredited and has zero reward.

Run the materializer to produce the starter workspace. The task test invokes the isolated scorer against that candidate and rejects modifications outside the target file.
