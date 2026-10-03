#!/usr/bin/env python3
"""Publish the focused Retrobution 2026-08-21 UI texture revision to FFOne.

The Unity extraction and comparison reports remain below FusionForge's ignored
work tree. Only decoded PNGs with semantic destinations cross into FFOneClient.
The Nanocom Clock textures are deliberately excluded. Routes the owner kept at the
previous 20260613 appearance (recipes/native/retention/previous-build-retention.json)
are skipped; retain-previous-build-assets.py owns them.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

from PIL import Image

import previous_build_retention


SCHEMA = "ffone.retrobution-ui-publication.v1"
EXPECTED_CLOCK_TEXTURES = {
    "pingbar_bg.png",
    "pingbar_empty.png",
    "pingbar_1.png",
    "pingbar_2.png",
    "pingbar_3.png",
}

MAIN_MANUAL_ROUTES = {
    "ChatBubbleIcon_buddy.png": "ui/gameplay/chat/icons/buddy.png",
    "ChatBubbleIcon_group.png": "ui/gameplay/chat/icons/group.png",
    "CombiIcon.png": "ui/gameplay/interaction/icons/combine.png",
    "CombiRulesIcon.png": "ui/gameplay/interaction/icons/combine-rules.png",
    "FirstUseComputress.png": "ui/gameplay/hints/first-use-computress.png",
    "TimeMachine_Icon.png": "ui/gameplay/interaction/icons/time-machine.png",
    "aim_grenade_line.png": "ui/gameplay/aiming/grenade-line.png",
    "aim_rocket_leftbracket.png": "ui/gameplay/aiming/rocket-left-bracket.png",
    "aim_rocket_point.png": "ui/gameplay/aiming/rocket-point.png",
    "ben_banner.png": "ui/gameplay/mission/guide-banners/ben.png",
    "chatCornerDrag_off.png.png": "ui/gameplay/chat/resize-normal.png",
    "chatCornerDrag_on.png.png": "ui/gameplay/chat/resize-hover.png",
    "compu_banner.png": "ui/gameplay/mission/guide-banners/computress.png",
    "dexter_banner.png": "ui/gameplay/mission/guide-banners/dexter.png",
    "edd_banner.png": "ui/gameplay/mission/guide-banners/edd.png",
    "load.png": "ui/gameplay/loading/load.png",
    "map_icon_25_bgactive.tga.png": "ui/world-map/markers/map_icon_25_active.png",
    "map_icon_woosh.png": "ui/world-map/markers/map_icon_woosh.png",
    "mb_009.png": "ui/gameplay/speech/mission-tail.png",
    "mbubble.png": "ui/gameplay/speech/mission-box.png",
    "mojo_banner.png": "ui/gameplay/mission/guide-banners/mojo.png",
    "neonshinebg.png": "ui/world-map/markers/neon-shine-background.png",
    "newEmoteMenuBox.png": "ui/gameplay/chat/emote-menu-box.png",
    "npcicon_auction.png": "ui/gameplay/interaction/icons/auction.png",
    "q_bubble.png": "ui/gameplay/speech/quick-chat-box.png",
    "qb_009.png": "ui/gameplay/speech/quick-chat-tail.png",
    "setting_back_tmp.png": "ui/option/panel-main.png",
    "swordtail.png": "ui/gameplay/effects/sword-tail.png",
    "ui_searchtab2.png.png": "ui/bank-mode/search-field.png",
    "user_win.png": "ui/gameplay/interaction/player/window.png",
    "usericon_antibuddy.png": "ui/gameplay/interaction/player/remove-buddy.png",
    "usericon_buddy.png": "ui/gameplay/interaction/player/add-buddy.png",
    "usericon_group.png": "ui/gameplay/interaction/player/group.png",
    "usericon_trade.png": "ui/gameplay/interaction/player/trade.png",
}

CHARACTER_CREATION_SELECTION = {
    "CSBG.png",
    "CharCreationBG.png",
    "HelpSmallBG.png",
    "login_screen_bg_16x10.png",
}
CHARACTER_CREATION_MANUAL_ROUTES = {
    "HelpSmallBG.png": "ui/character/creation/help/background.png",
}

EMOTE_ICON_ROUTES = {
    "AngryIcon.png": "angry.png",
    "cheerIcon.png": "cheer.png",
    "chill_outIcon.png": "chill-out.png",
    "clapIcon.png": "clap.png",
    "danceIcon.png": "dance.png",
    "extrasIcon.png": "extras.png",
    "flexIcon.png": "flex.png",
    "happyIcon.png": "happy.png",
    "loveIcon.png": "love.png",
    "noIcon.png": "no.png",
    "okayIcon.png": "okay.png",
    "sadIcon.png": "sad.png",
    "scaredIcon.png": "scared.png",
    "tauntIcon.png": "taunt.png",
    "thanksIcon.png": "thanks.png",
    "yesIcon.png": "yes.png",
}


@dataclass(frozen=True)
class Publication:
    container: str
    source_name: str
    source: Path
    destination: Path
    old_rgba_sha256: str | None
    new_rgba_sha256: str
    route_kind: str


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def rgba_sha256(path: Path) -> str:
    with Image.open(path) as image:
        return hashlib.sha256(image.convert("RGBA").tobytes()).hexdigest()


def load_report(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if value.get("schema") != "ffone.retrobution-ui-texture-diff.v1":
        raise RuntimeError(f"unsupported diff report: {path}")
    return value


def report_root(project: Path, report: dict[str, object], field: str) -> Path:
    value = Path(str(report[field]))
    if value.is_absolute():
        return value.resolve()
    editor_root = project.resolve().parents[2]
    return (editor_root / value).resolve()


def record_map(report: dict[str, object]) -> dict[str, dict[str, object]]:
    return {
        str(record["unityPng"]): record
        for record in report["changedSelected"]
    }


def add_publication(
    plan: dict[Path, Publication],
    *,
    native_root: Path,
    new_root: Path,
    container: str,
    record: dict[str, object],
    destination: str,
    route_kind: str,
) -> None:
    source = (new_root / str(record["new"]["path"])).resolve()
    target = (native_root / destination).resolve()
    if native_root.resolve() not in target.parents:
        raise RuntimeError(f"destination escaped native root: {target}")
    if not source.is_file():
        raise RuntimeError(f"decoded source is missing: {source}")

    source_rgba = rgba_sha256(source)
    expected_rgba = str(record["new"]["rgbaSha256"])
    if source_rgba != expected_rgba:
        raise RuntimeError(f"source pixel hash drifted for {source}")

    old = record.get("old")
    publication = Publication(
        container=container,
        source_name=str(record["unityPng"]),
        source=source,
        destination=target,
        old_rgba_sha256=None if old is None else str(old["rgbaSha256"]),
        new_rgba_sha256=expected_rgba,
        route_kind=route_kind,
    )
    previous = plan.get(target)
    if previous is not None and previous.new_rgba_sha256 != publication.new_rgba_sha256:
        raise RuntimeError(
            f"conflicting sources for {target}: {previous.source_name} and "
            f"{publication.source_name}"
        )
    plan[target] = publication


def add_pixel_routes(
    plan: dict[Path, Publication],
    *,
    native_root: Path,
    new_root: Path,
    container: str,
    record: dict[str, object],
    prefix: str,
) -> int:
    matches = [str(value) for value in record["nativePixelMatches"]]
    for match in matches:
        add_publication(
            plan,
            native_root=native_root,
            new_root=new_root,
            container=container,
            record=record,
            destination=f"{prefix}{match}",
            route_kind="old-pixel-identity",
        )
    return len(matches)


def build_plan(project: Path, native_root: Path) -> tuple[dict[Path, Publication], dict[str, object]]:
    reports_root = project / "reports"
    main = load_report(reports_root / "main-texture-diff.json")
    character = load_report(reports_root / "character-creation-texture-diff.json")
    tutorial = load_report(reports_root / "tutorial-texture-diff.json")
    icons = load_report(reports_root / "icons-texture-diff.json")

    excluded = {
        str(record["unityPng"]) for record in main["excludedNanocomClock"]
    }
    if excluded != EXPECTED_CLOCK_TEXTURES:
        raise RuntimeError(f"Nanocom Clock exclusion drifted: {sorted(excluded)}")

    plan: dict[Path, Publication] = {}
    main_root = report_root(project, main, "newRoot")
    for record in main["changedSelected"]:
        if add_pixel_routes(
            plan,
            native_root=native_root,
            new_root=main_root,
            container="main.unity3d",
            record=record,
            prefix="ui/",
        ):
            continue
        name = str(record["unityPng"])
        destination = MAIN_MANUAL_ROUTES.get(name)
        if destination is None:
            raise RuntimeError(f"unrouted changed main UI texture: {name}")
        add_publication(
            plan,
            native_root=native_root,
            new_root=main_root,
            container="main.unity3d",
            record=record,
            destination=destination,
            route_kind="reviewed-semantic-route",
        )

    character_root = report_root(project, character, "newRoot")
    character_records = record_map(character)
    for name in sorted(CHARACTER_CREATION_SELECTION):
        record = character_records[name]
        if name != "HelpSmallBG.png" and add_pixel_routes(
            plan,
            native_root=native_root,
            new_root=character_root,
            container="CharacterCreation.resourceFile",
            record=record,
            prefix="ui/",
        ):
            continue
        add_publication(
            plan,
            native_root=native_root,
            new_root=character_root,
            container="CharacterCreation.resourceFile",
            record=record,
            destination=CHARACTER_CREATION_MANUAL_ROUTES[name],
            route_kind="reviewed-semantic-route",
        )

    tutorial_root = report_root(project, tutorial, "newRoot")
    for record in tutorial["changedSelected"]:
        name = str(record["unityPng"])
        if name.startswith("TutorialPan_") or name == "nanoicon_34.png":
            if not add_pixel_routes(
                plan,
                native_root=native_root,
                new_root=tutorial_root,
                container="Tutorial.resourceFile",
                record=record,
                prefix="",
            ):
                raise RuntimeError(f"missing native identity route for {name}")
            continue
        if name == "dexter_hologram.png":
            destination = "ui/gameplay/guide/holograms/dexter.png"
        elif name in EMOTE_ICON_ROUTES:
            destination = f"ui/gameplay/chat/emotes/{EMOTE_ICON_ROUTES[name]}"
        else:
            continue
        add_publication(
            plan,
            native_root=native_root,
            new_root=tutorial_root,
            container="Tutorial.resourceFile",
            record=record,
            destination=destination,
            route_kind="reviewed-semantic-route",
        )

    icons_root = report_root(project, icons, "newRoot")
    nano_rows = 0
    for record in icons["changedSelected"]:
        name = str(record["unityPng"])
        if re.fullmatch(r"(?:nanoicon|nanoready)_\d+\.png", name) is None:
            continue
        nano_rows += 1
        if not add_pixel_routes(
            plan,
            native_root=native_root,
            new_root=icons_root,
            container="Icons.resourceFile",
            record=record,
            prefix="",
        ):
            raise RuntimeError(f"missing native Nano icon route for {name}")
    if nano_rows != 83:
        raise RuntimeError(f"expected 83 changed Nano icon rows, got {nano_rows}")

    retained = previous_build_retention.retained_ui_routes()
    kept = sorted(
        path for path in plan if path.relative_to(native_root).as_posix() in retained
    )
    for path in kept:
        del plan[path]

    metadata = {
        "retainedPreviousBuildRoutes": [
            path.relative_to(native_root).as_posix() for path in kept
        ],
        "mainChangedUiTextures": len(main["changedSelected"]),
        "characterCreationSelectedTextures": len(CHARACTER_CREATION_SELECTION),
        "tutorialSelectedTextures": sum(
            1
            for record in tutorial["changedSelected"]
            if str(record["unityPng"]).startswith("TutorialPan_")
            or str(record["unityPng"]) == "nanoicon_34.png"
            or str(record["unityPng"]) == "dexter_hologram.png"
            or str(record["unityPng"]) in EMOTE_ICON_ROUTES
        ),
        "nanoIconSourceTextures": nano_rows,
        "excludedNanocomClockTextures": sorted(EXPECTED_CLOCK_TEXTURES),
    }
    return plan, metadata


def validate_current(plan: dict[Path, Publication]) -> None:
    for publication in plan.values():
        target = publication.destination
        if not target.exists() or publication.route_kind != "old-pixel-identity":
            continue
        current = rgba_sha256(target)
        if current not in {publication.old_rgba_sha256, publication.new_rgba_sha256}:
            raise RuntimeError(f"native target has unrelated edits: {target}")


def apply_plan(plan: dict[Path, Publication]) -> None:
    for publication in plan.values():
        publication.destination.parent.mkdir(parents=True, exist_ok=True)
    for publication in plan.values():
        next_path = publication.destination.with_name(publication.destination.name + ".next")
        shutil.copyfile(publication.source, next_path)
        if rgba_sha256(next_path) != publication.new_rgba_sha256:
            raise RuntimeError(f"staged PNG verification failed: {next_path}")
    for publication in plan.values():
        next_path = publication.destination.with_name(publication.destination.name + ".next")
        os.replace(next_path, publication.destination)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--project", required=True, type=Path)
    parser.add_argument("--old-build", required=True, type=Path)
    parser.add_argument("--new-build", required=True, type=Path)
    parser.add_argument("--native-root", required=True, type=Path)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()

    project = args.project.resolve()
    native_root = args.native_root.resolve()
    plan, metadata = build_plan(project, native_root)
    validate_current(plan)

    source_containers = sorted({publication.container for publication in plan.values()})
    inputs = []
    for build_id, root in (
        ("retrobution-20260613", args.old_build.resolve()),
        ("retrobution-20260821", args.new_build.resolve()),
    ):
        for container in source_containers:
            path = root / container
            if not path.is_file():
                raise RuntimeError(f"source container is missing: {path}")
            inputs.append({
                "buildId": build_id,
                "container": container,
                "bytes": path.stat().st_size,
                "sha256": sha256(path),
            })

    if args.apply:
        apply_plan(plan)

    outputs = []
    for publication in sorted(plan.values(), key=lambda value: value.destination.as_posix()):
        state = "missing"
        output_sha = None
        output_bytes = None
        if publication.destination.is_file():
            state = (
                "current"
                if rgba_sha256(publication.destination) == publication.new_rgba_sha256
                else "stale"
            )
            output_sha = sha256(publication.destination)
            output_bytes = publication.destination.stat().st_size
        if args.apply and state != "current":
            raise RuntimeError(f"published target verification failed: {publication.destination}")
        outputs.append({
            "route": publication.destination.relative_to(native_root).as_posix(),
            "container": publication.container,
            "sourceName": publication.source_name,
            "routeProof": publication.route_kind,
            "sourceRgbaSha256": publication.new_rgba_sha256,
            "outputBytes": output_bytes,
            "outputSha256": output_sha,
            "state": state,
        })

    receipt = {
        "schema": SCHEMA,
        "generatedAtUtc": datetime.now(timezone.utc).isoformat(),
        "status": "published-verified" if args.apply else "planned",
        "sourceRole": "primary",
        "supersedesPrimaryRevision": "retrobution-20260613",
        "sourceBuild": "retrobution-20260821",
        "selection": metadata,
        "counts": {
            "publishedRoutes": len(outputs),
            "currentRoutes": sum(output["state"] == "current" for output in outputs),
            "missingRoutes": sum(output["state"] == "missing" for output in outputs),
        },
        "inputs": inputs,
        "outputs": outputs,
        "replay": {
            "extract": "fusionforge fusionforge unityextract --images -o <staging> <container>",
            "compare": "work/projects/retrobution-ui-20260821.ffclient/compare_ui_textures.py",
            "publish": "python tools/legacy-sources/publish-retrobution-ui-20260821.py --project <project> --old-build <old> --new-build <new> --native-root <assets/game> --receipt <receipt> --apply",
        },
        "intentionalExclusion": "Nanocom Clock ping/connection shelf textures and behavior were explicitly declined by the owner.",
    }
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(receipt["counts"], indent=2))


if __name__ == "__main__":
    main()
