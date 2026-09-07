"""CardBench format gold: Two-Headed Giant and mini-Commander.

The RAV duel engine in `engine/` remains 1v1 (defender = next seat, no command
zone, no shared team life). Protocol `SupportLevel` for those formats stays
Absent until rust M5 fixtures exist. This module is the Harbor-grade format
slice: public RAV names and P/T, vanilla combat, and the format rules.

Two-Headed Giant: 4 seats, teams (0,1) vs (2,3), shared team life, shared
team turn, attacks the opposing team.

Mini-Commander: 4-seat free-for-all, command zone, commander tax, 40 life,
21 commander damage. Libraries are singleton mini-lists, not 100-card EDH.
Mana is generic (lands tap for 1). Oracle text beyond P/T is out of slice.
"""

from __future__ import annotations

from typing import Any


CARDS: dict[str, dict[str, Any]] = {
    "Plains": {"cmc": 0, "power": 0, "toughness": 0, "land": True, "legendary": False},
    "Island": {"cmc": 0, "power": 0, "toughness": 0, "land": True, "legendary": False},
    "Swamp": {"cmc": 0, "power": 0, "toughness": 0, "land": True, "legendary": False},
    "Mountain": {"cmc": 0, "power": 0, "toughness": 0, "land": True, "legendary": False},
    "Forest": {"cmc": 0, "power": 0, "toughness": 0, "land": True, "legendary": False},
    "Watchwolf": {"cmc": 2, "power": 3, "toughness": 3, "land": False, "legendary": False},
    "Greater Mossdog": {"cmc": 4, "power": 3, "toughness": 3, "land": False, "legendary": False},
    "Boros Recruit": {"cmc": 1, "power": 1, "toughness": 1, "land": False, "legendary": False},
    "Elves of Deep Shadow": {"cmc": 1, "power": 1, "toughness": 1, "land": False, "legendary": False},
    "Tolsimir Wolfblood": {"cmc": 5, "power": 3, "toughness": 4, "land": False, "legendary": True},
    "Agrus Kos, Wojek Veteran": {"cmc": 5, "power": 3, "toughness": 3, "land": False, "legendary": True},
    "Savra, Queen of the Golgari": {"cmc": 4, "power": 2, "toughness": 2, "land": False, "legendary": True},
    "Circu, Dimir Lobotomist": {"cmc": 4, "power": 2, "toughness": 3, "land": False, "legendary": True},
    "Razia, Boros Archangel": {"cmc": 8, "power": 6, "toughness": 3, "land": False, "legendary": True},
    "Szadek, Lord of Secrets": {"cmc": 7, "power": 5, "toughness": 5, "land": False, "legendary": True},
    "Chorus of the Conclave": {"cmc": 6, "power": 3, "toughness": 8, "land": False, "legendary": True},
    "Sisters of Stone Death": {"cmc": 7, "power": 7, "toughness": 5, "land": False, "legendary": True},
}

CANDIDATE = 0
THG_TEAMS = ((0, 1), (2, 3))


def team_of(seat: int) -> int:
    return 0 if seat < 2 else 1


def teammates(seat: int) -> tuple[int, int]:
    return THG_TEAMS[team_of(seat)]


class MagicMultiplayerEngine:
    ENV_FAMILY = "magic-formats-gold"
    def reset_world(self, world: dict[str, Any], max_steps: int) -> None:
        fmt = str(world["format"])
        if fmt not in {"two_headed_giant", "commander"}:
            raise ValueError(f"unknown format {fmt}")
        seats_in = list(world["seats"])
        if len(seats_in) != 4:
            raise ValueError("gold is 4 seats")
        self._format = fmt
        self._max_steps = max_steps
        self._steps = 0
        self._next_id = 1
        self._skip_first_draw = True
        self._winner_team: int | None = None
        self._winner_seat: int | None = None
        self._attacks: list[dict[str, Any]] = []
        self._blocks: dict[int, int] = {}
        self._commander_damage: dict[int, dict[int, int]] = {i: {} for i in range(4)}
        self._seats: list[dict[str, Any]] = []
        for index, spec in enumerate(seats_in):
            self._seats.append(self._seat_from_spec(index, spec))
        if fmt == "two_headed_giant":
            life = list(world.get("team_life") or [30, 30])
            if len(life) != 2:
                raise ValueError("2HG needs two team life totals")
            self._team_life = [int(life[0]), int(life[1])]
        else:
            self._team_life = None
        starting = int(world.get("starting_seat") or 0)
        self._begin_turn(starting)
        self._maybe_eliminate()

    def _seat_from_spec(self, index: int, spec: dict[str, Any]) -> dict[str, Any]:
        commander = spec.get("commander")
        if commander is not None and commander not in CARDS:
            raise ValueError(f"unknown commander {commander}")
        board: list[dict[str, Any]] = []
        for item in spec.get("battlefield") or []:
            board.append(self._permanent(index, item["name"], sick=bool(item.get("sick", False))))
        for item in spec.get("lands") or []:
            perm = self._permanent(index, item["name"], sick=False)
            perm["tapped"] = bool(item.get("tapped", False))
            board.append(perm)
        in_zone = bool(spec.get("command_zone", commander is not None and self._format == "commander"))
        if commander and not in_zone:
            if not any(card["name"] == commander for card in board):
                board.append(self._permanent(index, commander, sick=False, commander=True))
        damage = spec.get("commander_damage") or {}
        for attacker, amount in damage.items():
            self._commander_damage[index][int(attacker)] = int(amount)
        return {
            "life": int(spec.get("life") or (40 if self._format == "commander" else 20)),
            "lost": False,
            "hand": list(spec.get("hand") or []),
            "library": list(spec.get("library") or []),
            "board": board,
            "commander": commander,
            "command_zone": in_zone and commander is not None,
            "commander_casts": int(spec.get("commander_casts") or 0),
            "land_played": False,
            "frozen": str(spec.get("frozen") or ("idle" if index == 1 and self._format == "two_headed_giant" else "attacker")),
        }

    def _permanent(self, owner: int, name: str, *, sick: bool, commander: bool | None = None) -> dict[str, Any]:
        card = CARDS[name]
        ident = self._next_id
        self._next_id += 1
        legendary = bool(card["legendary"])
        return {
            "id": ident,
            "name": name,
            "owner": owner,
            "power": int(card["power"]),
            "toughness": int(card["toughness"]),
            "damage": 0,
            "sick": sick,
            "tapped": False,
            "land": bool(card["land"]),
            "commander": legendary if commander is None else commander,
        }

    def _living(self) -> list[int]:
        return [i for i, seat in enumerate(self._seats) if not seat["lost"]]

    def _begin_turn(self, start_seat: int) -> None:
        if self._format == "two_headed_giant":
            self._active_team = team_of(start_seat)
            for seat in teammates(start_seat):
                if not self._seats[seat]["lost"]:
                    self._untap(seat)
                    if not self._skip_first_draw:
                        self._draw(seat)
                    self._seats[seat]["land_played"] = False
            self._skip_first_draw = False
            self._phase = "main"
            self._group_index = 0
            self._set_priority_from_group()
            return
        seat = start_seat
        living = self._living()
        if seat not in living:
            seat = living[0] if living else 0
        self._active_seat = seat
        self._untap(seat)
        if not self._skip_first_draw:
            self._draw(seat)
        self._skip_first_draw = False
        self._seats[seat]["land_played"] = False
        self._phase = "main"
        self._priority = seat

    def _untap(self, seat: int) -> None:
        for perm in self._seats[seat]["board"]:
            perm["tapped"] = False
            perm["sick"] = False
            perm["damage"] = 0

    def _draw(self, seat: int) -> None:
        library = self._seats[seat]["library"]
        if library:
            self._seats[seat]["hand"].append(library.pop(0))

    def _set_priority_from_group(self) -> None:
        seats = THG_TEAMS[self._active_team if self._phase != "blockers" else 1 - self._active_team]
        living = [seat for seat in seats if not self._seats[seat]["lost"]]
        if not living:
            self._priority = CANDIDATE
            return
        self._group_index = min(self._group_index, len(living) - 1)
        self._priority = living[self._group_index]

    def who_acts(self) -> str:
        if self.done():
            return "policy"
        return "policy" if self._priority == CANDIDATE else "frozen"

    def done(self) -> bool:
        return self._winner_team is not None or self._winner_seat is not None or self._steps >= self._max_steps

    def legal_actions(self) -> list[str]:
        if self.done():
            return []
        actions = ["pass"]
        seat = self._priority
        if self._phase == "main":
            if not self._seats[seat]["land_played"]:
                seen: set[str] = set()
                for name in self._seats[seat]["hand"]:
                    if name in seen or not CARDS[name]["land"]:
                        continue
                    seen.add(name)
                    actions.append(f"play_land:{name}")
            seen_spells: set[str] = set()
            for name in self._seats[seat]["hand"]:
                if name in seen_spells or CARDS[name]["land"]:
                    continue
                seen_spells.add(name)
                if self._can_pay(seat, int(CARDS[name]["cmc"])):
                    actions.append(f"cast:{name}")
            commander = self._seats[seat]["commander"]
            if commander and self._seats[seat]["command_zone"]:
                tax = int(CARDS[commander]["cmc"]) + 2 * int(self._seats[seat]["commander_casts"])
                if self._can_pay(seat, tax):
                    actions.append("cast_commander")
            return actions
        if self._phase == "attackers":
            attacking = {item["id"] for item in self._attacks}
            for perm in self._seats[seat]["board"]:
                if perm["land"] or perm["sick"] or perm["tapped"] or perm["id"] in attacking:
                    continue
                if self._format == "two_headed_giant":
                    actions.append(f"attack:{perm['id']}")
                else:
                    for target in self._living():
                        if target != seat:
                            actions.append(f"attack:{perm['id']}:{target}")
            return actions
        defending_team_or_seat = self._priority
        incoming = [item for item in self._attacks if self._targets(item, defending_team_or_seat)]
        blocked = set(self._blocks)
        for attacker in incoming:
            if attacker["id"] in blocked:
                continue
            for perm in self._seats[seat]["board"]:
                if perm["land"] or perm["tapped"] or perm["id"] in self._blocks.values():
                    continue
                actions.append(f"block:{attacker['id']}:{perm['id']}")
        return actions

    def _targets(self, attack: dict[str, Any], seat: int) -> bool:
        if self._format == "two_headed_giant":
            return team_of(seat) == attack["target"]
        return attack["target"] == seat

    def _can_pay(self, seat: int, cost: int) -> bool:
        untapped = sum(1 for perm in self._seats[seat]["board"] if perm["land"] and not perm["tapped"])
        return untapped >= cost

    def _pay(self, seat: int, cost: int) -> None:
        remaining = cost
        for perm in self._seats[seat]["board"]:
            if remaining <= 0:
                return
            if perm["land"] and not perm["tapped"]:
                perm["tapped"] = True
                remaining -= 1

    def step(self, action: str) -> None:
        if self.done():
            return
        legal = self.legal_actions()
        if action not in legal:
            action = "pass" if "pass" in legal else (legal[0] if legal else "pass")
        self._steps += 1
        if action == "pass":
            self._pass()
        elif action.startswith("play_land:"):
            self._play_land(action.split(":", 1)[1])
        elif action.startswith("cast:"):
            self._cast(action.split(":", 1)[1])
        elif action == "cast_commander":
            self._cast_commander()
        elif action.startswith("attack:"):
            self._declare_attack(action)
        elif action.startswith("block:"):
            _, attacker, blocker = action.split(":", 2)
            self._blocks[int(attacker)] = int(blocker)
        self._maybe_eliminate()
        if self._steps >= self._max_steps and self._winner_team is None and self._winner_seat is None:
            # Timeout is a loss for the candidate.
            if self._format == "two_headed_giant":
                self._winner_team = 1
            else:
                living = [seat for seat in self._living() if seat != CANDIDATE]
                self._winner_seat = living[0] if living else None

    def frozen_action(self) -> str:
        legal = self.legal_actions()
        if not legal:
            return "pass"
        kind = self._seats[self._priority]["frozen"]
        if kind == "idle":
            return "pass"
        if self._phase == "attackers":
            attack = next((item for item in legal if item.startswith("attack:")), None)
            if attack and self._format == "commander":
                preferred = next((item for item in legal if item.endswith(f":{CANDIDATE}")), attack)
                return preferred
            return attack or "pass"
        return "pass"

    def _play_land(self, name: str) -> None:
        seat = self._priority
        self._seats[seat]["hand"].remove(name)
        self._seats[seat]["board"].append(self._permanent(seat, name, sick=False))
        self._seats[seat]["land_played"] = True

    def _cast(self, name: str) -> None:
        seat = self._priority
        self._pay(seat, int(CARDS[name]["cmc"]))
        self._seats[seat]["hand"].remove(name)
        self._seats[seat]["board"].append(self._permanent(seat, name, sick=True))

    def _cast_commander(self) -> None:
        seat = self._priority
        name = self._seats[seat]["commander"]
        tax = int(CARDS[name]["cmc"]) + 2 * int(self._seats[seat]["commander_casts"])
        self._pay(seat, tax)
        self._seats[seat]["command_zone"] = False
        self._seats[seat]["commander_casts"] += 1
        self._seats[seat]["board"].append(self._permanent(seat, name, sick=True, commander=True))

    def _declare_attack(self, action: str) -> None:
        parts = action.split(":")
        ident = int(parts[1])
        perm = self._perm(ident)
        perm["tapped"] = True
        if self._format == "two_headed_giant":
            target: int = 1 - team_of(self._priority)
        else:
            target = int(parts[2])
        self._attacks.append({"id": ident, "owner": perm["owner"], "target": target, "commander": perm["commander"]})

    def _perm(self, ident: int) -> dict[str, Any]:
        for seat in self._seats:
            for perm in seat["board"]:
                if perm["id"] == ident:
                    return perm
        raise KeyError(ident)

    def _pass(self) -> None:
        if self._phase == "main":
            if self._format == "two_headed_giant":
                self._group_index += 1
                seats = [seat for seat in THG_TEAMS[self._active_team] if not self._seats[seat]["lost"]]
                if self._group_index < len(seats):
                    self._set_priority_from_group()
                    return
            self._phase = "attackers"
            self._group_index = 0
            if self._format == "two_headed_giant":
                self._set_priority_from_group()
            else:
                self._priority = self._active_seat
            return
        if self._phase == "attackers":
            if self._format == "two_headed_giant":
                self._group_index += 1
                seats = [seat for seat in THG_TEAMS[self._active_team] if not self._seats[seat]["lost"]]
                if self._group_index < len(seats):
                    self._set_priority_from_group()
                    return
                self._phase = "blockers"
                self._group_index = 0
                self._set_priority_from_group()
                return
            self._phase = "blockers"
            self._block_queue = self._defender_queue()
            if not self._block_queue:
                self._resolve_combat()
                return
            self._priority = self._block_queue.pop(0)
            return
        # blockers
        if self._format == "two_headed_giant":
            self._group_index += 1
            defending = [seat for seat in THG_TEAMS[1 - self._active_team] if not self._seats[seat]["lost"]]
            if self._group_index < len(defending):
                self._set_priority_from_group()
                return
            self._resolve_combat()
            return
        if self._block_queue:
            self._priority = self._block_queue.pop(0)
            return
        self._resolve_combat()

    def _defender_queue(self) -> list[int]:
        targets = []
        for attack in self._attacks:
            if attack["target"] not in targets and not self._seats[attack["target"]]["lost"]:
                targets.append(attack["target"])
        return targets

    def _resolve_combat(self) -> None:
        dead: list[int] = []
        for attack in self._attacks:
            attacker = self._perm(attack["id"])
            blocker_id = self._blocks.get(attack["id"])
            if blocker_id is not None:
                blocker = self._perm(blocker_id)
                attacker["damage"] += blocker["power"]
                blocker["damage"] += attacker["power"]
                if attacker["damage"] >= attacker["toughness"]:
                    dead.append(attacker["id"])
                if blocker["damage"] >= blocker["toughness"]:
                    dead.append(blocker["id"])
            else:
                self._deal_to_target(attack, attacker["power"])
        for ident in dict.fromkeys(dead):
            self._die(ident)
        self._attacks = []
        self._blocks = {}
        self._maybe_eliminate()
        if self.done():
            return
        self._advance_turn()

    def _deal_to_target(self, attack: dict[str, Any], amount: int) -> None:
        if self._format == "two_headed_giant":
            self._team_life[attack["target"]] -= amount
            return
        seat = attack["target"]
        self._seats[seat]["life"] -= amount
        if attack["commander"]:
            owner = attack["owner"]
            current = self._commander_damage[seat].get(owner, 0) + amount
            self._commander_damage[seat][owner] = current
            if current >= 21:
                self._seats[seat]["lost"] = True

    def _die(self, ident: int) -> None:
        for seat in self._seats:
            for perm in list(seat["board"]):
                if perm["id"] != ident:
                    continue
                seat["board"].remove(perm)
                if perm["commander"] and seat["commander"] == perm["name"]:
                    seat["command_zone"] = True
                return

    def _maybe_eliminate(self) -> None:
        if self._format == "two_headed_giant":
            assert self._team_life is not None
            if self._team_life[0] <= 0:
                self._winner_team = 1
            elif self._team_life[1] <= 0:
                self._winner_team = 0
            return
        for index, seat in enumerate(self._seats):
            if seat["life"] <= 0:
                seat["lost"] = True
        living = self._living()
        if len(living) == 1:
            self._winner_seat = living[0]
        elif not living:
            self._winner_seat = None

    def _advance_turn(self) -> None:
        if self._format == "two_headed_giant":
            nxt = 1 - self._active_team
            start = THG_TEAMS[nxt][0]
            if self._seats[start]["lost"]:
                start = THG_TEAMS[nxt][1]
            self._begin_turn(start)
            return
        living = self._living()
        if not living:
            return
        if self._active_seat in living:
            nxt = living[(living.index(self._active_seat) + 1) % len(living)]
        else:
            nxt = living[0]
        self._begin_turn(nxt)

    def observation_env(self) -> dict[str, Any]:
        seats = []
        for index, seat in enumerate(self._seats):
            seats.append(
                {
                    "seat": index,
                    "team": team_of(index) if self._format == "two_headed_giant" else index,
                    "life": self._life_for(index),
                    "lost": seat["lost"],
                    "hand": list(seat["hand"]) if index == CANDIDATE else [],
                    "hand_size": len(seat["hand"]),
                    "library_size": len(seat["library"]),
                    "battlefield": [
                        {
                            "id": perm["id"],
                            "name": perm["name"],
                            "power": perm["power"],
                            "toughness": perm["toughness"],
                            "tapped": perm["tapped"],
                            "sick": perm["sick"],
                            "land": perm["land"],
                            "commander": perm["commander"],
                        }
                        for perm in seat["board"]
                    ],
                    "commander": seat["commander"],
                    "command_zone": seat["command_zone"],
                    "commander_casts": seat["commander_casts"],
                    "commander_damage_taken": dict(self._commander_damage[index]),
                }
            )
        return {
            "format": self._format,
            "phase": self._phase,
            "priority_seat": self._priority,
            "candidate_seat": CANDIDATE,
            "team_life": list(self._team_life) if self._team_life is not None else None,
            "attacks": list(self._attacks),
            "blocks": {str(key): value for key, value in self._blocks.items()},
            "seats": seats,
            "won": self._candidate_won(),
        }

    def _life_for(self, seat: int) -> int:
        if self._team_life is not None:
            return self._team_life[team_of(seat)]
        return int(self._seats[seat]["life"])

    def _candidate_won(self) -> bool:
        if self._format == "two_headed_giant":
            return self._winner_team == 0
        return self._winner_seat == CANDIDATE

    def observation_text(self) -> str:
        env = self.observation_env()
        lines = [
            f"format={env['format']} phase={env['phase']} priority={env['priority_seat']}",
        ]
        if env["team_life"] is not None:
            lines.append(f"team_life={env['team_life']}")
        for seat in env["seats"]:
            board = ", ".join(
                f"{card['id']}:{card['name']} {card['power']}/{card['toughness']}"
                + (" tapped" if card["tapped"] else "")
                + (" sick" if card["sick"] else "")
                for card in seat["battlefield"]
            ) or "empty"
            zone = " command-zone" if seat["command_zone"] else ""
            lines.append(
                f"seat {seat['seat']} team={seat['team']} life={seat['life']} "
                f"hand={seat['hand'] if seat['seat'] == CANDIDATE else seat['hand_size']} "
                f"board=[{board}]{zone}"
            )
        if env["attacks"]:
            lines.append(f"attacks={env['attacks']}")
        lines.append("legal: " + " ".join(self.legal_actions()))
        return "\n".join(lines)

    def metrics(self, world_id: str, steps: int, illegal: int) -> dict[str, Any]:
        won = self._candidate_won()
        reason = "win" if won else ("timeout" if self._steps >= self._max_steps else "loss")
        return {
            "world_id": world_id,
            "success": won,
            "illegal_actions": illegal,
            "terminated": self.done(),
            "steps": steps,
            "score": 1.0 if won else 0.0,
            "reason": reason,
            "format": self._format,
        }


def creature(name: str, sick: bool = False) -> dict[str, Any]:
    return {"name": name, "sick": sick}


def thg_race_world(*, world_id: str, seed: int, attacker: str, blocker: str) -> dict[str, Any]:
    """Starting team can race a 3-life opposing team; passing loses to a 5-power swing."""
    return {
        "world_id": world_id,
        "seed": seed,
        "format": "two_headed_giant",
        "starting_seat": 0,
        "team_life": [4, 3],
        "seats": [
            {"frozen": "idle", "battlefield": [creature(attacker)], "hand": [], "library": []},
            {"frozen": "idle", "battlefield": [], "hand": [], "library": []},
            {"frozen": "attacker", "battlefield": [creature(blocker)], "hand": [], "library": []},
            {"frozen": "idle", "battlefield": [], "hand": [], "library": []},
        ],
    }


def thg_block_world(*, world_id: str, seed: int, wall: str, beater: str, threat: str) -> dict[str, Any]:
    """Opposing team acts first with lethal. Blocking then attacking wins; racing without a block loses."""
    return {
        "world_id": world_id,
        "seed": seed,
        "format": "two_headed_giant",
        "starting_seat": 2,
        "team_life": [5, 3],
        "seats": [
            {
                "frozen": "idle",
                "battlefield": [creature(wall), creature(beater)],
                "hand": [],
                "library": [],
            },
            {"frozen": "idle", "battlefield": [], "hand": [], "library": []},
            {"frozen": "attacker", "battlefield": [creature(threat)], "hand": [], "library": []},
            {"frozen": "idle", "battlefield": [], "hand": [], "library": []},
        ],
    }


def commander_race_world(*, world_id: str, seed: int, beater: str, commander: str) -> dict[str, Any]:
    """Three 2-life opponents. Attacking each seat wins; passing times out."""
    return {
        "world_id": world_id,
        "seed": seed,
        "format": "commander",
        "starting_seat": 0,
        "seats": [
            {
                "life": 40,
                "commander": commander,
                "command_zone": True,
                "frozen": "idle",
                "battlefield": [creature(beater)],
                "hand": [],
                "library": [],
            },
            {"life": 2, "commander": "Agrus Kos, Wojek Veteran", "command_zone": True, "frozen": "idle", "battlefield": [], "hand": [], "library": []},
            {"life": 2, "commander": "Savra, Queen of the Golgari", "command_zone": True, "frozen": "idle", "battlefield": [], "hand": [], "library": []},
            {"life": 2, "commander": "Circu, Dimir Lobotomist", "command_zone": True, "frozen": "idle", "battlefield": [], "hand": [], "library": []},
        ],
    }


def commander_block_world(
    *,
    world_id: str,
    seed: int,
    wall: str,
    threat: str,
    commander: str,
    prior_damage: int = 15,
) -> dict[str, Any]:
    """Seat 1's commander is one unblocked swing from 21 damage. Block, then finish the table."""
    return {
        "world_id": world_id,
        "seed": seed,
        "format": "commander",
        "starting_seat": 1,
        "seats": [
            {
                "life": 40,
                "commander": commander,
                "command_zone": True,
                "frozen": "idle",
                "battlefield": [creature(wall)],
                "hand": [],
                "library": [],
                "commander_damage": {"1": prior_damage},
            },
            {
                "life": 3,
                "commander": threat,
                "command_zone": False,
                "frozen": "attacker",
                "battlefield": [creature(threat)],
                "hand": [],
                "library": [],
            },
            {
                "life": 3,
                "commander": "Savra, Queen of the Golgari",
                "command_zone": True,
                "frozen": "idle",
                "battlefield": [],
                "hand": [],
                "library": [],
            },
            {
                "life": 3,
                "commander": "Circu, Dimir Lobotomist",
                "command_zone": True,
                "frozen": "idle",
                "battlefield": [],
                "hand": [],
                "library": [],
            },
        ],
    }
