"""Publish the map/target icons of the NPCs the modded build added.

`avatar_util_semantic_icon_path` builds an icon path straight from the table:
`m_pNpcIconData[m_iIcon1]` gives an `m_iIconType` and an `m_iIconNumber`, and the
runtime opens `icons/entities/<family>/<prefix>_<number>.png`. The added NPCs
carry icon numbers equal to their own NPC id, and those PNGs ship inside the
per-NPC snapshot bundles rather than in any build's `Icons.resourceFile`, so
nothing in the clean pipeline installs them.

The tool is additive and refuses to overwrite: an icon number that already has a
file belongs to the clean build and is never touched.

    python tools/legacy-sources/install-modded-npc-icons.py --native-target ../FFOneClient/assets/game
    python tools/legacy-sources/install-modded-npc-icons.py --native-target ../FFOneClient/assets/game --apply
"""

from __future__ import annotations

import argparse
import base64
import json
import struct
import subprocess
import sys
from pathlib import Path

SNAPSHOT_ROOT = Path("../builds/retrobution-20260613.ffclient/npcs")
WORK = Path("work/legacy-sources/modded-npc-icons-20260903")

# m_iIconType -> (directory under icons/entities, filename prefix). Only the
# families the added NPCs actually use are listed; an unexpected type is an
# error rather than a guess.
ICON_FAMILIES = {
    4: ("npc", "npcicon"),
    8: ("mobs", "mobicon"),
    10: ("hnpc", "hnpcicon"),
}


def run(*args: str) -> str:
    result = subprocess.run(
        args, capture_output=True, text=True, encoding="utf-8", errors="replace"
    )
    if result.returncode != 0:
        raise SystemExit(f"command failed: {' '.join(args)}\n{result.stdout}\n{result.stderr}")
    return result.stdout


def bundle_file(directory: Path) -> Path:
    files = [entry for entry in sorted(directory.iterdir()) if entry.is_file()]
    if len(files) != 1:
        raise SystemExit(f"{directory} holds {len(files)} files, expected exactly one bundle")
    return files[0]


def texture_path_id(bundle: Path, name: str) -> int:
    listing = run("cargo", "run", "--quiet", "--", "fusionforge", "list-contents", str(bundle))
    matches = []
    for line in listing.splitlines():
        parts = line.split("\t")
        if len(parts) >= 4 and parts[2] == "Texture2D" and parts[3] == name:
            matches.append(int(parts[0]))
    if len(matches) != 1:
        raise SystemExit(f"{bundle} holds {len(matches)} Texture2D named {name!r}, expected one")
    return matches[0]


def png_size(data: bytes) -> tuple[int, int]:
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit("exported payload is not a PNG")
    width, height = struct.unpack(">II", data[16:24])
    return width, height


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-target", type=Path, required=True)
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--report", type=Path, default=WORK / "install-report.json")
    args = parser.parse_args()

    assets = args.native_target
    tables = json.loads((assets / "data/tables/table-set.json").read_text("utf-8"))
    npc_table = tables["tables"][0]["value"]["m_pNpcTable"]
    npc_rows, icon_rows = npc_table["m_pNpcData"], npc_table["m_pNpcIconData"]

    WORK.mkdir(parents=True, exist_ok=True)
    planned, skipped, problems = [], [], []
    seen: dict[tuple[int, int], int] = {}

    for record in sorted(SNAPSHOT_ROOT.glob("*/*.npc-import.json")):
        blueprint = json.loads(record.read_text("utf-8"))["blueprint"]
        npc_id = blueprint["npcId"]
        icon_asset = blueprint.get("iconAsset")
        icon_bundle = blueprint.get("iconBundle")
        if not icon_asset or not icon_bundle:
            problems.append({"npcId": npc_id, "reason": "record names no icon"})
            continue

        row = npc_rows[npc_id]
        icon_index = row.get("m_iIcon1", 0)
        if not 0 <= icon_index < len(icon_rows):
            problems.append({"npcId": npc_id, "reason": f"m_iIcon1 {icon_index} out of range"})
            continue
        icon_row = icon_rows[icon_index]
        icon_type, icon_number = icon_row["m_iIconType"], icon_row["m_iIconNumber"]
        family = ICON_FAMILIES.get(icon_type)
        if family is None:
            problems.append({"npcId": npc_id, "reason": f"unmapped icon type {icon_type}"})
            continue
        directory, prefix = family
        relative = f"icons/entities/{directory}/{prefix}_{icon_number:02}.png"
        target = assets / relative

        # Several NPCs legitimately share one icon row (3147 and 3406 do).
        key = (icon_type, icon_number)
        if key in seen:
            skipped.append({"npcId": npc_id, "relative": relative, "reason": f"already planned for NPC {seen[key]}"})
            continue
        if target.exists():
            skipped.append({"npcId": npc_id, "relative": relative, "reason": "already installed"})
            continue
        seen[key] = npc_id

        source_name = Path(icon_asset).stem
        bundle_directory = Path("..") / "builds/retrobution-20260613.ffclient" / icon_bundle
        if bundle_directory.is_dir():
            bundle = bundle_file(bundle_directory)
        elif bundle_directory.is_file():
            bundle = bundle_directory
        else:
            problems.append({"npcId": npc_id, "reason": f"icon bundle {icon_bundle} is missing"})
            continue

        path_id = texture_path_id(bundle, source_name)
        exported = WORK / f"{source_name}.json"
        run(
            "cargo", "run", "--quiet", "--", "fusionforge", "export-exact-texture",
            str(bundle), str(path_id), str(exported),
        )
        document = json.loads(exported.read_text("utf-8"))
        data_url = document["payload"]["dataUrl"]
        marker = "base64,"
        png = base64.b64decode(data_url[data_url.index(marker) + len(marker):])
        width, height = png_size(png)
        planned.append(
            {
                "npcId": npc_id,
                "sourceName": source_name,
                "sourceBundle": str(bundle),
                "pathId": path_id,
                "iconType": icon_type,
                "iconNumber": icon_number,
                "relative": relative,
                "width": width,
                "height": height,
                "byteLength": len(png),
            }
        )
        if args.apply:
            target.parent.mkdir(parents=True, exist_ok=True)
            staged = target.with_suffix(".png.next")
            staged.write_bytes(png)
            staged.replace(target)

    report = {
        "schema": "ffone.modded-npc-icon-install.v1",
        "nativeTarget": str(assets),
        "applied": args.apply,
        "counts": {
            "installed": len(planned),
            "skipped": len(skipped),
            "problems": len(problems),
        },
        "installed": planned,
        "skipped": skipped,
        "problems": problems,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=1, ensure_ascii=False) + "\n", "utf-8")
    print(f"installed={len(planned)} skipped={len(skipped)} problems={len(problems)} applied={args.apply}")
    for entry in planned:
        print(f"  {entry['relative']:44} {entry['width']}x{entry['height']} from {entry['sourceName']}")
    for entry in problems:
        print(f"  PROBLEM npc {entry['npcId']}: {entry['reason']}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
