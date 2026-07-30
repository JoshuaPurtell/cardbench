# Engine bundle

`cardbench/pokemon/engine` remains a P1 compile-substrate check.

`cardbench/magic/engine` has a runnable Rust reference verifier:

```bash
./adapters/harbor/run.sh engine verify magic
```

It validates Ravnica-block manifests and deck construction, then compares six
public RAV scenario event-log digests against fixed baselines. The result is a
standard `engine-check.json`, `reward.txt`, and Harbor `lane-receipt.json`.
Candidate-engine bundle/Codex rollout promotion remains deliberately separate.
