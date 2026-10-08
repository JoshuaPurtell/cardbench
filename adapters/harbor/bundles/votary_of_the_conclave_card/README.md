# Votary of the Conclave task bundle

This bundle evaluates Votary of the Conclave (#35), a white `{W}` 1/1 creature with `{G}: Regenerate target creature`. Harbor reads `varieties/magic/tasks/rav-votary-of-the-conclave-pvp/harbor.toml` for its isolated single-card route.

The trusted authority casts Votary, lets Brainspoil target it, then responds by paying green to regenerate the targeted creature. It checks that the shield is used, Votary is tapped and remains on the battlefield, and Brainspoil does not destroy it. The trace is deterministic, diagnostic-only at 1/291, and awards zero reward.
