# Grifter's Blade — RAV #263

Implement Grifter's Blade in the candidate module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/grifters_blade.rs`.

The printed contract is a colorless `{3}` Artifact — Equipment with Flash. As it enters, its controller may choose a creature they control; if they do, it enters attached to that creature. This is a non-targeting as-enters choice: it does not create a triggered ability or a response window. If the controller declines or has no legal creature, the Equipment enters unattached. The equipped creature gets `+1/+1`, and Equip `{1}` can attach it to a creature its controller controls at sorcery speed.

The authority casts the Blade during the opponent's upkeep, selects a controlled creature at the permanent-entry decision, then pays `{1}` at sorcery speed to move the Equipment to a different controlled creature. The trace must show the choice actor, the choice before `SpellResolved` and battlefield entry, immediate attachment before entry triggers, the `+1/+1` layer, deterministic replay, semantic mutations, and terminal continuation.

`CardDefinition` represents `Artifact` but does not store printed artifact subtypes. The task retains Equipment as public card identity and asserts its runtime Equipment behavior through the attachment binding.

The current engine continuation handles permanent spells resolving from the stack. Noncast effects that put Grifter's Blade directly onto the battlefield are outside this task's supported entry paths and are not claimed by the authority.

Candidate code is the only writable workspace file. The scorer reports zero reward and zero card credit.

## Run locally

After the shared entry-attachment substrate, registry, and session target are integrated, use the task materializer and scorer:

```sh
python3 varieties/magic/scripts/materialize_grifters_blade_task.py \
  --workspace /tmp/grifters-blade-workspace \
  --manifest /tmp/grifters-blade-manifest.json
python3 varieties/magic/scripts/score_grifters_blade_pvp.py \
  --workspace /tmp/grifters-blade-workspace \
  --manifest /tmp/grifters-blade-manifest.json \
  --output-root /tmp/grifters-blade-score
```

The authority target is `rav-grifters-blade-pvp`. Cargo execution, scoring, and Harbor trials are serialized outside this authoring task.
