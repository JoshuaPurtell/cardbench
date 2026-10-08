#!/usr/bin/env python3
"""Materialize and verify the public Wojek Apothecary task workspace."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from pathlib import Path

from rav_card_task_workspace import ignore_magic_task_workspace, stub_trusted_authority_bins

REPO = Path(__file__).resolve().parents[3]
MAGIC = REPO / "varieties" / "magic"
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/wojek_apothecary.rs")
STUB = '''//! Compile-valid but semantically incomplete starter for RAV Wojek Apothecary.

use std::collections::BTreeSet;
use cardbench_magic_engine::{ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost, TargetRequirement};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-WOJEK-APOTHECARY",
        name: "Wojek Apothecary",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::White, Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "colored-cost-casting", "base-characteristics", "tap-radiance-prevent-one-damage"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-WOJEK-APOTHECARY",
        ability: ActivatedAbility {
            id: "tap-radiance-prevent-one-damage",
            mana_cost: ManaCost::new(0),
            tap_cost: true,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::Creature],
            effects: vec![Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 1 }],
        },
    }
}
'''


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def tree_manifest(root: Path) -> dict[str, str]:
    for path in root.rglob("*"):
        relative = path.relative_to(root)
        if path.is_symlink():
            raise ValueError(f"workspace contains a symbolic link: {relative}")
        if any(part in {".git", ".private", ".sealed"} for part in relative.parts):
            raise ValueError(f"private authority leaked into workspace: {relative}")
    return {
        path.relative_to(root).as_posix(): sha256(path)
        for path in sorted(root.rglob("*"))
        if path.is_file() and path.relative_to(root).parts[0] not in {".codex", "logs"}
    }


def materialize(workspace: Path, manifest_path: Path) -> None:
    if workspace.exists():
        shutil.rmtree(workspace)
    workspace.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(MAGIC, workspace / "varieties" / "magic", ignore=ignore_magic_task_workspace)
    stub_trusted_authority_bins(workspace)
    target = workspace / TARGET
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(STUB, encoding="utf-8")
    hashes = tree_manifest(workspace)
    if hashes.get(TARGET.as_posix()) != hashlib.sha256(STUB.encode()).hexdigest():
        raise RuntimeError("Wojek Apothecary stub was not written at the declared target")
    manifest = {
        "schema_version": "cardbench.magic.task_workspace.v1",
        "task_id": "cardbench/magic/card",
        "target_file": TARGET.as_posix(),
        "allowed_modified_files": [TARGET.as_posix()],
        "files": hashes,
        "workspace_sha256": hashlib.sha256(
            "".join(name + chr(0) + digest + chr(10) for name, digest in hashes.items()).encode()
        ).hexdigest(),
    }
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, indent=2) + chr(10), encoding="utf-8")


def verify(workspace: Path, manifest_path: Path) -> None:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if set(manifest["allowed_modified_files"]) != {TARGET.as_posix()} or Path(manifest["target_file"]) != TARGET:
        raise ValueError("manifest must allow only the declared Wojek Apothecary module")
    target = workspace / TARGET
    if target.is_symlink() or not target.is_file():
        raise ValueError("candidate module must be a regular file")
    if target.stat().st_size > 64 * 1024:
        raise ValueError("candidate module exceeds 64 KiB")
    expected: dict[str, str] = manifest["files"]
    actual = tree_manifest(workspace)
    added = sorted(set(actual) - set(expected))
    removed = sorted(set(expected) - set(actual))
    changed = sorted(name for name in set(actual) & set(expected) if name != TARGET.as_posix() and actual[name] != expected[name])
    if added or removed or changed:
        raise ValueError("candidate changed files outside Wojek Apothecary module: " + json.dumps({"added": added[:10], "removed": removed[:10], "changed": changed[:10]}))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--verify", action="store_true")
    args = parser.parse_args()
    if args.verify:
        verify(args.workspace.resolve(), args.manifest.resolve())
    else:
        materialize(args.workspace.resolve(), args.manifest.resolve())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
