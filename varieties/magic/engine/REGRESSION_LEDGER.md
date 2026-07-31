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
- **Status:** red regression recorded; repair pending.
