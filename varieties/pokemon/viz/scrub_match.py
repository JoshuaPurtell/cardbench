#!/usr/bin/env python3
"""Render JSONL state snapshots into numbered PNG frames."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from render_frame import render_frame


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--eventlog", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    for index, line in enumerate(args.eventlog.read_text().splitlines()):
        event = json.loads(line)
        if "state" in event:
            render_frame(event["state"], args.output_dir / f"{index:04d}.png")


if __name__ == "__main__":
    main()
