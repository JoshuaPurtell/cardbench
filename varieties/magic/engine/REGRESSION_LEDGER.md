# Magic engine regression ledger

This public ledger records rules defects first established by a failing Rust
contract. Entries name engine semantics and public rules references, never card
rules prose or private benchmark material.

## MTG-TRANS-001 — hidden-zone transmute search receipt

- **Rule boundary:** Comprehensive Rules 701.20 and 701.23, used by 702.53.
- **Failure contract:** `event_log_contract::transmute_requires_a_public_reveal_and_permits_no_result_search`.
- **Observed failure:** the engine moved a selected library card directly to a
  hand without a reveal event and rejected the legal no-result search choice.
- **Required repair:** emit `CardRevealed` before the selected card moves, and
  allow `found: None` while still paying costs and shuffling the library.
- **Resolution:** `CardRevealed` now precedes the public hand move and
  `PolicyAction::Transmute` carries `Option<ObjectId>`; the regression
  contract passes for both a selected card and a legal no-result search.
- **Scope note:** this ledger item does not assert that activated abilities are
  fully modeled on the stack; that remains a separate stack-abstraction gap.

## MTG-CAST-002 — definition-bound mana activation during spell payment

- **Rule boundary:** Comprehensive Rules 117.1b and 601.2g.
- **Failure contract:** `cast_payment_context_red::paid_bundle_mana_ability_can_pay_a_spell_cost_without_pre_floating`.
- **Observed failure:** a spell cast whose controller had exactly one mana to
  pay a bound paid-bundle source's activation cost was rejected before that
  source could produce its legal mana output. The request API could express
  only pre-floated mana, so an ordinary mana-payment sequence was impossible.
- **Required repair:** a typed cast-payment context must permit ordered,
  definition-bound mana-ability activations without creating stack objects.
  The complete cast, including activation costs, outputs, spell payment, zone
  move, stack object, and canonical receipts, must remain atomic.
- **Resolution:** `CastRequest` now carries an ordered typed
  `payment_mana_abilities` vector limited to existing definition-bound mana
  abilities. The atomic cast transition emits a payment-context receipt,
  matching bound activation/cost/output receipts, and then `SpellCast`; every
  later failure restores the complete game state and event log. The invariant
  audit rejects a payment-context receipt without its matching activation or
  spell receipt, and rejects paid-bundle receipt sequences with missing or
  reordered cost/output records. The regression passes for both the legal
  cast and an intentionally unaffordable final spell payment.

## MTG-CAST-003 — typed basic-land mana during spell payment

- **Rule boundary:** Comprehensive Rules 106.1, 117.1b, 305.6, and 601.2g.
- **Failure contract:**
  `basic_land_cast_payment_red::typed_rav_basic_lands_can_pay_a_colored_spell_cost_inside_one_cast`.
- **Observed failure:** the cast request can now state the two explicit typed
  basic-land activations, but the engine rejects the first one with
  `intrinsic basic-land mana abilities are unavailable while paying a spell cost`.
  A RAV player therefore cannot complete an ordinary colored spell payment
  from untapped typed lands without artificially pre-floating mana in a
  separate action.
- **Required repair:** preserve the typed, ordered source-and-color choices
  inside the existing atomic cast-payment transaction; validate each land,
  tap it, add exactly its intrinsic color, write causally ordered receipts,
  and restore all changes if a later activation or final spell payment fails.
- **Resolution:** pending.
