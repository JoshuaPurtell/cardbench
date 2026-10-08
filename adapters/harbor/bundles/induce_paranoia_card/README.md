# Induce Paranoia card bundle

This bundle evaluates the isolated RAV #56 card-definition task. The candidate may edit only `varieties/magic/sets/ravnica_city_of_guilds/src/cards/induce_paranoia.rs`.

The card contract is the blue `{2}{U}{U}` instant and its conditional black-spent counter-and-mill effect. Two shuffled, full-game PvP traces use an opposing Carven Caryatid spell of mana value 3: one spends black and mills the exact three cards at the top of its controller's library; the control spends no black and mills none. Both games continue to a terminal result and replay deterministically. The trusted authority owns the event, source-gate, and mutation checks. It awards no card or set credit and no Harbor reward.
