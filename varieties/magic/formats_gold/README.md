# Magic format gold (Two-Headed Giant / mini-Commander)

Rust M5 ported these rules into the RAV engine, so protocol
`SupportLevel::TwoHeadedGiant` and `::Commander` are now **Supported**, defended
by `engine/tests/formats_m5.rs` (`MULTIPLAYER_HANDOFF.md`). Those flags are only
allowed to move with engine fixtures behind them:
`capability_manifest_claims_only_what_is_implemented` reads the claim, and
`test_engine.py` checks the fixture file is still there.

This directory stays the Harbor-grade **format gold** — a second, independent
statement of the same rules, not a substitute for the rust engine:

- 2HG: 4 seats, shared team life, shared team turn, attack the opposing team
- mini-Commander: 4-seat FFA, command zone, tax, 40 life, 21 commander damage

Public RAV names and P/T. Vanilla combat. Not 100-card EDH. Not a 1v1 relabel.

```bash
cd varieties/magic/formats_gold
python3 -m pytest test_engine.py -q
```
