"""Rename every audio file to one scheme: the directory is the owner, the file is the cue.

The tree grew from many source bundles, so its file names carry whatever each
bundle happened to call a clip -- `BtrCup_NanAngry01` beside `Buttercup_NanDismiss01`,
`M_Mandrd1_Defeat_1` beside `m_mandroid1_hurt_1`. The directory already names the
owner, so repeating it in the file name is both redundant and the place where the
spellings diverge. One rule removes both problems:

    audio/<category>/[<locale>/]<owner...>/<cue>.ogg
    <category>/[<group>/]<owner...>/<cue>

where the cue is the slug of the clip's `trueName` with the owner's own prefix
removed. `Buttercup_NanDismiss01` under `nano_buttercup` becomes `nandismiss01`;
`M_Mandrd1_Defeat_1` under `m_mandrd1` becomes `defeat_1`.

Playback is unaffected. The runtime resolves a clip through the catalogue -- by
`trueName`, which this never touches, or by `logicalKey`, whose previous value is
kept as an alias. Only the bytes' location changes, and every locale of a clip is
renamed together so the two stay in step.

The rename is refused outright if two clips of one owner would take the same name,
so a collision can never silently drop a file.

    python tools/legacy-sources/normalize-audio-filenames.py --native-target ../FFOneClient/assets/game
    ... --apply
"""

from __future__ import annotations

import argparse
import collections
import json
import re
import sys
from pathlib import Path

CATALOG = "_runtime/audio.json"
CATALOG_SCHEMA = "ffone.semantic-audio-catalog.v5"
LOCALES = {"en", "ru", "de"}
GROUPS = {"nanos"}
SUFFIXES = "bcdefghijklmnopqrstuvwxyz"


def slug(value: str) -> str:
    return re.sub(r"_+", "_", re.sub(r"[^a-z0-9]+", "_", value.lower())).strip("_")


def owner_spells_its_clips(names: list[str]) -> bool:
    """True when the leading token of an owner's clips is the owner's own name.

    A per-character owner writes every clip as `<character>_<cue>` and shows at
    most two spellings of that character. A shared pool such as `sfx/shared` or
    `ambient` has hundreds of different leading tokens, because there the first
    token is part of the cue -- `Abandon_Mission`, `ButtercupLair_wAmbient_Loop` --
    and stripping it would both destroy the name and collide.
    """
    return len(collections.Counter(slug(name).split("_")[0] for name in names)) <= 2


def cue_roots(names: list[str]) -> set[str]:
    """The cue vocabulary an owner shows under its own dominant spelling.

    Nearly every owner spells its clips one way; the twenty-one that do not use a
    second, shorter or longer form of the same character (`btrcup` beside
    `buttercup`, `ice_king` beside `iceking`). Reading the vocabulary off the
    dominant group means the minority spelling can be recognised and removed
    without a hand-written table of character nicknames.
    """
    leading = collections.Counter(slug(name).split("_")[0] for name in names)
    if not leading:
        return set()
    dominant = leading.most_common(1)[0][0]
    roots = set()
    for name in names:
        tokens = slug(name).split("_")
        if tokens[0] == dominant and len(tokens) > 1:
            roots.add(re.sub(r"\d+$", "", tokens[1]))
    return roots - {""}


def cue_of(owner: str, true_name: str, roots: set[str]) -> str:
    """The clip's name with the owner's own prefix taken off the front.

    One leading token is dropped by default; more are dropped only when doing so
    lands on a cue the owner already uses, which is how a two-word spelling like
    `Ice King_NanDismiss01` reduces to the same `nandismiss01` its sixty-six
    siblings use.
    """
    tokens = slug(true_name).split("_")
    for take in (3, 2):
        if len(tokens) > take:
            root = re.sub(r"\d+$", "", tokens[take])
            if root and root in roots:
                return "_".join(tokens[take:])
    if len(tokens) > 1:
        root = re.sub(r"\d+$", "", tokens[1])
        if root in roots or not roots:
            return "_".join(tokens[1:])
    return "_".join(tokens[1:]) if len(tokens) > 1 else tokens[0]


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

    by_owner: dict[tuple[str, str], list[str]] = collections.defaultdict(list)
    for entry in catalog["assets"]:
        by_owner[(entry["category"], entry["owner"])].append(entry["trueName"])
    roots = {key: cue_roots(names) for key, names in by_owner.items()}
    prefixed = {key: owner_spells_its_clips(names) for key, names in by_owner.items()}

    # Two clips of one owner can be different takes of the same cue -- Ampfibian's
    # dismissal exists as both `AmpFibian_NanDismiss01` and `Amphibian_NanDismiss01`,
    # and they are different audio. The first by source name keeps the plain cue and
    # the rest take the catalogue's own `_take_b` suffix, so neither is lost.
    grouped: dict[tuple[str, str], list[dict]] = collections.defaultdict(list)
    for entry in catalog["assets"]:
        key_owner = (entry["category"], entry["owner"])
        cue = (
            cue_of(entry["owner"], entry["trueName"], roots[key_owner])
            if prefixed[key_owner]
            else slug(entry["trueName"])
        )
        if not cue:
            raise SystemExit(f"{entry['trueName']!r} leaves no cue under owner {entry['owner']!r}")
        grouped[(entry["files"][0]["path"].rsplit("/", 1)[0], cue)].append(entry)

    cue_for: dict[int, str] = {}
    takes = []
    for (_, cue), group in grouped.items():
        group.sort(key=lambda entry: entry["trueName"])
        for position, entry in enumerate(group):
            final = cue if position == 0 else f"{cue}_take_{SUFFIXES[position - 1]}"
            cue_for[id(entry)] = final
            if position:
                takes.append({"trueName": entry["trueName"], "cue": final})

    planned, unchanged = [], 0
    claimed: dict[str, str] = {}
    for entry in catalog["assets"]:
        cue = cue_for[id(entry)]
        moves = []
        for file in entry["files"]:
            directory = file["path"].rsplit("/", 1)[0]
            target = f"{directory}/{cue}.ogg"
            owner = claimed.setdefault(target, entry["trueName"])
            if owner != entry["trueName"]:
                raise SystemExit(f"{target} is claimed by both {owner!r} and {entry['trueName']!r}")
            if target != file["path"]:
                moves.append((file["path"], target))
        # The key mirrors the file's location without the locale directory, the
        # same rule `normalize-audio-keys.py` enforces; computing it any other
        # way here would leave the two tools disagreeing.
        segments = entry["files"][0]["path"].split("/")[1:]
        category, rest = segments[0], segments[1:]
        if rest and rest[0] in LOCALES:
            rest = rest[1:]
        previous = entry["logicalKey"].split("/")
        group = previous[1] if len(previous) > 2 and previous[1] in GROUPS else None
        key = "/".join([category] + ([group] if group else []) + rest[:-1] + [cue])
        if not moves and key == entry["logicalKey"]:
            unchanged += 1
            continue
        planned.append({"entry": entry, "moves": moves, "key": key, "cue": cue})

    if args.apply and planned:
        staged = []
        for change in planned:
            for source, target in change["moves"]:
                temporary = assets / f"{target}.rename-next"
                temporary.parent.mkdir(parents=True, exist_ok=True)
                (assets / source).replace(temporary)
                staged.append((temporary, assets / target))
        for temporary, target in staged:
            temporary.replace(target)
        for change in planned:
            entry = change["entry"]
            for source, target in change["moves"]:
                for file in entry["files"]:
                    if file["path"] == source:
                        file["path"] = target
            if change["key"] != entry["logicalKey"]:
                entry["aliases"] = sorted(set(entry.get("aliases", [])) | {entry["logicalKey"]})
                entry["logicalKey"] = change["key"]
        catalog["assets"].sort(key=lambda entry: entry["logicalKey"])
        temporary = catalog_path.with_suffix(".json.next")
        temporary.write_text(json.dumps(catalog, indent=1, ensure_ascii=False) + "\n", "utf-8")
        temporary.replace(catalog_path)

    report = {
        "schema": "ffone.audio-filename-normalisation.v1",
        "applied": bool(args.apply and planned),
        "counts": {
            "assets": len(catalog["assets"]),
            "renamed": len(planned),
            "alreadyCanonical": unchanged,
            "filesMoved": sum(len(change["moves"]) for change in planned),
            "distinctTakesKeptApart": len(takes),
        },
        "takes": takes,
        "byCategory": dict(
            collections.Counter(change["entry"]["category"] for change in planned).most_common()
        ),
        "examples": [
            {"trueName": c["entry"]["trueName"], "to": c["moves"][0][1] if c["moves"] else None}
            for c in planned[:12]
        ],
    }
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=1, ensure_ascii=False) + "\n", "utf-8")
    counts = report["counts"]
    print(
        f"assets {counts['assets']} | renamed {counts['renamed']} | already canonical "
        f"{counts['alreadyCanonical']} | files moved {counts['filesMoved']} | applied {report['applied']}"
    )
    for category, count in report["byCategory"].items():
        print(f"  {category:10} {count}")
    for example in report["examples"][:6]:
        print(f"  {example['trueName']:32} -> {example['to']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
