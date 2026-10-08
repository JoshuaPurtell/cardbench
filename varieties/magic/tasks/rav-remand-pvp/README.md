# RAV Remand single-card PvP task

This task exercises Remand (RAV collector #63) in a shuffled two-player,
60-card constructed game. Seat zero casts Remand for `{1}{U}` in response to
the exact Watchwolf spell cast by seat one. The trace requires the spell to be
countered by Remand, returned to its owner's hand, and followed by one card
drawn by Remand's controller. The game continues to a clean seat-zero combat
win and replays deterministically from the same seed.

The trusted authority checks the printed instant identity and validates the
candidate module as a literal-only definition. It also reruns the gameplay
trace with the target sent to the wrong zone, the draw removed, and the counter
removed. Each mutation must fail a distinct gameplay assertion. The materialized
starter is compile-valid and intentionally uses the wrong counter destination.

The report retains the full RAV denominator of 291 identities and 306
printings. Remand is the single pilot pass; reward and credited-card counts
remain zero, and whole-set coverage remains incomplete. A candidate trial and
independent review have not been run.

Create and verify a public workspace with
`varieties/magic/scripts/materialize_remand_task.py`. The scorer's
`--report-file` mode validates a local authority report without starting a
candidate worker. Harbor scoring after submission uses a networkless,
resource-limited Docker worker and a separately mounted trusted authority.
