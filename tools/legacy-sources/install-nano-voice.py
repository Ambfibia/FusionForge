"""Publish the nano voice sets FFOne's added nanos have no audio for.

FFOne installs 63 nano models but only the stock nano voices, so the nanos added
above that set are silent: nothing in `_runtime/audio.json` owns them. The
academy donor ships their clips in its `Nano_0NN.resourceFile` bundles, and those
`AudioClip` payloads are already Ogg Vorbis -- the content-pack cooker validates
exactly that and emits the stream unchanged -- so this publication transcodes
nothing.

`install-semantic-audio` cannot do it: it keys on the project `asset-manifest.json`
the native tree no longer carries, the same reason the equipment installers had to
be replaced by narrow additive ones. This tool is that narrow installer for voice.

It is additive and refuses to overwrite. Ownership is decided by
`plan-nano-voice.py`, which accepts a clip only when its own name prefix matches
either the installed model's `logicalName` or the `m_strName` the nano table gives
that model. Both are data; nothing is assigned by similarity or bundle number.

    python tools/legacy-sources/install-nano-voice.py --native-target ../FFOneClient/assets/game \\
      --clips work/legacy-sources/nano-voice-20260903/clips
    ... --apply
"""

from __future__ import annotations

import argparse
import base64
import json
import sys
from pathlib import Path

CATALOG = "_runtime/audio.json"
CATALOG_SCHEMA = "ffone.semantic-audio-catalog.v5"


def clip_stem(true_name: str) -> str:
    """`Ice King_NanDismiss01` -> `ice_king_nandismiss01`, matching the installed set."""
    lowered = true_name.strip().lower()
    cleaned = []
    for character in lowered:
        if character.isalnum():
            cleaned.append(character)
        elif character in " -_.":
            cleaned.append("_")
        else:
            raise SystemExit(f"clip name {true_name!r} has an unsupported character {character!r}")
    stem = "".join(cleaned)
    while "__" in stem:
        stem = stem.replace("__", "_")
    return stem.strip("_")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-target", type=Path, required=True)
    parser.add_argument("--clips", type=Path, required=True)
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--report", type=Path, default=None)
    args = parser.parse_args()

    assets = args.native_target
    catalog_path = assets / CATALOG
    catalog = json.loads(catalog_path.read_text("utf-8"))
    if catalog["schema"] != CATALOG_SCHEMA:
        raise SystemExit(f"audio catalog schema must be {CATALOG_SCHEMA}")

    registry = json.loads((assets / "_runtime/characters.json").read_text("utf-8"))
    # A nano owner is the package route of an installed nano model, plus any name
    # the catalogue already owns clips under -- Belladonna and Computress keep an
    # owner without a separately installed model.
    nano_names = {model["glb"].split("/")[-2] for model in registry["models"] if model["category"] == "nano"}
    nano_names |= {entry["owner"] for entry in catalog["assets"] if entry["owner"].startswith("nano_")}
    existing_keys = {entry["logicalKey"] for entry in catalog["assets"]}
    existing_true_names = {entry["trueName"].strip().lower() for entry in catalog["assets"]}

    planned, skipped, problems = [], [], []
    for owner_dir in sorted(args.clips.iterdir()):
        if not owner_dir.is_dir():
            continue
        owner = owner_dir.name
        if owner not in nano_names:
            problems.append({"owner": owner, "reason": "no installed nano model carries this name"})
            continue
        for document_path in sorted(owner_dir.glob("*.json")):
            document = json.loads(document_path.read_text("utf-8"))
            value = document.get("value", {})
            true_name = value.get("m_Name")
            payload = (value.get("audio data") or {}).get("base64")
            if document.get("type") != "AudioClip" or not true_name or not payload:
                problems.append({"owner": owner, "clip": document_path.name, "reason": "not an AudioClip payload"})
                continue
            data = base64.b64decode(payload)
            if data[:4] != b"OggS":
                problems.append({"owner": owner, "clip": true_name, "reason": "payload is not an Ogg stream"})
                continue
            stem = clip_stem(true_name)
            relative = f"audio/voice/en/{owner}/{stem}.ogg"
            logical_key = f"voice/nanos/{owner}/{stem}"
            # A nano already owning clips is normal -- only the individual clip
            # decides. The trueName check keeps a second spelling of the same
            # source clip from landing twice under different stems.
            if (
                logical_key in existing_keys
                or true_name.strip().lower() in existing_true_names
                or (assets / relative).exists()
            ):
                skipped.append({"owner": owner, "clip": true_name, "reason": "already installed"})
                continue
            existing_true_names.add(true_name.strip().lower())
            existing_keys.add(logical_key)
            entry = {
                "logicalKey": logical_key,
                "trueName": true_name,
                "category": "voice",
                "owner": owner,
                "files": [{"locale": "en", "path": relative}],
            }
            # The installed set marks only the dismissal lines, which the runtime
            # plays when a nano is recovered.
            if "_nandismiss" in stem:
                entry["scope"] = "nano_recovery"
            planned.append({"entry": entry, "bytes": data, "relative": relative})

    if args.apply and planned:
        for item in planned:
            target = assets / item["relative"]
            target.parent.mkdir(parents=True, exist_ok=True)
            staged = target.with_suffix(".ogg.next")
            staged.write_bytes(item["bytes"])
            staged.replace(target)
        catalog["assets"].extend(item["entry"] for item in planned)
        catalog["assets"].sort(key=lambda entry: entry["logicalKey"])
        catalog["counts"]["assets"] = len(catalog["assets"])
        catalog["counts"]["voice"] = sum(1 for e in catalog["assets"] if e["category"] == "voice")
        staged = catalog_path.with_suffix(".json.next")
        staged.write_text(json.dumps(catalog, indent=1, ensure_ascii=False) + "\n", "utf-8")
        staged.replace(catalog_path)

    owners = sorted({item["entry"]["owner"] for item in planned})
    report = {
        "schema": "ffone.nano-voice-install.v1",
        "nativeTarget": str(assets),
        "applied": bool(args.apply and planned),
        "counts": {
            "clips": len(planned),
            "owners": len(owners),
            "skipped": len(skipped),
            "problems": len(problems),
            "catalogAssetsAfter": catalog["counts"]["assets"],
        },
        "owners": owners,
        "problems": problems,
    }
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=1, ensure_ascii=False) + "\n", "utf-8")
    print(f"clips={len(planned)} owners={len(owners)} skipped={len(skipped)} problems={len(problems)} applied={report['applied']}")
    for owner in owners:
        count = sum(1 for item in planned if item["entry"]["owner"] == owner)
        print(f"  {owner:24} {count:4} clips")
    for problem in problems[:10]:
        print(f"  PROBLEM {problem}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
