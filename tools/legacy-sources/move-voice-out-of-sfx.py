"""Move spoken lines that were catalogued as shared SFX into the voice category.

Several legacy sound bundles were imported by package rather than meaning. That
is right for the creature and UI sounds that make up most of them, but a few
hundred files are performed speech -- the player avatar's emotes, attacks and
jump efforts, and the combat grunts and greetings of named characters. Being
catalogued as SFX keeps them out of the localisation pipeline, which only walks
`audio/voice/<locale>/`, so they can never be translated.

The split is by cue, not by guesswork. A creature's clips are animation states --
`stand1..4`, `death`, `wound`, `ready`, `skill0`, `corruptak`, `dodge`, `megatak`
-- and they stay where they are. What moves is the performance family:

* `f_avatar_*` and `m_avatar_*`, the player's own emote, attack and movement
  voice. Non-vocal shared movement effects (`Avatar_Flying`, `Avatar_SwimBack`
  and `Avatar_SwimIdle`) do not match these gender-owned families and stay SFX;
* `<character>_hurt_N`, `_atk_N`, `_defeat_N`, `_death_N` grunt sets;
* `m_sact3_*` and `f_pirate2_*`, which the source build itself filed under `vo/`
  rather than `sound/` -- those two are voice on the original client's own say-so.

Each moved clip joins the voice owner that character already has, so nothing
creates a near-duplicate owner: `professor_utonium_atk_1` becomes
`utonium_atk_1` under the existing `utonium`, and Mandroid's two source spellings
(`m_mandroid1_*`, `m_mandrd1_*`) both land on the installed `m_mandrd1`.

`trueName` is never rewritten -- it is the exact source `m_Name` and the runtime
resolves audio by it -- and the previous `sfx/shared/...` key is kept as an alias.

    python tools/legacy-sources/move-voice-out-of-sfx.py --native-target ../FFOneClient/assets/game
    ... --apply
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

CATALOG = "_runtime/audio.json"
CATALOG_SCHEMA = "ffone.semantic-audio-catalog.v5"

# Source stem prefix -> (voice owner, stem prefix to publish under). The owner is
# always one the catalogue already uses for that character, except the two avatar
# owners, which follow the established gendered `f_`/`m_` owner convention
# (`f_chef1`, `m_chef1`, `f_kid1`).
FAMILIES = [
    ("f_avatar_", "f_avatar", "avatar_"),
    ("m_avatar_", "m_avatar", "avatar_"),
    ("m_mandroid1_", "m_mandrd1", "m_mandrd1_"),
    ("m_mandrd1_", "m_mandrd1", "m_mandrd1_"),
    ("m_sact3_", "m_sact3", "m_sact3_"),
    ("f_pirate2_", "f_pirate2", "f_pirate2_"),
    ("professor_utonium_", "utonium", "utonium_"),
    ("mojo_jojo_", "mojojojo", "mojojojo_"),
    ("billy_", "billy", "billy_"),
    ("blossom_", "blossom", "blossom_"),
    ("dexter_", "dexter", "dexter_"),
    ("eduardo_", "eduardo", "eduardo_"),
    ("grim_", "grim", "grim_"),
    ("kevin_", "kevin", "kevin_"),
    ("mandark_", "mandark", "mandark_"),
    ("vilgax_", "vilgax", "vilgax_"),
]

# Only these cues are performed speech. A creature's animation-state clips share
# the same `<name>_<cue>` shape, so the cue is what separates them.
PERFORMANCE_CUE = re.compile(
    r"^(hurt|atk|atkshrt|defeat|death|defeat1|hurt1|gdluck|gooluck|greetting|greeting)"
    r"[0-9]*(_[0-9]+)?$"
)
AVATAR_OWNERS = {"f_avatar", "m_avatar"}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-target", type=Path, required=True)
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--report", type=Path, default=None)
    args = parser.parse_args()

    assets = args.native_target
    catalog_path = assets / CATALOG
    catalog = json.loads(catalog_path.read_text("utf-8"))
    if catalog["schema"] != CATALOG_SCHEMA:
        raise SystemExit(f"audio catalog schema must be {CATALOG_SCHEMA}")

    existing_owners = {entry["owner"] for entry in catalog["assets"] if entry["category"] == "voice"}
    existing_keys = {entry["logicalKey"] for entry in catalog["assets"]}
    # Some of this family already reached voice on an earlier import, and the
    # clip is the same source object, so its trueName is the identity to check.
    voice_entries: dict[str, dict] = {}
    for entry in catalog["assets"]:
        if entry["category"] == "voice":
            voice_entries.setdefault(entry["trueName"].strip().lower(), entry)

    moves, skipped, duplicates = [], [], []
    for entry in catalog["assets"]:
        if entry["category"] != "sfx":
            continue
        if len(entry["files"]) != 1:
            continue
        path = entry["files"][0]["path"]
        if not path.startswith("audio/sfx/"):
            continue
        # `audio/sfx/shared/zone_local/` and `.../world_shared/` hold scope-local
        # second copies of a handful of clips. They are a packaging decision of
        # their own and are left exactly where they are. Other SFX groups are
        # eligible because the player attack and jump voices were split into
        # `combat` and `movement` during the original native import.
        if path.startswith("audio/sfx/shared/") and path.count("/") != 3:
            continue
        stem = path.rsplit("/", 1)[-1][:-4]
        family = next((f for f in FAMILIES if stem.startswith(f[0])), None)
        if family is None:
            continue
        source_prefix, owner, target_prefix = family
        cue = stem[len(source_prefix) :]
        if owner not in AVATAR_OWNERS and not PERFORMANCE_CUE.match(cue):
            skipped.append({"stem": stem, "cue": cue, "reason": "not a performed-speech cue"})
            continue
        published = voice_entries.get(entry["trueName"].strip().lower())
        if published is not None:
            # The same source clip already sits under voice. Keeping the SFX
            # copy as well is a duplicate, not a second take -- but only the
            # bytes may say so, so they are compared before anything is dropped.
            here = (assets / path).read_bytes()
            there = (assets / published["files"][0]["path"]).read_bytes()
            if hashlib.sha256(here).digest() == hashlib.sha256(there).digest():
                duplicates.append({
                    "entry": entry,
                    "stem": stem,
                    "path": path,
                    "key": entry["logicalKey"],
                    "publishedAs": published["logicalKey"],
                    "publishedPath": published["files"][0]["path"],
                })
            else:
                skipped.append({"stem": stem, "cue": cue, "reason": "a different take of this clip is already under voice"})
            continue
        new_stem = f"{target_prefix}{cue}"
        new_path = f"audio/voice/en/{owner}/{new_stem}.ogg"
        new_key = f"voice/{owner}/{new_stem}"
        moves.append(
            {
                "entry": entry,
                "owner": owner,
                "ownerExisted": owner in existing_owners,
                "trueName": entry["trueName"],
                "from": path,
                "to": new_path,
                "fromKey": entry["logicalKey"],
                "toKey": new_key,
            }
        )

    targets = [move["to"] for move in moves]
    keys = [move["toKey"] for move in moves]
    if len(set(targets)) != len(targets) or len(set(keys)) != len(keys):
        raise SystemExit("two clips would take the same voice path or key")
    clash = [move for move in moves if move["toKey"] in existing_keys or (assets / move["to"]).exists()]
    if clash:
        raise SystemExit(f"{len(clash)} clips already exist in voice, first: {clash[0]['to']}")

    if args.apply and (moves or duplicates):
        for duplicate in duplicates:
            (assets / duplicate["path"]).unlink()
            catalog["assets"].remove(duplicate["entry"])
            target = voice_entries[duplicate["entry"]["trueName"].strip().lower()]
            target["aliases"] = sorted(set(target.get("aliases", [])) | {duplicate["key"]})
        for move in moves:
            target = assets / move["to"]
            target.parent.mkdir(parents=True, exist_ok=True)
            (assets / move["from"]).replace(target)
            entry = move["entry"]
            entry["category"] = "voice"
            entry["owner"] = move["owner"]
            entry["files"] = [{"locale": "en", "path": move["to"]}]
            entry["aliases"] = sorted(set(entry.get("aliases", [])) | {move["fromKey"]})
            entry["logicalKey"] = move["toKey"]
        catalog["assets"].sort(key=lambda entry: entry["logicalKey"])
        catalog["counts"]["assets"] = len(catalog["assets"])
        catalog["counts"]["voice"] = sum(1 for e in catalog["assets"] if e["category"] == "voice")
        catalog["counts"]["sfx"] = sum(1 for e in catalog["assets"] if e["category"] == "sfx")
        temporary = catalog_path.with_suffix(".json.next")
        temporary.write_text(json.dumps(catalog, indent=1, ensure_ascii=False) + "\n", "utf-8")
        temporary.replace(catalog_path)

    by_owner: dict[str, int] = {}
    for move in moves:
        by_owner[move["owner"]] = by_owner.get(move["owner"], 0) + 1
    report = {
        "schema": "ffone.voice-reclassification.v1",
        "applied": bool(args.apply and (moves or duplicates)),
        "counts": {
            "moved": len(moves),
            "owners": len(by_owner),
            "duplicatesDropped": len(duplicates),
            "leftInSfx": len(skipped),
        },
        "duplicatesDropped": [
            {k: d[k] for k in ("stem", "key", "publishedAs")} for d in duplicates
        ],
        "byOwner": by_owner,
        "newOwners": sorted({m["owner"] for m in moves if not m["ownerExisted"]}),
        "moves": [{k: m[k] for k in ("trueName", "from", "to", "toKey")} for m in moves],
        "leftInSfx": skipped,
    }
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=1, ensure_ascii=False) + "\n", "utf-8")
    print(
        f"moved {len(moves)} clips to voice across {len(by_owner)} owners; "
        f"dropped {len(duplicates)} byte-identical SFX duplicates; "
        f"left in sfx {len(skipped)}; applied {report['applied']}"
    )
    for owner, count in sorted(by_owner.items(), key=lambda kv: -kv[1]):
        mark = "" if owner in existing_owners else "  (new owner)"
        print(f"  {owner:20} {count:4}{mark}")
    for item in skipped[:10]:
        print(f"  left in sfx: {item['stem']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
