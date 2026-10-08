# Bramble Elemental card task

Implement the candidate-owned Bramble Elemental definition and optional Aura-attachment trigger in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/bramble_elemental.rs`. The trace casts the green `{3}{G}{G}` 4/4, attaches one Moldervine Cloak to Bramble and another to a different creature, accepts the first trigger, and verifies exactly two green 1/1 Saprolings. Only attachment to Bramble itself produces that trigger; Aura controller identity does not control the trigger's controller.

The shuffled legal 60-card duel continues through the Saprolings' attack to a terminal pilot win. The trusted authority verifies exact object and incarnation receipts, cast payments, replay determinism, and semantic mutations for a removed trigger, the old controller-wide Aura-entry condition, the wrong token count, and an enter-the-battlefield condition.

The task covers one name in the RAV 291-name / 306-printing denominator. All diagnostic rewards and credits remain zero.
