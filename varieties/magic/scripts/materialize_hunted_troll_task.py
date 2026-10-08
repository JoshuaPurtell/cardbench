#!/usr/bin/env python3
"""Create and verify the deterministic public Hunted Troll workspace."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from pathlib import Path

from rav_card_task_workspace import ignore_magic_task_workspace, stub_trusted_authority_bins

REPO = Path(__file__).resolve().parents[3]
MAGIC = REPO / "varieties" / "magic"
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/hunted_troll.rs")
STUB = '''//! Candidate Hunted Troll definition and ability bindings.

use cardbench_magic_engine::{ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost, TargetRequirement, TokenSpec, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-HUNTED-TROLL", name: "Hunted Troll", set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Green, Color::Green]),
        colors: BTreeSet::from([Color::Green]), mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]), is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "colored-cost-casting", "base-characteristics", "etb-targeted-opponent-flying-faerie-tokens", "regeneration"],
        power: Some(7), toughness: Some(4), keywords: vec![], effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-HUNTED-TROLL",
        ability: ActivatedAbility { id: "self-regeneration", mana_cost: ManaCost::with_colors(0, [Color::Green]), tap_cost: false, sorcery_speed: false, additional_tap_creatures: 0, sacrifice_source: false, sacrifice_creatures: 0, sacrifice_lands: 0, discard_cards: 0, targets: vec![], effects: vec![Effect::RegenerateSource] },
    }
}

#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-HUNTED-TROLL",
        ability: TriggeredAbility { id: "etb-opponent-faeries", condition: TriggerCondition::EntersBattlefield, mana_cost: ManaCost::new(0), optional: false, targets: vec![TargetRequirement::Opponent], effects: vec![Effect::CreateTokenForTargetOpponent { token: TokenSpec::blue_faerie(), count: 4 }] },
    }
}
'''


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def tree_manifest(root: Path) -> dict[str, str]:
    for path in root.rglob("*"):
        relative = path.relative_to(root)
        if path.is_symlink():
            raise ValueError(f"workspace contains a symbolic link: {relative}")
        if any(part in {".git", ".private", ".sealed"} for part in relative.parts):
            raise ValueError(f"private authority leaked into workspace: {relative}")
    files = sorted(path for path in root.rglob("*") if path.is_file())
    return {
        path.relative_to(root).as_posix(): sha256(path.read_bytes())
        for path in files
        if path.relative_to(root).parts[0] not in {".codex", "logs"}
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
    if hashes.get(TARGET.as_posix()) != sha256(STUB.encode()):
        raise RuntimeError("candidate stub was not written at the declared Hunted Troll target")
    manifest = {
        "schema_version": "cardbench.magic.task_workspace.v1",
        "task_id": "cardbench/magic/card",
        "target_file": TARGET.as_posix(),
        "allowed_modified_files": [TARGET.as_posix()],
        "files": hashes,
        "workspace_sha256": sha256("".join(f"{name}\0{digest}\n" for name, digest in hashes.items()).encode()),
    }
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


def verify(workspace: Path, manifest_path: Path) -> None:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    target_rel = Path(manifest["target_file"])
    if set(manifest["allowed_modified_files"]) != {TARGET.as_posix()} or target_rel != TARGET:
        raise ValueError("manifest must allow only the declared Hunted Troll source file")
    target = workspace / target_rel
    if target.is_symlink() or not target.is_file() or target.stat().st_size > 64 * 1024:
        raise ValueError("candidate Hunted Troll source must be a regular file under 64 KiB")
    expected: dict[str, str] = manifest["files"]
    actual = tree_manifest(workspace)
    unexpected, missing = sorted(set(actual) - set(expected)), sorted(set(expected) - set(actual))
    changed = sorted(name for name in set(actual) & set(expected) if name != target_rel.as_posix() and actual[name] != expected[name])
    if unexpected or missing or changed:
        raise ValueError("candidate changed files outside the Hunted Troll module: " + json.dumps({"added": unexpected[:10], "removed": missing[:10], "changed": changed[:10]}))


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
