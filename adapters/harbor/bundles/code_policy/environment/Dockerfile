# Trial image for cardbench/pokemon/code_policy (`cb-cpo-pokemon`).
#
# Build from the cardbench checkout root:
#
#   docker build -t cb-cpo-pokemon \
#     -f adapters/harbor/bundles/code_policy/environment/Dockerfile .
#
# Shape is the GameBench DEO hillclimb bundle's, because the platform contract
# is the same one: `NestedTrialRuntime` extracts `/workspace` straight out of
# the image (`cp -a /workspace/. /out/`) and runs `/task/tests/test.sh` against
# the mutated copy. Nothing runs Harbor's task setup, so the workspace has to be
# COMPLETE at bake time.
#
# What is NOT the same: CardBench grades Rust, not Python. There is no
# per-candidate container sandbox here and this image must never be handed the
# docker socket -- the verifier compiles and executes the agent's own code, so
# the socket would be the agent's. The trial container is the sandbox.
#
# Toolchain pin. NOT 1.86 (what varieties/pokemon/engine/Dockerfile uses):
# benchmark_ai.py generates the benchmark package's Cargo.toml on the fly with
# no lockfile, so cargo resolves the newest compatible crates.io versions at
# bake time -- and those now need rustc >= 1.88 (`time-core` 0.1.9). 1.86 fails
# the prewarm outright. 1.97 is the toolchain the host checkout resolves
# against today; pinned rather than `rust:1-bookworm` so a re-bake compiles the
# same thing.
FROM rust:1.97-bookworm

ARG CARDBENCH_VARIETY=pokemon
ARG CARDBENCH_FAMILY=code_policy
ARG CANDIDATE_REL=candidate/policy.rs

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    CARDBENCH_VARIETY=${CARDBENCH_VARIETY} \
    CARDBENCH_FAMILY=${CARDBENCH_FAMILY} \
    CARDBENCH_CANDIDATE_REL=${CANDIDATE_REL} \
    CARDBENCH_REPO_ROOT=/task/cardbench \
    CARDBENCH_WORKSPACE_ROOT=/workspace \
    CARDBENCH_VERIFIER_OUT=/workspace/logs/verifier \
    CARDBENCH_CARGO_CACHE=/opt/cardbench/cargo \
    CARGO_BUILD_JOBS=4

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      bash ca-certificates jq python3 \
 && rm -rf /var/lib/apt/lists/*

# Two copies, deliberately.
#
# `/task/cardbench` is the graded one. tests/test.sh runs the sweep out of HERE,
# so the evaluator, the roster, the ranking origin (reference_policy_v2) and the
# whole sealed heldout split are outside anything the agent can edit. The agent
# gets `/workspace` and only `/workspace`, and the verifier reads exactly one
# file from it: candidate/policy.rs.
COPY . /task/cardbench
COPY adapters/harbor/bundles/code_policy/instruction.md /task/instruction.md
COPY adapters/harbor/bundles/code_policy/tests/test.sh /task/tests/test.sh

# The workspace copy, minus the sealed split. setup_workspace.sh fails the bake
# if any `.sealed` survives into it, or if the graded copy lost it.
RUN chmod +x /task/tests/test.sh \
      /task/cardbench/adapters/harbor/bundles/code_policy/environment/setup_workspace.sh \
 && /task/cardbench/adapters/harbor/bundles/code_policy/environment/setup_workspace.sh \
 && install -m 0755 \
      /task/cardbench/adapters/harbor/bundles/code_policy/environment/run_train_sweep.sh \
      /workspace/run_train_sweep.sh

# Compile the ranking origin now. Two reasons, either of which is sufficient:
#   * the graded container runs with `--network none`, so crates.io is not
#     reachable and an unpopulated CARGO_HOME would fail the build outright;
#   * a cold build is ~50s and the verifier has to fit two of them plus two
#     384-cell sweeps into its budget.
# Only the heldout roster is prewarmed: that is the graded surface, and a second
# train-roster tree is another ~350MB in the image for a path the verifier never
# takes.
RUN python3 /task/cardbench/adapters/harbor/bundles/code_policy/environment/prewarm_baseline.py heldout

# Only after the warm build: an empty cache plus offline would have failed it.
# From here on a build that reaches for the network fails loudly instead of
# hanging against a dead resolver inside `--network none`.
ENV CARGO_NET_OFFLINE=true

# instruction.md declares the roster the contract; the task id and the candidate
# path are build args, so AGENTS.md is generated here rather than checked in
# with values that could drift from the bake.
RUN printf '%s\n' \
    "# CardBench ${CARDBENCH_VARIETY} ${CARDBENCH_FAMILY}" \
    "" \
    "| key | value |" \
    "| --- | --- |" \
    "| task id | \`cardbench/${CARDBENCH_VARIETY}/${CARDBENCH_FAMILY}\` |" \
    "| submission | \`${CANDIDATE_REL}\` (one file, one \`pub struct\`) |" \
    "| roster / contract | \`varieties/${CARDBENCH_VARIETY}/rosters/${CARDBENCH_FAMILY}_v1.json\` |" \
    "| ranking origin | \`varieties/${CARDBENCH_VARIETY}/candidates/reference/reference_policy_v2.rs\` |" \
    "| train feedback | \`./run_train_sweep.sh\` |" \
    "" \
    "## Candidate contract" \
    "" \
    "A candidate is exactly ONE file: \`${CANDIDATE_REL}\`, defining one public" \
    "struct with \`new(seed: u64) -> Self\` and an \`impl tcg_ai::AiController\`." \
    "It is compiled into the benchmark binary, so it may use \`tcg_core\`," \
    "\`tcg_ai\`, \`rand\` and \`rand_chacha\`; nothing else in this workspace is" \
    "on its path." \
    "" \
    "## How you are scored" \
    "" \
    "On the HELDOUT split, which is not in this workspace: 4 decks and 8" \
    "opponent policies you cannot see, 384 cells. Your policy and the ranking" \
    "origin are run over identical cells and compared PAIRWISE, and the lift" \
    "must clear a paired-bootstrap confidence interval. Beating the origin on" \
    "an easier matchup earns nothing, and a handful of lucky cells does not" \
    "clear the interval." \
    "" \
    "\`./run_train_sweep.sh\` scores the TRAIN split (5 visible decks, 5 visible" \
    "opponents, 400 cells) and writes \`artifacts/train/per_cell.jsonl\`. That is" \
    "feedback, not your score." \
    "" \
    "## Rules" \
    "" \
    "- The graded evaluator, roster and reference policies live outside this" \
    "  workspace and are read from there. Editing your copies changes nothing" \
    "  about your grade." \
    "- Do not attempt to reconstruct the heldout split." \
    > /workspace/AGENTS.md

# The platform runs the verifier as `bash -lc …`, and a LOGIN shell sources
# /etc/profile, which overwrites PATH with the distro default and drops the rust
# image's /usr/local/cargo/bin. The grader then dies on `FileNotFoundError:
# 'cargo'` -- and, because run_policy_sweep.py catches everything and still
# writes a result, it dies as a scored zero rather than as an error. Put the
# toolchain back on the PATH of every login shell.
RUN printf 'export PATH="%s/bin:$PATH"\n' "${CARGO_HOME:-/usr/local/cargo}" \
      > /etc/profile.d/10-cardbench-rust.sh \
 && chmod 0644 /etc/profile.d/10-cardbench-rust.sh \
 && bash -lc 'command -v cargo && command -v rustc'

WORKDIR /workspace
CMD ["bash"]
