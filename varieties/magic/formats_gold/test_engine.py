"""Format gold tests. Rust M5 ported these rules; the protocol now claims Supported."""

from pathlib import Path

from engine import (
    MagicMultiplayerEngine,
    commander_block_world,
    thg_race_world,
)


def _play(engine: MagicMultiplayerEngine, choose) -> None:
    while not engine.done():
        if engine.who_acts() == "frozen":
            engine.step(engine.frozen_action())
            continue
        engine.step(choose(engine.legal_actions()))


def test_rust_protocol_claims_the_formats_its_fixtures_defend() -> None:
    magic = Path(__file__).resolve().parents[1]
    text = (magic / "protocol" / "src" / "version.rs").read_text(encoding="utf-8")
    assert "(FormatCapability::Commander, SupportLevel::Supported)" in text
    assert "(FormatCapability::TwoHeadedGiant, SupportLevel::Supported)" in text
    assert (magic / "engine" / "tests" / "formats_m5.rs").is_file()


def test_two_headed_giant_damage_hits_shared_team_life() -> None:
    engine = MagicMultiplayerEngine()
    engine.reset_world(
        thg_race_world(
            world_id="rule-thg",
            seed=1,
            attacker="Watchwolf",
            blocker="Szadek, Lord of Secrets",
        ),
        48,
    )
    assert engine.observation_env()["team_life"] == [4, 3]
    _play(engine, lambda legal: next((item for item in legal if item.startswith("attack:")), "pass"))
    assert engine.observation_env()["team_life"][1] <= 0
    assert engine.metrics("rule-thg", 0, 0)["score"] == 1.0


def test_commander_unblocked_swing_reaches_21() -> None:
    engine = MagicMultiplayerEngine()
    engine.reset_world(
        commander_block_world(
            world_id="rule-cmd",
            seed=1,
            wall="Chorus of the Conclave",
            threat="Razia, Boros Archangel",
            commander="Tolsimir Wolfblood",
        ),
        48,
    )
    _play(engine, lambda legal: "pass")
    assert engine.observation_env()["seats"][0]["lost"] is True
    assert engine.metrics("rule-cmd", 0, 0)["score"] == 0.0
