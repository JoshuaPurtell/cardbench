#!/usr/bin/env python3
"""Deterministic, dependency-free low-fi Pokemon board renderer."""

from __future__ import annotations

import argparse
import json
import struct
import zlib
from pathlib import Path

WIDTH = 256
HEIGHT = 160
PALETTE = {
    "felt": (23, 54, 57),
    "line": (105, 151, 139),
    "p1": (69, 142, 198),
    "p2": (211, 84, 79),
    "slot": (32, 76, 78),
    "card": (225, 212, 168),
    "hp": (74, 183, 92),
    "damage": (188, 57, 55),
    "energy": (247, 194, 62),
    "ink": (18, 25, 24),
}


def _rect(rgb: bytearray, x: int, y: int, w: int, h: int, color: tuple[int, int, int]) -> None:
    for yy in range(max(0, y), min(HEIGHT, y + h)):
        for xx in range(max(0, x), min(WIDTH, x + w)):
            i = (yy * WIDTH + xx) * 3
            rgb[i : i + 3] = bytes(color)


def _slot(rgb: bytearray, x: int, y: int, w: int, h: int, occupied: bool, hp: float, energy: int) -> None:
    _rect(rgb, x, y, w, h, PALETTE["line"])
    _rect(rgb, x + 2, y + 2, w - 4, h - 4, PALETTE["card"] if occupied else PALETTE["slot"])
    if occupied:
        bar = max(0, min(w - 6, round((w - 6) * hp)))
        _rect(rgb, x + 3, y + h - 6, w - 6, 3, PALETTE["damage"])
        _rect(rgb, x + 3, y + h - 6, bar, 3, PALETTE["hp"])
        for pip in range(min(energy, 6)):
            _rect(rgb, x + 4 + pip * 5, y + 4, 3, 3, PALETTE["energy"])


def render_frame(state: dict, output: Path) -> Path:
    rgb = bytearray(PALETTE["felt"] * (WIDTH * HEIGHT))
    players = [state.get("opponent", {}), state.get("player", {})]
    for side, player in enumerate(players):
        top = 7 if side == 0 else 91
        accent = PALETTE["p2"] if side == 0 else PALETTE["p1"]
        _rect(rgb, 2, top, 4, 61, accent)
        active = player.get("active") or {}
        _slot(
            rgb,
            100,
            top + 2,
            56,
            34,
            bool(active),
            float(active.get("hp_fraction", 1.0)),
            int(active.get("energy", 0)),
        )
        bench = player.get("bench", [])
        for idx in range(5):
            card = bench[idx] if idx < len(bench) else {}
            _slot(
                rgb,
                37 + idx * 37,
                top + 40,
                32,
                18,
                bool(card),
                float(card.get("hp_fraction", 1.0)),
                int(card.get("energy", 0)),
            )
        prizes = max(0, min(6, int(player.get("prizes", 6))))
        for idx in range(prizes):
            _rect(rgb, 9 + (idx % 2) * 8, top + 4 + (idx // 2) * 11, 6, 9, PALETTE["card"])
        deck = max(0, int(player.get("deck", 0)))
        discard = max(0, int(player.get("discard", 0)))
        _rect(rgb, 229, top + 4, 15, min(24, 4 + deck // 3), accent)
        _rect(rgb, 211, top + 4, 12, min(24, 3 + discard // 2), PALETTE["ink"])
    _rect(rgb, 0, 79, WIDTH, 2, PALETTE["line"])
    _rect(rgb, 126, 77, 4, 6, PALETTE["energy"])

    raw = b"".join(b"\x00" + bytes(rgb[y * WIDTH * 3 : (y + 1) * WIDTH * 3]) for y in range(HEIGHT))

    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", WIDTH, HEIGHT, 8, 2, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(png)
    return output


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--state", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    render_frame(json.loads(args.state.read_text()), args.output)


if __name__ == "__main__":
    main()
