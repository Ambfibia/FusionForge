"""Give every nano voice clip the same key and path shape.

Nano voice reached the tree from several source bundles that spell the same
character differently, so one nano can own `iceking_nandismiss01` beside
`ice_king_nandismiss01`, and Buttercup's set is `btrcup_*` while her dismissal
trio is `buttercup_*`. The audio is right; only the naming is arbitrary, and it
makes the catalogue hard to read and hard to check.

This rewrites the derived halves of a nano voice entry -- its `logicalKey` and
the file path -- into one shape:

    voice/nanos/<owner>/<owner without nano_>_<cue>
    audio/voice/<locale>/<owner>/<owner without nano_>_<cue>.ogg

`trueName` is never touched: it is the exact source `m_Name` and the runtime
resolves voice by it, so normalisation cannot change what plays. The previous
`logicalKey` is kept as an alias, so anything that referenced the old key keeps
working.

Where multiple identities share a cue, preserve each entry; different Ogg file
hashes alone do not prove different audio (stream serials and checksums vary).
The first by source name gets the plain stem and others get `_take_b`, `_take_c`.
If this would steal an existing key or alias, fail before writing anything;
stable semantic ownership takes precedence over cosmetic filename alignment.

    python tools/legacy-sources/normalize-nano-voice.py --native-target ../FFOneClient/assets/game
    ... --apply
"""

from __future__ import annotations

import argparse
import collections
import json
import re
import sys
from pathlib import Path

from audio_key_validation import renamed_aliases, validate_key_plan

CATALOG = "_runtime/audio.json"
CATALOG_SCHEMA = "ffone.semantic-audio-catalog.v5"
CUE = re.compile(r"_(?=Nan|Pwr|Comm)")
SUFFIXES = "bcdefghijklmnopqrstuvwxyz"


def stem_of(owner: str, true_name: str) -> str:
    parts = CUE.split(true_name, maxsplit=1)
    cue = parts[1] if len(parts) > 1 else true_name
    cue = re.sub(r"[^a-z0-9]+", "_", cue.lower()).strip("_")
    base = re.sub(r"[^a-z0-9]+", "_", owner.removeprefix("nano_").lower()).strip("_")
    return f"{base}_{cue}" if cue else base


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

    entries = [
        entry
        for entry in catalog["assets"]
        if entry["category"] == "voice" and entry["owner"].startswith("nano_")
    ]

    grouped: dict[tuple[str, str], list[dict]] = collections.defaultdict(list)
    for entry in entries:
        grouped[(entry["owner"], stem_of(entry["owner"], entry["trueName"]))].append(entry)

    renames, takes, unchanged = [], [], 0
    assigned: dict[str, dict] = {}
    for (owner, base_stem), group in sorted(grouped.items()):
        group.sort(key=lambda entry: entry["trueName"])
        for position, entry in enumerate(group):
            stem = base_stem if position == 0 else f"{base_stem}_take_{SUFFIXES[position - 1]}"
            if position:
                takes.append({"owner": owner, "trueName": entry["trueName"], "stem": stem})
            logical_key = f"voice/nanos/{owner}/{stem}"
            if logical_key in assigned and assigned[logical_key] is not entry:
                raise SystemExit(f"two entries would take the logical key {logical_key!r}")
            assigned[logical_key] = entry
            moves = []
            for file in entry["files"]:
                target = f"audio/voice/{file['locale']}/{owner}/{stem}.ogg"
                if target != file["path"]:
                    moves.append((file["path"], target))
            if logical_key == entry["logicalKey"] and not moves:
                unchanged += 1
                continue
            renames.append({"entry": entry, "logicalKey": logical_key, "moves": moves})

    # Nothing may land on a path another entry still holds.
    sources = {source for change in renames for source, _ in change["moves"]}
    targets = collections.Counter(target for change in renames for _, target in change["moves"])
    clashes = [
        target
        for target, count in targets.items()
        if count > 1 or ((assets / target).exists() and target not in sources)
    ]
    if clashes:
        raise SystemExit(f"{len(clashes)} target paths would collide, first: {clashes[0]}")

    validate_key_plan(
        catalog["assets"], {id(c["entry"]): c["logicalKey"] for c in renames}
    )

    if args.apply and renames:
        # Move every file aside first so a rename can never overwrite a file the
        # catalogue still points at.
        staged = []
        for change in renames:
            for source, target in change["moves"]:
                temporary = assets / f"{target}.normalise-next"
                temporary.parent.mkdir(parents=True, exist_ok=True)
                (assets / source).replace(temporary)
                staged.append((temporary, assets / target))
        for temporary, target in staged:
            temporary.replace(target)
        for change in renames:
            entry = change["entry"]
            for source, target in change["moves"]:
                for file in entry["files"]:
                    if file["path"] == source:
                        file["path"] = target
            # Derive the key from the path that was actually written rather than
            # from the plan, so a key can never end up describing another
            # entry's file when two takes of one cue swap stems.
            written = entry["files"][0]["path"].rsplit("/", 1)[-1].rsplit(".", 1)[0]
            key = f"voice/nanos/{entry['owner']}/{written}"
            if key != entry["logicalKey"]:
                entry["aliases"] = renamed_aliases(entry, key)
                entry["logicalKey"] = key
        catalog["assets"].sort(key=lambda entry: entry["logicalKey"])
        temporary = catalog_path.with_suffix(".json.next")
        temporary.write_text(json.dumps(catalog, indent=1, ensure_ascii=False) + "\n", "utf-8")
        temporary.replace(catalog_path)

    report = {
        "schema": "ffone.nano-voice-normalisation.v1",
        "applied": bool(args.apply and renames),
        "counts": {
            "nanoVoiceEntries": len(entries),
            "alreadyCanonical": unchanged,
            "renamed": len(renames),
            "distinctTakesKeptApart": len(takes),
        },
        "takes": takes,
        "examples": [
            {"trueName": change["entry"]["trueName"], "to": change["logicalKey"]}
            for change in renames[:12]
        ],
    }
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=1, ensure_ascii=False) + "\n", "utf-8")
    print(
        f"nano voice entries {len(entries)} | already canonical {unchanged} | "
        f"renamed {len(renames)} | distinct takes {len(takes)} | applied {report['applied']}"
    )
    for example in report["examples"][:8]:
        print(f"  {example['trueName']:32} -> {example['to']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
