# Convolute RAV card task

Implement the candidate-owned module for RAV #41, Convolute. It is a `{2}{U}` blue instant whose effect targets a spell and counters it unless that spell's controller pays `{4}` during resolution.

The trusted authority runs two independent shuffled, legal 60-card PvP games. In one, the opponent casts a real `{G}{W}` Watchwolf and has only one untapped land when Convolute resolves, so it explicitly declines `{4}` and the Watchwolf is countered. In the other, the opponent casts Elves of Deep Shadow with four untapped lands available and explicitly pays `{4}` through four listed mana abilities, so the creature resolves. Both traces verify Convolute's `{2}{U}` payment and exact target, deterministic replay, and a later seat-zero Watchwolf combat win. Static semantic mutations for the payment amount, spell color/cost, and unconditional-counter behavior must be detected. No card or set reward is awarded.
