FROM rust:1.86-bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
    bash \
    ca-certificates \
    jq \
    python3 \
    ripgrep \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY Cargo.toml Cargo.lock /tmp/setbench-core/
COPY scaffold/src /app/src
COPY tcg_core /deps/tcg_core
COPY tcg_rules_ex /deps/tcg_rules_ex

RUN python3 - <<'PY'
from pathlib import Path

src = Path('/tmp/setbench-core/Cargo.toml').read_text()
src = src.split('\n[workspace]\n', 1)[0]
src = src.replace('path = "scaffold/src/lib.rs"', 'path = "src/lib.rs"')
src = src.replace('tcg_core = { path = "./tcg_core" }', 'tcg_core = { path = "/deps/tcg_core" }')
src = src.replace('tcg_rules_ex = { path = "./tcg_rules_ex" }', 'tcg_rules_ex = { path = "/deps/tcg_rules_ex" }')
Path('/app/Cargo.toml').write_text(src)
Path('/app/Cargo.lock').write_text(Path('/tmp/setbench-core/Cargo.lock').read_text())
PY

RUN cargo check --package tcg_expansions
RUN cargo test --package tcg_expansions --no-run
