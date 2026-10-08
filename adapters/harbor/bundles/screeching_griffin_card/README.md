# Screeching Griffin card task

This bundle evaluates the RAV Screeching Griffin implementation in a legal seeded 60-card PvP game. The trusted trace checks its printed `{3}{W}` cost and Flying body, casts a real opposing Flying Skyknight, activates Griffin's red-mana ability against that target, observes the resolved restriction, challenges the block, and requires the match to continue through combat to a terminal result. The candidate owns both its card definition and ability binding.

The task is a diagnostic pilot for one of the 291 RAV card identities. A pass does not establish whole-set coverage or Harbor reward. The reference lane includes a targeted semantic mutation replacing the restriction with an unrelated target +1/+1 effect, plus a compile-valid stub gate; candidate execution uses the bounded generic Harbor worker.
