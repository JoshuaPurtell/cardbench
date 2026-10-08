#!/usr/bin/env python3
"""Materialize and verify the public Golgari Rot Farm task workspace."""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from pathlib import Path

from rav_card_task_workspace import ignore_magic_task_workspace, stub_trusted_authority_bins

REPO = Path(__file__).resolve().parents[3]
MAGIC = REPO / "varieties" / "magic"
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/golgari_rot_farm.rs")
STUB = '''//! Compile-valid but semantically wrong Golgari Rot Farm starter.
use std::collections::BTreeSet;
use cardbench_magic_engine::{ActivatedManaAbility, CardDefinition, CardType, Color, Effect, LandEntryBinding, ManaAbilityBinding, ManaAbilityOutput, ManaBundle, ManaCost, TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding};

pub(crate) fn definition() -> CardDefinition {
    CardDefinition { id: "RAV-GOLGARI-ROT-FARM", name: "Golgari Rot Farm", set_code: "RAV", mana_cost: ManaCost::new(0), colors: BTreeSet::new(), mana_colors: BTreeSet::new(), card_types: BTreeSet::from([CardType::Land]), is_basic_land: false, supported_rules: &["full-rules-fidelity", "enters-tapped", "etb-return-controlled-land", "fixed-black-green-mana-bundle"], power: None, toughness: None, keywords: vec![], effects: vec![] }
}

pub(crate) fn land_entry_binding() -> LandEntryBinding {
    LandEntryBinding { card_definition: "RAV-GOLGARI-ROT-FARM", enters_tapped: false, optional_life_payment: None }
}

pub(crate) fn mana_ability() -> ManaAbilityBinding {
    ManaAbilityBinding { card_definition: "RAV-GOLGARI-ROT-FARM", ability: ActivatedManaAbility { id: "golgari-rot-farm-bg", tap_cost: true, output: ManaAbilityOutput::Bundle(ManaBundle::new([(Color::Blue, 1), (Color::Green, 1)])), amount: 0, life_payment: None, controller_damage: None } }
}

pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding { card_definition: "RAV-GOLGARI-ROT-FARM", ability: TriggeredAbility { id: "return-controlled-land", condition: TriggerCondition::EntersBattlefield, mana_cost: ManaCost::new(0), optional: false, targets: vec![TargetRequirement::Creature], effects: vec![Effect::ReturnControlledLandToHand] } }
}
'''


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def tree_manifest(root: Path) -> dict[str, str]:
    files: dict[str, str] = {}
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if path.is_symlink():
            raise ValueError(f"workspace contains a symbolic link: {relative}")
        if any(part in {".git", ".private", ".sealed"} for part in relative.parts):
            raise ValueError(f"private authority leaked into workspace: {relative}")
        if path.is_file() and relative.parts[0] not in {".codex", "logs"}:
            files[relative.as_posix()] = digest(path.read_bytes())
    return files


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
    if hashes.get(TARGET.as_posix()) != digest(STUB.encode()):
        raise RuntimeError("compile-valid Golgari Rot Farm starter was not written")
    manifest = {
        "schema_version": "cardbench.magic.task_workspace.v1",
        "task_id": "cardbench/magic/card",
        "target_file": TARGET.as_posix(),
        "allowed_modified_files": [TARGET.as_posix()],
        "files": hashes,
        "workspace_sha256": digest("".join(f"{name}\0{value}\n" for name, value in hashes.items()).encode()),
    }
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


def verify(workspace: Path, manifest_path: Path) -> dict:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if set(manifest["allowed_modified_files"]) != {TARGET.as_posix()} or Path(manifest["target_file"]) != TARGET:
        raise ValueError("manifest must allow only the declared Golgari Rot Farm module")
    target = workspace / TARGET
    if target.is_symlink() or not target.is_file() or target.stat().st_size > 64 * 1024:
        raise ValueError("candidate Golgari Rot Farm source must be a regular file no larger than 64 KiB")
    expected: dict[str, str] = manifest["files"]
    actual = tree_manifest(workspace)
    unexpected = sorted(set(actual) - set(expected))
    missing = sorted(set(expected) - set(actual))
    changed = sorted(name for name in set(actual) & set(expected) if name != TARGET.as_posix() and actual[name] != expected[name])
    if unexpected or missing or changed:
        raise ValueError("candidate changed files outside the Golgari Rot Farm module: " + json.dumps({"added": unexpected[:10], "removed": missing[:10], "changed": changed[:10]}))
    return manifest


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
