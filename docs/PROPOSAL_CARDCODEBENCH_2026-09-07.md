# CardCodeBench — finalized proposal

Date: 2026-09-07  
Status: finalized proposal; implementation and release qualification remain pending  
Planned repository: `JoshuaPurtell/cardcodebench`  
Display name: **CardCodeBench**

## 1. Decision and mission

Build a standalone, Harbor-native dataset that measures an agent's ability to implement card-game behavior correctly. Establish first-class heldout evaluation for Pokémon, then extend the same task and evidence contracts to Magic.

The benchmark has three separately reported tracks: single cards, card families, and mechanics plus their related cards. Agents write Rust against a precisely scoped starting engine; executable behavioral checks determine correctness. Gameplay skill, deck optimization, and prompt optimization are outside this benchmark.

The repository owns tasks, annotations, split manifests, frozen engine snapshots, the grading harness, and release evidence. CardBench remains an upstream source of engine code and legacy task material, not a required sibling checkout or a benchmark to rename. Existing CardBench task IDs and registrations remain unchanged; new tasks receive CardCodeBench identities and explicit provenance mappings.

This document authorizes no paid runs, repository publication, migration, or deletion by itself. Its present location is a planning handoff; move a copy into the new repository when implementation begins. Start development privately; publish only sanitized public material after the publication gate.

## 2. Starting point and limitations

The 2026-09-07 checkout audit found:

| Surface | Observed state | Treatment |
| --- | --- | --- |
| Pokémon single cards | 212 specs/stubs: 101 Dragon Frontiers, 111 Holon Phantoms | Import candidates, not certified tasks |
| Crystal Guardians | Legacy expansion/task material; absent from the current single-card catalog | Inventory and author a qualified subset |
| Pokémon reference evaluator | Local Rust compilation/testing; catalog hashes recorded | Replace unsafe scoring/execution paths before authority use |
| Sealed card assets | Configured/default directory absent in the audited checkout | Locate authorized assets; verify provenance and hashes; do not assume lost |
| Harbor card lane | Registry scaffold; current agent entry point rejects card tasks | Implement a complete lane |
| Magic Ravnica | Substantial engine and executable card substrate | Freeze and qualify supported behaviors before exporting tasks |
| Guildpact / Dissension | Formal manifests, not executable expansion coverage | Later work; no ready claim |

The existing evaluator awards compilation credit, parses test totals from subprocess stdout, does not enforce all recorded pins, and lacks an adequate candidate-execution boundary. Catalog entries mostly declare one or two visible tests, with six declaring zero; this does not establish hidden coverage. Current receipts and old reference passes do not certify the new benchmark.

Public legacy implementations and tests can inform task construction but cannot be relabeled private heldout authority. Preserve their exposure history. Removing files from a workspace prevents direct lookup; it does not prove absence from model pretraining.

## 3. Task tracks and assignment contracts

| Track key | Assignment | Permitted changes | Primary success condition |
| --- | --- | --- | --- |
| `single_card` | Implement one card's complete specified behavior | Named card module and required registration | Every required behavior and applicable regression gate passes |
| `card_family` | Implement an enumerated, coherent group of cards | Listed modules and scoped shared helpers | Every listed card is complete; shared behavior and interactions pass |
| `mechanic_cards` | Implement one missing mechanic and its explicitly listed cards | Named engine extension points, helpers, and card modules | Independent mechanic contract, all listed cards, and regressions pass |

Every task declares its rules edition, card text/rulings, starting snapshot, interface contract, allowed files, supported prerequisites, submission format, public examples, resource limits, and required behavior categories. Hidden cases may exercise combinations not shown publicly, but may not introduce undisclosed requirements.

“All related cards” always means the explicit card list within named expansion(s), not an unbounded search. Family assignments must have a behavioral rationale, not just an arbitrary number of cards. Required non-target dependencies are supplied as supported fixtures without exposing target solutions.

For single-card/family tasks, the engine must already support every required primitive. Missing substrate disqualifies the task until fixed. For mechanic tasks, remove the target capability intentionally from a known-working snapshot using a deterministic transformation, and prove that the reference patch restores it. Accidental engine defects are never part of the assignment.

Build each task as a fresh, independent workspace. A multi-card task is one assignment, not a sequence receiving hidden feedback between cards. Progressive or multi-step curricula may be added later as a separately reported setting.

### 3.1 Deterministic removal of cards and mechanics

Generate assignments from a pinned, working reference snapshot using a versioned removal manifest. Do not merely delete a file or apply unchecked text substitutions.

The manifest names the exact source symbols/files to replace or remove, compile-valid replacement stubs, preserved interfaces and metadata, registration responsibilities, permitted solution paths, dependent cards, public starting-state checks, and private restoration/regression checks. Pin both the transformation and resulting workspace digest. Fail materialization if expected source hashes or symbols differ.

For a single-card task, replace the target behavior with a stub while preserving the engine primitives needed to implement it. For example, Rayquaza's identity and supplied metadata may remain, but its attack and conditional bonus implementations are absent. Registration is either supplied or explicitly assigned. Remove alternate implementations and target-solution-bearing fixtures, tests, documentation, Git history, binaries, and image/cache layers from the agent-visible artifact.

For a mechanic-plus-cards task, remove the smallest coherent named capability and enumerate all target cards. Preserve general engine infrastructure. An illustrative future Magic task could remove convoke and the listed convoke cards while preserving ordinary casting, creature tapping, and mana accounting. The prompt specifies the mechanic contract and card list; the verifier checks the mechanic independently, its card integrations, and ordinary-casting regressions. This is a task design example, not a claim that this exact removable substrate exists today.

Audit dependencies before removal. Cards that require the removed capability must be included in the assignment, explicitly stubbed/excluded, or otherwise handled by the declared starting-state contract; do not leave unrelated code accidentally broken. If behavior is tightly intertwined, first refactor and validate the reference engine to expose a clean boundary, then freeze a new snapshot. Removing an entire shared damage pipeline to omit one card's damage bonus is a different, broader task, not an automatic transformation.

Acceptance for every generated variant: the sanitized start builds and passes its declared unaffected checks, fails the intended missing-functionality checks, and passes the complete required suite after applying the private reference patch. Materializing it twice produces the same digest. All related removal-depth variants remain in the same overlap/split group, including single-card tasks that reveal solutions to family or mechanic tasks.

## 4. Expansion scope and delivery order

| Game | Initial expansion targets | Order |
| --- | --- | --- |
| Pokémon | Crystal Guardians, Dragon Frontiers, Holon Phantoms | Qualify supported Pokémon tasks first; prioritize CG/DF and admit HP only where the required substrate passes |
| Magic | Ravnica: City of Guilds, Guildpact, Dissension | Ravnica first, then GPT/DIS after their engine prerequisites are implemented and verified |

These are coverage targets, not commitments that every printed card is initially valid. Publish an inventory classifying each card/mechanic as included, excluded with reason, blocked on substrate, or unreviewed. Basic/vanilla cards remain annotated calibration items and cannot dominate the benchmark's difficulty profile.

The first qualification pilot targets **18 Pokémon tasks: 9 single-card, 6 family, and 3 mechanic-plus-cards tasks**. Select for mechanic diversity and independently verifiable behavior, not model success rates. These are engineering/dev tasks and cannot later become unseen final-test tasks. A smaller qualified pilot is preferable to admitting invalid tasks merely to meet the count; deviations require a recorded reason.

Do not set the full release size by extrapolating from 212 imported specs. Set it after the substrate, overlap, and coverage audits. Report independent split-group counts as well as task and card counts.

## 5. Repository and Harbor packaging

```text
cardcodebench/
  README.md
  AGENTS.md
  dataset.toml                     # generated public Harbor dataset manifest
  schemas/                         # task, split, result, receipt schemas
  manifests/
    tasks/                         # canonical authoring records
    splits/                        # public split manifests
    engine-locks/                  # immutable source/toolchain/dependency pins
  tasks/
    pokemon/<expansion-or-block>/<task-slug>/
    magic/<expansion-or-block>/<task-slug>/
  engines/
    pokemon/                       # sanitized, pinned public substrate
    magic/
  harness/
    materialize/
    sandbox/
    verify/
    receipts/
  tools/                           # build, validate, qualify, audit, report
  tests/
    harness/
    adversarial/
  reports/
    dataset-card.md
    coverage/
    qualification/
  provenance/
```

Authoring manifests are the source of truth; generated Harbor bundles and indexes must reproduce byte-for-byte from locked inputs. No hard-coded user paths, implicit sibling imports, mutable branch pins, or unrecorded runtime patches.

Use standard Harbor `instruction.md`, `task.toml`, environment definitions, verifier entry points, and reference-solution entry points for qualification. Pin the Harbor version and validate generated bundles against that version. Harbor supports separate verifier environments and designated submission-artifact transfer; use those facilities and test their behavior on each supported execution provider. [Harbor task format](https://www.harborframework.com/docs/tasks).

Canonical logical instance IDs use `cardcodebench/<game>/<track>/<slug>` in our metadata. Map them deterministically to Harbor's `<org>/<name>` identifier, for example `JoshuaPurtell/cardcodebench-pokemon-single-card-df-097-rayquaza-ex`. Keep the logical ID stable; record task revisions separately. Versioned `dataset.toml` manifests pin task archives by digest. Registry publication is an optional distribution step, not proof of validity. [Harbor dataset publication](https://www.harborframework.com/docs/datasets/publishing).

Public train/dev bundles carry public material only. Final prompts, membership, reference patches, hidden cases, and private grading images live in an access-controlled artifact store and private release manifest. A trusted operator assembles complete private Harbor bundles. A public entry point without those assets must report evaluator unavailable, never fabricate a score. Source visibility, registry visibility, image access, and model-facing mounts are separate controls.

Qualification solutions are available to the trusted reference runner only. Never include reference patches in agent image layers or shared build caches. Published reproducibility suites may release old solutions/tests, but are then marked exposed and retired from future unseen-test claims.

## 6. Heldout evaluation and leakage control

Maintain three distinct concepts:

1. **Task split:** `train`, `dev`, or `test`; governs whether task-specific experience may influence the model or checkpoint selection.
2. **Scenario visibility:** public examples versus private behavioral cases within a task.
3. **Generalization regime:** within-expansion card generalization, expansion transfer, or unseen-mechanic transfer.

Private tests on trained cards measure new-case correctness, not unseen-card generalization. Public task prompts with hidden graders likewise do not establish task novelty.

Construct an overlap graph before splitting. Mandatory grouping edges include shared target cards, reprints/equivalent implementations, target-solution dependencies, and family/mechanic assignments that directly reveal another task's solution. Put connected groups in one split. Do not connect every use of a generic primitive; sharing ordinary engine infrastructure is allowed and annotated. Audit broader mechanic similarity separately for the unseen-mechanic regime.

Start with a deterministic 60/20/20 train/dev/test allocation of independent groups, stratified where feasible by game, expansion, track, mechanics, and difficulty estimate. This is a target, not permission to break groups or force statistically weak strata. Report actual counts and allocation deviations. Reserve enough independent test groups to support the intended uncertainty claim; the pilot alone is not a statistical benchmark.

Expansion-transfer manifests hold out one complete expansion and exclude overlapping cross-expansion assignments from training. Use a separately defined cohort when reprints or prerequisites make a clean transfer split impossible. Do not claim unseen-mechanic transfer where the same mechanic was trained under another card name.

Before materialization, audit source trees, Git history, fixtures, documentation, build outputs, dependency packages, image layers, and caches for target solutions and private assets. Agent containers have no general internet access; model transport stays outside the task sandbox. Supply the rules and dependencies needed to solve offline.

Training and screening may use only train tasks. Checkpoint selection uses dev. Freeze final membership, cases, seeds, budgets, and analysis before final evaluation. Log final-suite access; final scores do not feed training or iterative task selection. After repeated adaptive inspection, downgrade the panel to dev and author a fresh final panel.

## 7. Annotations and difficulty

Require schema-validated annotations for every task:

| Category | Required information |
| --- | --- |
| Identity | Logical ID, revision, game, expansion list, card IDs, track, language |
| Rules | Rules edition/date, card-text provenance, rulings, declared simplifications |
| Mechanics | Ability/effect types, targeting, timing, zones, randomness, interactions |
| Scope | Allowed paths, APIs, prerequisites, missing capabilities, target card count |
| Difficulty estimate | Easy/medium/hard/expert, written rationale, reviewer, scope/complexity dimensions |
| Empirical calibration | Separate cohort ID, model/version, agent configuration, attempts, success count, uncertainty, wall time, tokens, cost |
| Integrity | Split group, overlap edges, exposure status, source lineage, asset/image hashes |
| Qualification | Gate status, evidence IDs, reviewer approvals, exclusions, known limitations |

Difficulty estimates describe required reasoning and implementation complexity, not merely file count. Empirical difficulty is conditional on the stated agent and budget; preserve raw attempts and uncertainty. Do not overwrite an expert estimate with “0/8 = hard.” Final-test calibration records stay restricted until the evaluation is concluded.

## 8. Grading and execution trust boundary

Use a clean trusted controller to validate and extract only allowlisted source artifacts, rejecting traversal, symlinks escaping the submission root, unexpected executable hooks, oversized payloads, and manifest/dependency changes outside the contract. Reconstruct the pinned workspace rather than trusting an agent-provided repository or binary.

Compile and execute candidate code in an unprivileged, resource-bounded worker with no credentials, network, host mounts, or writable cross-trial cache. This includes compilation because Rust build scripts, procedural macros, and compiler inputs can execute or access data. Rebuild against locked dependencies. Agent stdout, self-reported test counts, and agent-created reward files are never grading authority.

The trusted grader and expected answers must remain outside the candidate execution boundary. A separate Harbor verifier container isolates the agent session, but submitted Rust is still untrusted when executed inside it. Use an additional restricted worker and an out-of-process, bounded protocol: the controller supplies scenarios/actions, observes candidate state transitions/events, and computes assertions itself. The candidate sees required scenario inputs, not private expected outcomes or the complete suite. White-box tests may supplement coverage but cannot be the sole tamper-sensitive authority.

For mechanics requiring new state, define an explicit public state/event contract. Avoid a judge that only compares textual implementation details. Canonicalize irrelevant ordering and representation differences while preserving behaviorally meaningful timing, choices, and state. Legitimate alternative implementations must pass.

Test positive behavior, illegal/negative cases, boundary conditions, costs, targeting, timing, zones, deterministic randomness, and interactions with relevant existing mechanics. Independently review rules interpretation and expected outcomes; differential checks against gold alone can replicate a shared bug. Use hand-checked scenarios and property/metamorphic checks where appropriate.

Primary reward is binary full correctness: all required behavioral and applicable regression checks pass. Compilation alone earns no correctness credit. Secondary diagnostics include per-requirement pass rate, per-card completeness, mechanic correctness, and compile/execution status. Partial training rewards are explicit, separately versioned policies and never substituted for the headline metric.

Each expected check must be accounted for exactly once. Missing, duplicated, malformed, or truncated evidence invalidates verification. Candidate compilation errors, incorrect behavior, and attributable candidate resource exhaustion are candidate failures. Missing assets, pin mismatch, grader crashes, and provider outages are infrastructure errors with no authoritative score; use a predeclared bounded retry policy. Infrastructure failures must not silently disappear from reported denominators or become reward zero.

## 9. Qualification, metrics, and durable evidence

A task progresses through `draft -> assets_verified -> reference_green -> qualified -> released`. Every transition requires immutable evidence; a regression can quarantine or withdraw it. Gate meanings are separate from model success rates.

For every released task, require:

- A reviewed specification and solvable, pinned starting state.
- Reference success in two independent clean runs.
- Stub/no-op failure on required functionality.
- A curated wrong implementation for each critical behavior category, caught by the corresponding checks; report broader mutation coverage separately.
- Relevant engine regression checks and deterministic replay.
- Passing submission-integrity, leakage, timeout, output-forgery, and reward-tampering tests.
- At least one real agent trial with a complete trajectory and authoritative verdict; agent success is not required to qualify a genuinely hard task.
- Independent reviewer approval of rules coverage and evidence, not author self-certification alone.

Across each released track, demonstrate real agent execution and independent clean reruns on supported Harbor backends. Initially certify local Docker. Certify Daytona separately before advertising cloud equivalence; unsupported isolation features fail closed, with no silently weaker fallback. A live campaign is not required on every possible provider.

### 9.1 Required live-agent acceptance: Luna low and GPT-OSS via Tinker

The pilot is not accepted on reference solutions or mocked model calls alone. Run both of the following on a fixed engineering/dev acceptance panel of generated tasks:

| Arm | Required configuration | Evidence |
| --- | --- | --- |
| Luna low | `gpt-5.6-luna`, reasoning effort `low`, through the supported coding-agent runtime | Resolved model/configuration, full tool-using trajectory, submitted artifact, private verifier receipt |
| GPT-OSS | `openai/gpt-oss-20b` sampled through Tinker using the training-compatible coding-agent path | Tinker model/policy identity, exact sampled token IDs and behavior logprobs, per-call contexts and training masks, trajectory, artifact, verifier receipt |

Use the same task revisions, instructions, tool interface, public information, and verifier for both arms. Declare comparable time/tool/token limits in advance and record model-specific rendering differences; never silently switch models, inference providers, or agent paths. Tinker must supply the actual GPT-OSS actions, not merely score a trajectory generated elsewhere.

Operationalize “some will work” as **at least two distinct, nontrivial tasks fully solved by each arm** on the acceptance panel; the solved tasks need not be identical. Both arms must attempt all three pilot tracks, but success in every track is not required for the first pilot. A successful attempt means binary full correctness, not compilation or a partial score. Independently regrade each passing submission in a clean verifier and reproduce its pass. Report all attempts and failures, including unsuccessful tracks, with denominators and cost. Do not weaken graders or relabel final-test tasks to satisfy this gate.

Freeze the panel and a bounded attempt budget before execution. If either arm has no qualifying successes, the acceptance gate remains unmet: investigate task feasibility, harness fidelity, and capability difficulty on dev, version any changes, and rerun transparently. Failure on a genuinely hard individual task does not invalidate that task. Development acceptance successes establish usability and nonzero task signal, not RL uplift or final-test generalization.

For GPT-OSS, additionally prove that a real scored episode assembles into a valid training batch without reconstructed/fabricated logprobs, altered model output, or tool observations marked as generated targets. Report whether train-only repeated attempts produce mixed rewards; an all-pass or all-fail group alone does not demonstrate useful binary group-relative training signal. An actual optimizer update and heldout uplift campaign are subsequent RL experiment gates, not implied by this inference acceptance test.

Repeat the two-arm live acceptance gate for Magic when its pilot is introduced. No provider calls are launched by updating this proposal; execution requires the bounded campaign and authorization described in section 10.

Record append-only lifecycle events and a final receipt: dataset/task/split revisions, engine/toolchain/image/evaluator digests, submission hash, trial and attempt IDs, seeds, agent/model configuration, timestamps, execution status, required-check results, reward, resource use, cost, trace references, and retry lineage. Store durable artifacts outside temporary directories. Keep private cases and detailed final-test traces access-controlled; public summaries must not expose them.

Report per-game, per-track, and per-expansion task pass rates with independent group counts and uncertainty. Report family all-cards-correct separately from per-card completeness. Avoid a pooled headline dominated by easy cards or large families. If an aggregate is published, predeclare group/stratum weighting. Repeated attempts and overlapping tasks are correlated; use group-aware uncertainty and paired comparisons rather than treating every assertion or rollout as independent.

For RL studies, train only on train, select checkpoints on dev, and compare baseline/trained on the same frozen final tasks, attempt budgets, and execution settings. Record checkpoint identity and all evaluation accesses. Do not claim uplift from compilation credit, training reward, or checkpoint selection on test.

## 10. Throughput without weakening validity

Build sanitized engine/toolchain images once per digest; prewarm certified providers. Parallelize independent tasks and attempts with explicit sandbox, CPU, RAM, compiler, and provider limits. Keep candidate writes and writable caches trial-local; share only audited immutable dependencies. Reuse submitted artifacts for verifier retries without rerunning the agent when the retry policy permits it.

Measure queue, startup, agent, compile, verification, and artifact-upload time separately, including cold/warm distributions and infrastructure-error rate. Optimize the measured bottleneck. No launch-throughput claim substitutes for completed valid trials per minute.

Provider-backed campaigns require a bounded aggregate budget, expected actual cost, and applicable authorization before launch. This proposal contains no spend commitment. Artifact-store and provider credentials remain in authorized host-side mechanisms, never in task sandboxes or images; do not use Keychain by default.

## 11. Implementation milestones and exit gates

| Milestone | Deliverable | Exit gate |
| --- | --- | --- |
| M0: inventory and locks | New repo scaffold, source/exposure inventory, candidate pool, immutable engine/dependency locks | Reproducible source assembly; missing assets and substrate gaps explicitly classified |
| M1: Pokémon heldout vertical slice | One fully wired single-card task, trusted controller, restricted worker, private asset resolution, native Harbor bundle | Reference/negative controls, anti-cheat tests, offline clean run, durable receipt; no scoring fallback |
| M2: Pokémon multi-granularity pilot | Target 18 dev tasks, three tracks, deterministic removal manifests, annotations, overlap graph, split tooling | Per-task qualification; section 9.1 Luna-low and Tinker GPT-OSS acceptance, including two full task successes per arm; valid GPT-OSS training-batch assembly |
| M3: Pokémon v0.1 | Qualified CG/DF/HP coverage as supported, independently authored final cohort, dataset card, versioned Harbor manifests | Frozen leak-audited splits; every included task qualified; calibrated dev report; actual heldout campaign with reproducible evidence |
| M4: Magic Ravnica | Immutable rules substrate, RAV single-card then family/mechanic tasks using the same harness contract | Pokémon-equivalent qualification, deterministic removal/restoration checks, independent Magic rules review, section 9.1 two-arm acceptance |
| M5: multi-expansion Magic | Supported GPT/DIS substrate and task cohorts; transfer splits | No unsupported behavior hidden inside assignments; all included tasks pass release gates |
| M6: distribution and maintenance | Sanitized public repo/dataset, private final channel, contribution and withdrawal policies | Publication audit, fresh-machine reproduction, release manifest and evidence review |

Do M1 before bulk-exporting the catalog. Do not delay Pokémon validity behind Magic engine work. Harbor readiness does not depend on Dock, Workshop, visualizations, or optimizer integration; those can later consume the same task identities and receipts without becoming graders. Existing CardBench registrations remain untouched until an explicit migration decision.

## 12. Release governance and definition of done

Pin every release and retain prior results. Any change to starting code, rules, required cases, reward, split, or material resource contract creates a new revision. Document affected results; quarantine broken tasks and publish exclusions/corrections instead of silently rewriting a leaderboard. Recompute comparisons consistently on a corrected cohort where warranted.

Before public distribution, review licenses and attribution for engine code and structured card data separately. Do not assume the code license covers names/rules text or add card art. Public final-suite metadata must not reveal private membership unintentionally.

The initial release is done when a new operator can run the public Pokémon dataset from pinned artifacts, an authorized operator can run the private heldout suite, both produce trustworthy Harbor verdicts and durable evidence without local recovery scripts or manual grading, and the Luna-low/Tinker GPT-OSS acceptance panel has recorded the required real successes. Unsupported cards and Magic work remain visibly unready. No claim of full game-rules validity exceeds the declared, reviewed task contracts.

Success is a defensible measurement of card-code correctness, not a large number of task directories.

## 13. Audit references

- [Pokémon card catalog](../varieties/pokemon/cards/catalog.json)
- [Legacy single-card evaluator](../varieties/pokemon/scripts/run_card_eval.py)
- [Multi-file set-engine authoring machinery](../varieties/pokemon/scripts/run_set_engine_eval.py)
- [Current Harbor adapter](../adapters/harbor/scripts/run_harbor.py)
- [Current engine provenance/pins](../engine_pins.toml)
- [Magic substrate and coverage notes](../varieties/magic/README.md)

These are inputs to the migration, not certifications of the new dataset. The Harbor documentation cited above was checked on 2026-09-07; implementation must pin and test the actual installed version rather than assume documentation and runtime match.
