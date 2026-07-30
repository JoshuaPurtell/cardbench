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
