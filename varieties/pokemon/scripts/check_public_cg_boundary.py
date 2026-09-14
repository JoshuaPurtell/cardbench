#!/usr/bin/env python3
"""Fail when private Crystal Guardians implementation files become tracked."""

from __future__ import annotations

import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
ENGINE = ROOT / "varieties" / "pokemon" / "engine"


def main() -> int:
    tracked = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()

    leaked = [path for path in tracked if "/cg_private/" in f"/{path}"]
    if leaked:
        raise SystemExit("tracked private CG module:\n" + "\n".join(leaked))

    stub_root = ENGINE / "scaffold" / "src" / "cg" / "cards"
    non_stubs = []
    for path in tracked:
        candidate = ROOT / path
        if candidate.parent != stub_root or not candidate.name.startswith("cg_0"):
            continue
        body = candidate.read_text()
        if "public stub" not in body or "RuntimeHooks::empty()" not in body:
            non_stubs.append(path)
    if non_stubs:
        raise SystemExit("tracked CG card body is not a public stub:\n" + "\n".join(non_stubs))

    print("public CG boundary: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
