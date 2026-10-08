# RAV per-card PvP coverage inventory

Rules edition, printed-versus-Oracle policy, evidence requirements, and the
no-card-text retention boundary are fixed in
[`RAV_PVP_SOURCE_CONTRACT.md`](RAV_PVP_SOURCE_CONTRACT.md).

This is a work queue for expanding isolated, full-game PvP traces across Ravnica: City of Guilds. It deliberately distinguishes executable card definitions and focused engine-test leads from Harbor PvP trace credit.

## Current inventory

- Catalog denominator verified directly from `varieties/magic/sets/ravnica_city_of_guilds/catalog/rav_main_set.tsv`: **306 printings, 291 unique card names**, collector numbers 1 through 306 with no gaps or duplicates.
- The catalog executable IDs exactly match the `291` IDs in `RAV_FULL_FIDELITY_DEFINITION_IDS`; each unique card row also has a declared `supported_rules` list or a mapped shared factory in `src/lib.rs`.
- Isolated per-card PvP task directories currently found: **206/291** as of 2026-09-24. Run `python3 varieties/magic/scripts/report_rav_pvp_coverage.py` for the catalog-bound live list. Task presence and reference passes are diagnostics, not credit.
- Cards with no isolated per-card PvP task: **85/291** as of 2026-09-24. All 291 rows state `full_game_per_card_pvp_coverage_credited=no`.
- Rows with no test file whose filename names the card: **55**. Exact name/ID source-search hits are also included as limited samples; neither measure proves trace completeness.
- The live ledger also cross-checks each per-card task's name, definition ID, and collector number against the catalog-backed inventory.

## Parallel work queues

Each CSV row can be assigned independently to a card-trace author. `primary_parallel_work_queue` is a heuristic based on distinctive definition feature tags; generic fidelity, base-characteristic, and ordinary casting tags are excluded so they do not swamp the grouping. `likely_feature_branches` preserves overlapping groups. `declared_supported_rules` is the better starting point for understanding the current engine contract.

- **Stack targets, interaction, removal, damage, and prevention** (`stack_targets_interaction_and_removal`): 132 card rows (branch memberships overlap).
- **Library, graveyard, hand, exile, and private-zone choices** (`library_graveyard_and_private_zones`): 99 card rows (branch memberships overlap).
- **Combat and creature characteristics** (`combat_and_characteristics`): 89 card rows (branch memberships overlap).
- **Mana, casting costs, and lands** (`mana_costs_and_lands`): 70 card rows (branch memberships overlap).
- **Triggers and turn or zone events** (`triggers_and_turn_or_zone_events`): 68 card rows (branch memberships overlap).
- **Counters, continuous layers, and replacements** (`counters_layers_and_replacements`): 36 card rows (branch memberships overlap).
- **Baseline casting and characteristics** (`baseline_cast_and_characteristics`): 26 card rows (branch memberships overlap).
- **Auras, attachments, and Equipment** (`attachments_and_equipment`): 25 card rows (branch memberships overlap).

Primary queue assignment across all 291 rows (each card appears once): library/private zones 99; stack/target interaction 67; triggers/events 45; combat/characteristics 30; baseline 26; mana/lands 18; attachments/equipment 5; counters/layers/replacements 1.

## Reading the CSV

- One row represents one unique card name. Basic lands are each one row with four collector numbers; `printing_count` sums to 306 across the 291 rows.
- `positive_fidelity_manifest=yes` means the card’s definition ID is in the repository’s positive manifest. It records the current engine/card contract and does not mean a full-game PvP card task has passed.
- `declared_supported_rules` is taken from each `CardDefinition`; for the shared Signet, guild bounce land, shock land, and basic land factories, it is taken from that factory’s literal rule-tag list. The tags are identifiers only; no card rules text is copied here.
- `focused_test_file_candidates` lists test source files whose filename contains the card name or ID as a token sequence. `other_test_source_name_id_hit_count` and its sample catch name/ID references in other test files. Both are navigation aids, not assertions that a file independently proves every behavior or a complete player-vs-player trace.
- The CSV's task and receipt columns preserve the initial inventory snapshot. Use `python3 varieties/magic/scripts/report_rav_pvp_coverage.py` for the live task and receipt ledger. Reference receipts are diagnostics, not agent-qualified card credit. The live ledger currently reports 12 reference passes, no verified candidate trials, and no credited cards. Historical Char receipt review freshness is not revalidated by this inventory.
- `cross_card_dependency_notes` is a broad, heuristic reminder of shared engine surfaces likely to matter. Check the cited tests and bindings before treating a dependency as a blocker.

## Suggested implementation sequence

1. Start with cards whose declared tags fit baseline casting/characteristics or one existing stack effect. Add an isolated task and a normal shuffled two-player reference trace for each card.
2. Parallelize within one shared mechanic family (for example, targeted damage/removal, common keyword combat, guild mana/lands, or private library choices) so a shared verifier/fixture can be reused while each card still gets its own identity-bound trace.
3. Put cards with triggers, hidden-zone decisions, attachments, replacement effects, or multi-step combat interactions in separate bounded batches; preserve their response windows and decision ownership in the trace.
4. Credit only a card-specific PvP task after clean reference replay, semantic mutation rejection, independent review, and an actual candidate trial are all retained by the route. The CSV intentionally leaves every row uncredited today.

## Inventory limits

This artifact audits repository metadata and test-file leads, not the correctness of the 291 rules implementations. The branch assignments and dependency notes are inferred from CardBench-authored `supported_rules` identifiers. They do not quote or reproduce Oracle text, and they do not claim that every declared interaction works in every multiplayer or constructed-game context. The focused-test scan can have false positives (fixture mentions) and false negatives (generic test names). The active isolated PvP task reports remain diagnostic until candidate trials are retained.

Rebuild / verify the denominator with a small catalog-to-source audit; no test suite is needed for this inventory.
