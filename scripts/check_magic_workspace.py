#!/usr/bin/env python3
"""Build the exact workspace once; run independent Rust test binaries in parallel.

No providers or credentials. This is regression evidence, not EDH certification.
Every cargo-emitted test executable must finish successfully for a green receipt.
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import hashlib
import json
from pathlib import Path
import subprocess
import time


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--timeout", type=int, default=1200)
    args = parser.parse_args()
    if not 1 <= args.workers <= 16 or args.timeout <= 0:
        parser.error("workers must be 1..16 and timeout must be positive")
    root = Path(__file__).resolve().parents[1]
    workspace = root / "varieties/magic"

    def source_snapshot() -> dict[str, str]:
        listed = subprocess.check_output([
            "git", "ls-files", "-z", "--cached", "--others", "--exclude-standard",
            "--", "varieties/magic", "scripts/check_magic_workspace.py",
        ], cwd=root)
        paths = sorted(set(name.decode() for name in listed.split(b"\0") if name))
        return {name: hashlib.sha256((root / name).read_bytes()).hexdigest()
                if (root / name).is_file() else "missing" for name in paths}

    sources = source_snapshot()
    # A unique directory prevents concurrent invocations from mixing receipts.
    import tempfile
    (root / "artifacts").mkdir(exist_ok=True)
    out = Path(tempfile.mkdtemp(prefix="magic-regression-", dir=root / "artifacts"))
    command = ["cargo", "test", "--workspace", "--tests", "--no-run", "--locked",
               "--offline", "--jobs", str(args.workers), "--message-format=json"]
    print(f"Building workspace; evidence: {out}", flush=True)
    build = subprocess.run(command, cwd=workspace, text=True, capture_output=True)
    (out / "build.jsonl").write_text(build.stdout)
    (out / "build.stderr").write_text(build.stderr)
    receipt = {"schema": "cardbench.magic.workspace-regression.v1", "command": command,
               "build_exit_code": build.returncode, "full_edh_certified": False,
               "source_sha256": sources, "tests": []}
    if build.returncode:
        receipt["passed"] = False
        (out / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
        print(build.stderr[-8000:], flush=True)
        return 1
    if source_snapshot() != sources:
        receipt.update(passed=False, failure="source tree changed during build")
        (out / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
        print("Source tree changed during build; rerun on a stable tree", flush=True)
        return 1
    binaries = {}
    for line in build.stdout.splitlines():
        item = json.loads(line)
        if (item.get("reason") == "compiler-artifact" and item.get("executable")
                and item.get("profile", {}).get("test")):
            binaries[item["executable"]] = {
                "name": item["target"]["name"],
                "cwd": str(Path(item["manifest_path"]).parent),
            }
    if not binaries:
        raise RuntimeError("cargo emitted no test executables; refusing an empty green run")

    def run(binary: str) -> dict:
        path = Path(binary)
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        started = time.monotonic()
        log = out / f"{path.name}.log"
        with log.open("wb") as stream:
            try:
                result = subprocess.run([binary], cwd=binaries[binary]["cwd"], stdout=stream,
                                        stderr=subprocess.STDOUT, timeout=args.timeout)
                status = result.returncode
            except subprocess.TimeoutExpired:
                status = "timeout"
        return {"target": binaries[binary]["name"], "cwd": binaries[binary]["cwd"],
                "executable": binary, "sha256": digest,
                "binary_unchanged": hashlib.sha256(path.read_bytes()).hexdigest() == digest,
                "exit_code": status, "seconds": round(time.monotonic() - started, 3),
                "log": str(log)}

    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        futures = [pool.submit(run, binary) for binary in sorted(binaries)]
        for future in as_completed(futures):
            result = future.result()
            receipt["tests"].append(result)
            count = len(receipt["tests"])
            if result["exit_code"] != 0 or count % 50 == 0 or count == len(binaries):
                print(f"{count}/{len(binaries)}: {result['target']} -> {result['exit_code']}", flush=True)
    receipt["tests"].sort(key=lambda item: item["executable"])
    receipt["source_unchanged"] = source_snapshot() == sources
    receipt["passed"] = receipt["source_unchanged"] and all(
        item["exit_code"] == 0 and item["binary_unchanged"] for item in receipt["tests"])
    (out / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(f"passed={receipt['passed']}; receipt={out / 'receipt.json'}", flush=True)
    return 0 if receipt["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
