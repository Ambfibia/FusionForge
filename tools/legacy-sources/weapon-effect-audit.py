"""Audit every XDT weapon row against the published projectile/effect contracts.

`cnAvatarAttack.AttackTarget` branches on `m_iTargetMode` (eWeaponTargetMode:
0 Hand, 1 Melee, 2 Pistol, 3 Shotgun, 4 Sniper, 5 Rocket, 6 Grenade) and
`MakeBullet` uses `m_iEffect1`, or `m_iEffect2` while the weapon battery is
charged. Rocket and grenade rows additionally drive `cnWarHead` with
`m_iDurationTime * 0.1` and `m_iDeliverTime * 0.5`.

A weapon's firing effect is correct only when its bullet types resolve to a
published BulletTable row.
"""

from __future__ import annotations

import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

FFONE = Path(r"D:/CodexProject/FusionFallProject/FFOneClient")
CONTRACTS = FFONE / "crates/ffone-runtime-contracts/src/tutorial_effects.rs"

TARGET_MODES = {
    0: "Hand",
    1: "Melee",
    2: "Pistol",
    3: "Shotgun",
    4: "Sniper",
    5: "Rocket",
    6: "Grenade",
}


def const_ints(text: str, name: str) -> list[int]:
    match = re.search(rf"pub const {name}: \[i32; \d+\] = \[(.*?)\];", text, re.S)
    if not match:
        raise SystemExit(f"{name} not found")
    return [int(v) for v in re.findall(r"-?\d+", match.group(1))]


def main() -> int:
    contracts = CONTRACTS.read_text("utf-8")
    published_bullets = set(const_ints(contracts, "RETROBUTION_TUTORIAL_BULLET_TYPES"))
    projectile_effects = set(const_ints(contracts, "RETROBUTION_TUTORIAL_PROJECTILE_EFFECT_IDS"))
    weapon_effects = set(const_ints(contracts, "RETROBUTION_WEAPON_EFFECT_IDS"))

    catalog = json.loads(
        (FFONE / "assets/game/map/shared/projectiles/catalog.json").read_text("utf-8")
    )
    catalog_bullets = {row["bulletType"] for row in catalog["rows"]}
    if catalog_bullets != published_bullets:
        print("WARNING: contract and published catalog disagree on bullet types")
        print("  contract only :", sorted(published_bullets - catalog_bullets))
        print("  catalog only  :", sorted(catalog_bullets - published_bullets))

    tables = json.loads((FFONE / "assets/game/data/tables/table-set.json").read_text("utf-8"))
    weapon = tables["tables"][0]["value"]["m_pWeaponItemTable"]
    rows, names = weapon["m_pItemData"], weapon["m_pItemStringData"]

    by_mode: Counter[str] = Counter()
    unresolved: dict[int, list] = defaultdict(list)
    warhead_bad: list = []
    obtainable = 0

    for item_id, row in enumerate(rows):
        if item_id == 0:
            continue
        # An unobtainable row has no name and no price; the client never equips it.
        name_index = row.get("m_iItemName", 0)
        name = (
            (names[name_index].get("m_strName") or "").strip()
            if 0 <= name_index < len(names)
            else ""
        )
        if not name:
            continue
        obtainable += 1
        mode = row.get("m_iTargetMode", 0)
        by_mode[TARGET_MODES.get(mode, f"unknown({mode})")] += 1

        for column in ("m_iEffect1", "m_iEffect2"):
            effect = row.get(column, 0)
            if effect == 0:
                continue  # m_iEffect2 == 0 means "no charged variant"
            if effect not in published_bullets:
                unresolved[effect].append((item_id, name, column, mode))

        if mode in (5, 6):
            duration = row.get("m_iDurationTime", 0)
            deliver = row.get("m_iDeliverTime", 0)
            if duration <= 0 or (mode == 6 and deliver <= 0):
                warhead_bad.append((item_id, name, TARGET_MODES[mode], duration, deliver))

    print(f"published BulletTable rows      : {len(published_bullets)}")
    print(f"published projectile particles  : {len(projectile_effects)}")
    print(f"published weapon effect ids     : {len(weapon_effects)}")
    print(f"obtainable weapon rows          : {obtainable}")
    print()
    print("target mode distribution:")
    for mode in TARGET_MODES.values():
        if by_mode.get(mode):
            print(f"  {mode:8} {by_mode[mode]:5d}")
    print()
    print(f"bullet types referenced but NOT published: {len(unresolved)}")
    for effect in sorted(unresolved):
        uses = unresolved[effect]
        sample = uses[0]
        print(
            f"  effect {effect:5d}  used by {len(uses):3d} slot(s)"
            f"  e.g. item {sample[0]} {sample[1][:30]!r} via {sample[2]}"
            f" mode={TARGET_MODES.get(sample[3], sample[3])}"
        )
    print()
    print(f"rocket/grenade rows with invalid warhead timing: {len(warhead_bad)}")
    for entry in warhead_bad[:10]:
        print(f"  item {entry[0]} {entry[1][:30]!r} {entry[2]} duration={entry[3]} deliver={entry[4]}")

    if len(sys.argv) > 1:
        Path(sys.argv[1]).write_text(
            json.dumps(
                {
                    "publishedBulletTypes": sorted(published_bullets),
                    "obtainableWeaponRows": obtainable,
                    "targetModeDistribution": dict(by_mode),
                    "unresolvedBulletTypes": {
                        str(k): [
                            {"item": i, "name": n, "column": c, "targetMode": TARGET_MODES.get(m, m)}
                            for i, n, c, m in v
                        ]
                        for k, v in sorted(unresolved.items())
                    },
                    "invalidWarheadTiming": warhead_bad,
                },
                indent=1,
                ensure_ascii=False,
            ),
            "utf-8",
        )
        print(f"\nwrote {sys.argv[1]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
