"""Shared public-workspace filtering for isolated Magic card tasks."""

from __future__ import annotations

import tomllib
from pathlib import Path

_EXCLUDED_NAMES = {
    ".cache",
    ".codex",
    ".git",
    ".private",
    ".sealed",
    "artifacts",
    "target",
    "__pycache__",
    "rav_card_task_workspace.py",
}


def ignore_magic_task_workspace(relative_path: str, names: list[str]) -> set[str]:
    """Exclude task gold, trace authorities, and task-specific review metadata."""
    ignored: set[str] = set()
    for name in names:
        if name in _EXCLUDED_NAMES:
            ignored.add(name)
        elif (
            Path(relative_path).name == "tasks"
            and name.startswith("rav-")
            and name.endswith("-pvp")
            and name != "rav-set-pvp"
        ):
            ignored.add(name)
        elif name.startswith("rav_") and name.endswith("_pvp.rs"):
            ignored.add(name)
        elif name.startswith("score_") and name.endswith("_pvp.py"):
            ignored.add(name)
        elif name.startswith("materialize_") and name.endswith("_task.py"):
            ignored.add(name)
    return ignored


def stub_trusted_authority_bins(workspace: Path) -> None:
    """Replace every registered RAV PvP authority binary with a safe stub."""
    session = workspace / "varieties" / "magic" / "session"
    manifest_path = session / "Cargo.toml"
    manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    for target in manifest.get("bin", []):
        if not isinstance(target, dict):
            continue
        name = target.get("name")
        if not isinstance(name, str) or not name.startswith("rav-") or not name.endswith("-pvp"):
            continue
        relative = Path(target.get("path", "src/main.rs"))
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError(f"RAV authority binary path escapes the session crate: {relative}")
        source = session / relative
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_text(_authority_stub(name), encoding="utf-8")
    validate_public_workspace(workspace)


def _authority_stub(name: str) -> str:
    message = name.replace("-", " ")
    return (
        "fn main() {\n"
        f'    eprintln!("trusted {message} authority is available only to Harbor after submission");\n'
        "    std::process::exit(2);\n"
        "}\n"
    )


def validate_public_workspace(workspace: Path) -> None:
    """Fail materialization if any trusted card task implementation remains."""
    session = Path("varieties/magic/session")
    session_manifest = workspace / "varieties" / "magic" / "session" / "Cargo.toml"
    targets = tomllib.loads(session_manifest.read_text(encoding="utf-8")).get("bin", [])
    safe_authorities = {
        (session / Path(target["path"])).as_posix(): _authority_stub(target["name"])
        for target in targets
        if isinstance(target, dict)
        and isinstance(target.get("name"), str)
        and target["name"].startswith("rav-")
        and target["name"].endswith("-pvp")
        and isinstance(target.get("path"), str)
    }
    for path in workspace.rglob("*"):
        relative = path.relative_to(workspace)
        if (
            path.is_dir()
            and relative.parts[-2:-1] == ("tasks",)
            and path.name.startswith("rav-")
            and path.name.endswith("-pvp")
            and path.name != "rav-set-pvp"
        ):
            raise ValueError(f"per-card task authority leaked into candidate workspace: {relative}")
        if not path.is_file():
            continue
        name = path.name
        if (
            name.startswith("score_") and name.endswith("_pvp.py")
            or name.startswith("materialize_") and name.endswith("_task.py")
            or name == "rav_card_task_workspace.py"
        ):
            raise ValueError(f"trusted task tooling leaked into candidate workspace: {relative}")
        if name.startswith("rav_") and name.endswith("_pvp.rs"):
            expected = safe_authorities.get(relative.as_posix())
            if expected is None or path.read_text(encoding="utf-8") != expected:
                raise ValueError(f"trusted PvP authority leaked into candidate workspace: {relative}")
