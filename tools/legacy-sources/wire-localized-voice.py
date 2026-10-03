"""Connect the Russian voice already on disk to the catalogue entries that need it.

`audio/voice/ru/` holds 1433 recorded clips, but only three of them are named by
`_runtime/audio.json`. Every other one is invisible: the catalogue is what maps a
logical clip to its per-locale file, so a translation the tree already contains
never plays. The English entry sits there with a single `en` file and the Russian
take beside it goes unused.

Matching is by owner and stem, the same identity the English side uses, with the
`trueName` slug accepted as a second spelling because the Russian files were
written with the source clip's own casing (`M_Mandrd1_Defeat_1.ogg`). While a file
is being wired it is also renamed to the English stem, so both locales of one clip
carry one name.

A Russian file with no English counterpart is reported, never deleted and never
guessed at.

    python tools/legacy-sources/wire-localized-voice.py --native-target ../FFOneClient/assets/game
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


def slug(value: str) -> str:
    return re.sub(r"[^a-z0-9]+", "_", value.lower()).strip("_")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-target", type=Path, required=True)
    parser.add_argument("--locale", default="ru")
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--report", type=Path, default=None)
    args = parser.parse_args()

    assets = args.native_target
    catalog_path = assets / CATALOG
    catalog = json.loads(catalog_path.read_text("utf-8"))
    if catalog["schema"] != CATALOG_SCHEMA:
        raise SystemExit(f"audio catalog schema must be {CATALOG_SCHEMA}")

    root = assets / "audio" / "voice" / args.locale
    if not root.is_dir():
        raise SystemExit(f"{root} does not exist")

    available: dict[tuple[str, str], Path] = {}
    for path in root.rglob("*.ogg"):
        available[(path.parent.name.lower(), path.stem.lower())] = path

    wired, already, missing = [], 0, 0
    used: set[Path] = set()
    for entry in catalog["assets"]:
        if entry["category"] != "voice":
            continue
        wired_file = next((f for f in entry["files"] if f["locale"] == args.locale), None)
        if wired_file is not None:
            already += 1
            # Already connected, so its file is in use -- otherwise a re-run
            # would report every wired translation as an orphan.
            used.add(assets / wired_file["path"])
            continue
        english = next((f for f in entry["files"] if f["locale"] == "en"), None)
        if english is None:
            continue
        stem = english["path"].rsplit("/", 1)[-1][: -len(".ogg")]
        owner = entry["owner"].lower()
        source = available.get((owner, stem.lower())) or available.get(
            (owner, slug(entry["trueName"]))
        )
        if source is None:
            missing += 1
            continue
        used.add(source)
        target = f"audio/voice/{args.locale}/{entry['owner']}/{stem}.ogg"
        wired.append({"entry": entry, "from": source, "to": target, "renamed": source != assets / target})

    targets = collections.Counter(item["to"] for item in wired)
    clashes = [target for target, count in targets.items() if count > 1]
    if clashes:
        raise SystemExit(f"{len(clashes)} localized files would collide, first: {clashes[0]}")

    orphans = sorted(
        str(path.relative_to(assets)).replace("\\", "/")
        for path in available.values()
        if path not in used
    )

    if args.apply and wired:
        for item in wired:
            destination = assets / item["to"]
            destination.parent.mkdir(parents=True, exist_ok=True)
            if item["from"] != destination:
                item["from"].replace(destination)
            item["entry"]["files"].append({"locale": args.locale, "path": item["to"]})
            item["entry"]["files"].sort(key=lambda file: file["locale"])
        temporary = catalog_path.with_suffix(".json.next")
        temporary.write_text(json.dumps(catalog, indent=1, ensure_ascii=False) + "\n", "utf-8")
        temporary.replace(catalog_path)

    report = {
        "schema": "ffone.localized-voice-wiring.v1",
        "locale": args.locale,
        "applied": bool(args.apply and wired),
        "counts": {
            "filesOnDisk": len(available),
            "wired": len(wired),
            "alreadyWired": already,
            "englishWithoutTranslation": missing,
            "translationsWithoutEnglish": len(orphans),
            "renamedToTheEnglishStem": sum(1 for item in wired if item["renamed"]),
        },
        "byOwner": dict(
            collections.Counter(item["entry"]["owner"] for item in wired).most_common()
        ),
        "translationsWithoutEnglish": orphans,
    }
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=1, ensure_ascii=False) + "\n", "utf-8")
    counts = report["counts"]
    print(
        f"{args.locale}: on disk {counts['filesOnDisk']} | wired {counts['wired']} | "
        f"already {counts['alreadyWired']} | no translation {counts['englishWithoutTranslation']} | "
        f"orphan translations {counts['translationsWithoutEnglish']} | applied {report['applied']}"
    )
    for owner, count in list(report["byOwner"].items())[:8]:
        print(f"  {owner:24} {count}")
    for orphan in orphans[:8]:
        print(f"  ORPHAN {orphan}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
