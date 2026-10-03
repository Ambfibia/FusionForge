"""Give every audio entry the same logical-key shape.

The catalogue's own convention, followed by all but a handful of its entries, is

    <category>/[<group>/]<owner>/<stem>

with the file at `audio/<category>/[<locale>/]<owner>/<stem>.ogg`. `nanos` is the
one established group: nano voice is filed as `voice/nanos/nano_x/...` while its
files live in `audio/voice/en/nano_x/`, and that grouping is deliberate.

Two places drift from it and nothing else does:

* `voice/npcs/computress/dialogue/...` invents a group and then a further segment
  after the owner, where every other non-nano voice owner is plain `voice/<owner>`;
* `voice/btrcup/...` names an owner that is not the entry's owner (`buttercup`)
  and a stem that is not the file's stem (`buttercup_tut01.ogg`).

Only the key is rewritten -- no file moves, no `trueName` edits, and the previous
key is kept as an alias, so a consumer holding the old string keeps resolving.

    python tools/legacy-sources/normalize-audio-keys.py --native-target ../FFOneClient/assets/game
    ... --apply
"""

from __future__ import annotations

import argparse
import collections
import json
import sys
from pathlib import Path

from audio_key_validation import renamed_aliases, validate_key_plan

CATALOG = "_runtime/audio.json"
CATALOG_SCHEMA = "ffone.semantic-audio-catalog.v5"
LOCALES = {"en", "ru", "de"}
# Groups the catalogue already uses between the category and the owner.
GROUPS = {"nanos"}


def canonical_key(entry: dict) -> str:
    """The key mirrors the file's own location, minus the locale directory.

    A category may nest further than one owner -- `sfx/shared/zone_local/...` and
    `sfx/nano_skills/damage/deedee/...` both exist -- so every segment below the
    category is kept rather than collapsed to a single owner. The one addition is
    the `nanos` group, which the catalogue puts in the key but not in the path.
    """
    path = entry["files"][0]["path"].split("/")
    category = path[1]
    rest = path[2:]
    if rest and rest[0] in LOCALES:
        rest = rest[1:]
    rest = list(rest)
    rest[-1] = rest[-1].rsplit(".", 1)[0]
    previous = entry["logicalKey"].split("/")
    group = previous[1] if len(previous) > 2 and previous[1] in GROUPS else None
    return "/".join([category] + ([group] if group else []) + rest)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-target", type=Path, required=True)
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--report", type=Path, default=None)
    args = parser.parse_args()

    catalog_path = args.native_target / CATALOG
    catalog = json.loads(catalog_path.read_text("utf-8"))
    if catalog["schema"] != CATALOG_SCHEMA:
        raise SystemExit(f"audio catalog schema must be {CATALOG_SCHEMA}")

    changes = []
    for entry in catalog["assets"]:
        wanted = canonical_key(entry)
        if wanted != entry["logicalKey"]:
            changes.append({"entry": entry, "from": entry["logicalKey"], "to": wanted})

    taken = {entry["logicalKey"] for entry in catalog["assets"]} - {c["from"] for c in changes}
    wanted = collections.Counter(c["to"] for c in changes)
    clashes = [key for key, count in wanted.items() if count > 1 or key in taken]
    if clashes:
        raise SystemExit(f"{len(clashes)} keys would collide, first: {clashes[0]}")

    validate_key_plan(catalog["assets"], {id(c["entry"]): c["to"] for c in changes})

    if args.apply and changes:
        for change in changes:
            entry = change["entry"]
            entry["aliases"] = renamed_aliases(entry, change["to"])
            entry["logicalKey"] = change["to"]
        catalog["assets"].sort(key=lambda entry: entry["logicalKey"])
        temporary = catalog_path.with_suffix(".json.next")
        temporary.write_text(json.dumps(catalog, indent=1, ensure_ascii=False) + "\n", "utf-8")
        temporary.replace(catalog_path)

    report = {
        "schema": "ffone.audio-key-normalisation.v1",
        "applied": bool(args.apply and changes),
        "counts": {"assets": len(catalog["assets"]), "rewritten": len(changes)},
        "byPrefix": dict(
            collections.Counter("/".join(c["from"].split("/")[:-1]) for c in changes).most_common()
        ),
        "changes": [{"from": c["from"], "to": c["to"]} for c in changes],
    }
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=1, ensure_ascii=False) + "\n", "utf-8")
    print(f"assets {len(catalog['assets'])} | keys rewritten {len(changes)} | applied {report['applied']}")
    for prefix, count in report["byPrefix"].items():
        print(f"  {prefix:40} {count}")
    for change in report["changes"][:6]:
        print(f"  {change['from']}  ->  {change['to']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
