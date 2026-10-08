Implement the candidate-owned Grifter's Blade definition and bindings in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/grifters_blade.rs`.

The RAV #263 contract is a colorless `{3}` Artifact — Equipment with Flash. Its controller may choose a creature they control as it enters and attach it to that creature; this is a non-targeting as-enters choice, not an ETB trigger, target, or response window. If declined or no legal creature exists, it enters unattached. The equipped creature gets `+1/+1`; Equip `{1}` attaches at sorcery speed to a creature its controller controls.

The trusted authority casts the Blade during the opponent's upkeep, chooses a controlled creature while the spell is suspended at entry, then pays `{1}` to move it onto a different controlled creature. It checks the decision actor and ordering, both recipients' +1/+1 results, deterministic replay, and mutations. Direct noncast effects that put the Equipment onto the battlefield are outside the current resumable-choice substrate and are not part of the task's fidelity claim.

Only the declared candidate module is writable. The task awards no reward or card credit.
