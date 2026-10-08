# Magic set-PvP Harbor bundle

The public agent workspace is built by `materialize_char_task.py` and contains
the compile-valid Char stub plus shown RAV fixtures. The submitted change is
restricted to the Char module. Harbor checks workspace hashes, then compiles
and runs the candidate in an offline Docker worker with no host credential or
heldout-data mount.

The Codex route requires the shared runner's Linux UID isolation and probes
that its agent UID cannot read the host checkout or Git metadata. It stops
before launch if that boundary cannot be proven. The task-side smoke writes a
zero reward; only the trusted post-submission Docker scorer can award pilot
credit.

This pilot carries a 291-name RAV denominator and 306 printing count. A Char
pass is useful pilot evidence; it does not make the RAV set complete. GPT and
DIS are outside this task and remain manifested, non-executable sets.
