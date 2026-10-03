#!/usr/bin/env python3
"""Keep the owner-selected Retrobution 20260613 assets in FFOne.

recipes/native/retention/previous-build-retention.json declares UI textures,
character models, NPC texture overrides and NPC mesh rows whose previous-build
appearance the owner chose over the 20260821 primary revision. This tool
re-derives every declared payload from the clean previous build below Editor
work/, proves object identity and pixels, and installs only the declared native
files. Without a mode it plans; --check fails unless every output is retained;
--apply installs and writes the receipt. Run --check after every primary
migration, UI publication or table-set rebuild.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import io
import json
import os
import re
import shutil
import struct
import subprocess
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

from PIL import Image

import previous_build_retention as retention

EDITOR_ROOT = Path(__file__).resolve().parents[2]
SCHEMA = "fusionforge.native-publication.v1"
RECEIPT_ID = "previous-build-retention-20260912"
NPC_VISUAL_KEYS = [
    "m_fScale",
    "m_iHeight",
    "m_fAnimationSpeed",
    "m_fWalkAnimationSpeed",
    "m_fRunAnimationSpeed",
]
TABLE_SET = "data/tables/table-set.json"
NPC_TEXTURES = "data/tables/npc_texture_overrides.json"
WROTE = re.compile(r"^Wrote (\d+) bytes to (.+)$")


@dataclass
class Output:
    path: str
    payload: bytes
    before: bytes | None
    kind: str


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def rgba_sha256(data: bytes) -> str:
    with Image.open(io.BytesIO(data)) as image:
        return sha256(image.convert("RGBA").tobytes())


def f32(value: float) -> bytes:
    return struct.pack("<f", float(value))


def portable(path: Path) -> str:
    return Path(os.path.relpath(path.resolve(), EDITOR_ROOT)).as_posix()


def run(*command: object) -> list[str]:
    result = subprocess.run(
        [str(part) for part in command], cwd=EDITOR_ROOT, capture_output=True, text=True
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"{' '.join(map(str, command))} failed:\n{result.stdout}{result.stderr}"
        )
    return result.stdout.splitlines()


def fresh(path: Path) -> Path:
    if not path.resolve().is_relative_to((EDITOR_ROOT / "work").resolve()):
        raise RuntimeError(f"staging must stay below Editor work/: {path}")
    if path.exists():
        shutil.rmtree(path)
    path.mkdir(parents=True)
    return path


def json_bytes(value: object, template: bytes) -> bytes:
    """Serialize like the native tree does, keeping the file's newline style."""
    text = json.dumps(value, ensure_ascii=False, indent=2) + "\n"
    if b"\r\n" in template:
        text = text.replace("\n", "\r\n")
    return text.encode("utf-8")


def load_native_json(path: Path) -> tuple[dict, bytes]:
    raw = path.read_bytes()
    value = json.loads(raw)
    if json_bytes(value, raw) != raw:
        raise RuntimeError(f"{path} does not round-trip; refusing to reformat it")
    return value, raw


def glb_parts(data: bytes) -> tuple[dict, bytes]:
    magic, version, length = struct.unpack_from("<4sII", data, 0)
    json_length, json_type = struct.unpack_from("<II", data, 12)
    if magic != b"glTF" or version != 2 or length != len(data) or json_type != 0x4E4F534A:
        raise RuntimeError("not a single-JSON-chunk glTF 2.0 binary")
    return json.loads(data[20 : 20 + json_length]), data[20 + json_length :]


def glb_pack(document: dict, tail: bytes) -> bytes:
    chunk = json.dumps(document, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    chunk += b" " * (-len(chunk) % 4)
    header = struct.pack("<IIII", 2, 20 + len(chunk) + len(tail), len(chunk), 0x4E4F534A)
    return b"glTF" + header + chunk + tail


def rewrite(value: object, redirects: dict[str, str]) -> object:
    if isinstance(value, dict):
        return {key: rewrite(item, redirects) for key, item in value.items()}
    if isinstance(value, list):
        return [rewrite(item, redirects) for item in value]
    if isinstance(value, str):
        return redirects.get(value, value)
    return value


def texture_bindings(document: dict):
    for material in document.get("materials", []):
        ffone = (material.get("extras") or {}).get("ffone") or {}
        for binding in ffone.get("textureBindings", []):
            if binding.get("mipLevels"):
                yield binding


def sharing_contract(binding: dict) -> str:
    return json.dumps(
        {
            "colorSpace": binding.get("colorSpace"),
            "sampler": (binding.get("sampler") or {}).get("descriptor"),
            "levels": len(binding["mipLevels"]),
        },
        sort_keys=True,
    )


def exact_texture_rgba(tools: argparse.Namespace, container: Path, path_id: int, out: Path) -> str:
    run(tools.fusionforge, "export-exact-texture", container, path_id, out)
    payload = json.loads(out.read_text(encoding="utf-8"))["payload"]
    if any(payload[key] for key in ("alphaMaskApplied", "rgbRepair", "resized", "tintApplied")):
        raise RuntimeError(f"{container.name}:{path_id} exact payload is not a plain decode")
    return rgba_sha256(base64.b64decode(payload["dataUrl"].split("base64,", 1)[1]))


def verify_containers(declaration: dict, previous_build: Path) -> list[dict]:
    containers = []
    for name, expected in sorted(declaration["source"]["containers"].items()):
        data = (previous_build / name).read_bytes()
        if len(data) != expected["bytes"] or sha256(data) != expected["sha256"]:
            raise RuntimeError(f"previous-build container drifted: {name}")
        containers.append(
            {"relativeContainer": name, "containerBytes": len(data), "containerSha256": expected["sha256"]}
        )
    return containers


def stage_ui(entry: dict, args: argparse.Namespace, work: Path, target_root: Path) -> Output:
    container = args.previous_build / entry["container"]
    folder = fresh(work / "ui" / entry["route"].replace("/", "__"))
    exact = exact_texture_rgba(args, container, entry["pathId"], folder / "exact.json")
    writes = [
        int(match.group(1))
        for line in run(
            args.fusionforge, "unityextract", "--images", "-o", folder / "extract",
            "--filter", entry["extractFilter"], container,
        )
        if (match := WROTE.match(line.strip()))
        and Path(match.group(2)).name.lower() == entry["extractedFile"].lower()
    ]
    if len(writes) != 1:
        raise RuntimeError(
            f"{entry['route']}: filter {entry['extractFilter']!r} wrote "
            f"{entry['extractedFile']} {len(writes)} times; the object is ambiguous"
        )
    payload = (folder / "extract" / entry["extractedFile"]).read_bytes()
    if not rgba_sha256(payload) == exact == entry["rgbaSha256"]:
        raise RuntimeError(f"{entry['route']}: extracted pixels differ from PathID {entry['pathId']}")
    target = target_root / entry["route"]
    before = target.read_bytes() if target.is_file() else None
    allowed = {entry["rgbaSha256"], entry["declinedPrimaryRgbaSha256"]}
    if before is not None and rgba_sha256(before) not in allowed:
        raise RuntimeError(f"{entry['route']}: native target has unrelated edits")
    return Output(entry["route"], payload, before, "ui-texture")


def stage_model(
    entry: dict, args: argparse.Namespace, work: Path, target_root: Path, accepted: dict[str, str]
) -> tuple[Output, list[Path]]:
    folder = fresh(work / "models" / entry["logicalName"])
    source = folder / f"{entry['logicalName']}.source.json"
    run(
        args.fusionforge, "export-logical-model-source", args.previous_build / entry["container"],
        entry["route"], source, entry["navigationWorkDir"],
    )
    run(args.pipeline, "native", "publish-logical-model", source, entry["family"], folder / "candidate")
    candidate = folder / "candidate/models" / entry["family"] / f"{entry['logicalName']}.glb"
    document, tail = glb_parts(candidate.read_bytes())

    target = target_root / entry["path"]
    root = target_root.resolve()
    redirects: dict[str, str] = {}
    for image in document["images"]:
        levels = sorted(image["extras"]["ffone"]["mipLevels"], key=lambda level: level["level"])
        stem = Path(levels[0]["uri"]).stem
        shared = entry["sharedTextures"].get(stem)
        if shared is None:
            raise RuntimeError(f"{entry['path']}: no reviewed native owner for texture {stem}")
        for level in levels:
            local = candidate.parent / level["uri"]
            if level["level"] == 0:
                native_uri = f"{shared}.png"
            else:
                native_uri = f"{shared}.mips/{Path(level['uri']).name}"
            native = (target.parent / native_uri).resolve()
            if not native.is_relative_to(root) or not native.is_file():
                raise RuntimeError(f"{entry['path']}: {native_uri} is missing from the native tree")
            if not sha256(local.read_bytes()) == sha256(native.read_bytes()) == level["pngSha256"]:
                raise RuntimeError(f"{entry['path']}: {native_uri} is not the exact {stem} level {level['level']}")
            redirects[level["uri"]] = native_uri
        chain = (target.parent / f"{shared}.mips").resolve()
        expected = {Path(level["uri"]).name for level in levels[1:]}
        actual = {path.name for path in chain.iterdir()} if chain.is_dir() else set()
        if actual != expected:
            raise RuntimeError(f"{entry['path']}: native chain {shared} is not the complete source chain")

    document = rewrite(document, redirects)
    payload = glb_pack(document, tail)
    shared_files = {
        (target.parent / binding["mipLevels"][0]["uri"]).resolve()
        for binding in texture_bindings(document)
    }
    before = target.read_bytes() if target.is_file() else None
    allowed = {entry["declinedPrimarySha256"], sha256(payload), accepted.get(entry["path"])}
    if before is not None and sha256(before) not in allowed:
        raise RuntimeError(f"{entry['path']}: native target has unrelated edits")
    return Output(entry["path"], payload, before, "character-model"), [candidate, source, *shared_files]


def verify_sharing(document: dict, target: Path, target_root: Path) -> list[str]:
    """Each shared chain must already be used with this exact colour/sampler contract."""
    ours: dict[Path, str] = {}
    for binding in texture_bindings(document):
        ours[(target.parent / binding["mipLevels"][0]["uri"]).resolve()] = sharing_contract(binding)
    owners: dict[Path, set[str]] = {}
    for glb in (target_root / "characters").rglob("*.glb"):
        if glb.resolve() == target.resolve():
            continue
        other, _ = glb_parts(glb.read_bytes())
        for binding in texture_bindings(other):
            ref = (glb.parent / binding["mipLevels"][0]["uri"]).resolve()
            if ref in ours:
                owners.setdefault(ref, set()).add(sharing_contract(binding))
    notes = []
    for ref, contract in sorted(ours.items()):
        if ref in owners and contract not in owners[ref]:
            raise RuntimeError(f"{ref} is shared with an incompatible colour/sampler contract")
        route = ref.relative_to(target_root.resolve()).as_posix()
        notes.append(f"{route}: {'contract shared with existing owners' if ref in owners else 'no other owner'}")
    return notes


def find_table(value: object, key: str) -> dict | None:
    if isinstance(value, dict):
        if key in value:
            return value[key]
        children = value.values()
    elif isinstance(value, list):
        children = value
    else:
        return None
    for child in children:
        found = find_table(child, key)
        if found is not None:
            return found
    return None


def npc_name(table: dict, row: dict) -> str:
    return table["m_pNpcStringData"][row["m_iNpcName"]]["m_strName"]


def stage_npc_rows(spec: dict, args: argparse.Namespace, work: Path, table_doc: dict) -> tuple[list[dict], Path]:
    dump = fresh(work / "xdt") / "previous-xdtdatas.json"
    run(args.fusionforge, "dump-xdt", args.previous_build / spec["container"], spec["pathId"], dump)
    previous = find_table(json.loads(dump.read_text(encoding="utf-8")), spec["table"])
    native = next(t["value"][spec["table"]] for t in table_doc["tables"] if spec["table"] in t["value"])
    mesh = previous["m_pNpcMeshData"][spec["previousMeshRow"]]
    if mesh != spec["mesh"]:
        raise RuntimeError("previous mesh row differs from the declaration")
    rows = [i for i, row in enumerate(previous["m_pNpcData"]) if row["m_iMesh"] == spec["previousMeshRow"]]
    if rows != spec["rows"]:
        raise RuntimeError(f"previous rows selecting mesh {spec['previousMeshRow']} differ: {rows}")
    mesh_index = next(i for i, value in enumerate(native["m_pNpcMeshData"]) if value == mesh)
    changes = []
    for number in rows:
        row, source = native["m_pNpcData"][number], previous["m_pNpcData"][number]
        if npc_name(native, row) != npc_name(previous, source):
            raise RuntimeError(f"NPC row {number} is not the same NPC in the previous build")
        change: dict[str, list] = {}
        if row["m_iMesh"] != mesh_index:
            current = native["m_pNpcMeshData"][row["m_iMesh"]]
            if current != spec["declinedPrimaryMesh"]:
                raise RuntimeError(f"NPC row {number} selects an unrelated mesh {current}")
            change["m_iMesh"] = [row["m_iMesh"], mesh_index]
            row["m_iMesh"] = mesh_index
        for key in NPC_VISUAL_KEYS:
            if f32(row[key]) != f32(source[key]):
                change[key] = [row[key], source[key]]
                row[key] = source[key]
        if change:
            changes.append({"row": number, "name": npc_name(native, row), **change})
    return changes, dump


def stage_npc_texture(entry: dict, args: argparse.Namespace, work: Path, target_root: Path, doc: dict) -> Path:
    exact = fresh(work / "npc-textures" / entry["trueName"]) / "exact.json"
    rgba = exact_texture_rgba(args, args.previous_build / entry["container"], entry["pathId"], exact)
    payload = (target_root / entry["path"]).read_bytes()
    if not rgba == rgba_sha256(payload) == entry["rgbaSha256"] or sha256(payload) != entry["sha256"]:
        raise RuntimeError(f"{entry['path']} is not the previous-build {entry['trueName']} texture")
    record = next(t for t in doc["textures"] if t["trueName"] == entry["trueName"])
    if record["sampler"] != entry["sampler"]:
        raise RuntimeError(f"{entry['trueName']} sampler contract changed")
    allowed = {(entry["path"], entry["sha256"]), (entry["declinedPrimaryPath"], entry["declinedPrimarySha256"])}
    if (record["path"], record["sha256"]) not in allowed:
        raise RuntimeError(f"{entry['trueName']} override has unrelated edits")
    record["path"], record["sha256"] = entry["path"], entry["sha256"]
    return exact


def git_state() -> tuple[str, bool]:
    head = run("git", "rev-parse", "HEAD")[0].strip()
    dirty = bool("".join(run("git", "status", "--porcelain")).strip())
    return head, dirty


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--declaration", type=Path, default=retention.DECLARATION)
    parser.add_argument("--previous-build", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--target-root", type=Path, required=True)
    parser.add_argument("--receipt", type=Path)
    parser.add_argument("--fusionforge", type=Path, default=Path("target/release/fusionforge.exe"))
    parser.add_argument("--pipeline", type=Path, default=Path("target/debug/fusionforge.exe"))
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--apply", action="store_true")
    mode.add_argument("--check", action="store_true")
    args = parser.parse_args()
    for name in ("declaration", "previous_build", "work", "target_root", "fusionforge", "pipeline"):
        setattr(args, name, getattr(args, name).resolve())
    if args.receipt:
        args.receipt = args.receipt.resolve()
    target_root, work = args.target_root, args.work
    declaration = retention.load(args.declaration)
    accepted = {}
    if args.receipt and args.receipt.is_file():
        accepted = {o["path"]: o["sha256"] for o in json.loads(args.receipt.read_text(encoding="utf-8"))["outputs"]}

    containers = verify_containers(declaration, args.previous_build)
    outputs = [stage_ui(entry, args, work, target_root) for entry in declaration["ui"]]
    intermediates: list[Path] = [work / "ui" / e["route"].replace("/", "__") / "exact.json" for e in declaration["ui"]]
    sharing_notes = []
    for entry in declaration["models"]:
        output, files = stage_model(entry, args, work, target_root, accepted)
        outputs.append(output)
        intermediates.extend(files[:2])
        sharing_notes += verify_sharing(glb_parts(output.payload)[0], target_root / entry["path"], target_root)

    textures, textures_raw = load_native_json(target_root / NPC_TEXTURES)
    for entry in declaration["npcTextureOverrides"]:
        intermediates.append(stage_npc_texture(entry, args, work, target_root, textures))
    outputs.append(Output(NPC_TEXTURES, json_bytes(textures, textures_raw), textures_raw, "table-data"))

    tables, tables_raw = load_native_json(target_root / TABLE_SET)
    row_changes, dump = stage_npc_rows(declaration["npcMeshRows"], args, work, tables)
    intermediates.append(dump)
    outputs.append(Output(TABLE_SET, json_bytes(tables, tables_raw), tables_raw, "table-data"))

    def state(output: Output) -> str:
        if output.before is not None and output.before == output.payload:
            return "current"
        if output.before is not None and accepted.get(output.path) == sha256(output.before):
            return "accepted"
        return "pending"

    report = [
        {"path": o.path, "kind": o.kind, "state": state(o), "sha256": sha256(o.payload),
         "beforeSha256": None if o.before is None else sha256(o.before)}
        for o in outputs
    ]
    (work / "plan.json").write_text(json.dumps({"outputs": report, "npcRowChanges": row_changes}, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({o["path"]: o["state"] for o in report}, indent=2))
    print(f"NPC rows to restore: {len(row_changes)}")
    if args.check:
        pending = [o["path"] for o in report if o["state"] == "pending"]
        if pending:
            raise SystemExit(f"previous-build retention is not current: {pending}")
        return
    if not args.apply:
        return

    for output in outputs:
        if output.before is not None and output.before != output.payload:
            backup = work / "backup" / output.path
            backup.parent.mkdir(parents=True, exist_ok=True)
            backup.write_bytes(output.before)
    for output in outputs:
        target = target_root / output.path
        staged = target.with_name(target.name + ".next")
        staged.write_bytes(output.payload)
        os.replace(staged, target)
        if target.read_bytes() != output.payload:
            raise RuntimeError(f"published target verification failed: {output.path}")

    if args.receipt is None:
        return
    head, dirty = git_state()
    tools = [Path(__file__), Path(retention.__file__)]
    receipt = {
        "schema": SCHEMA,
        "id": RECEIPT_ID,
        "status": "accepted",
        "generatedAtUtc": datetime.now(timezone.utc).isoformat(),
        "supersedes": [],
        "supersedesOutputsOf": declaration["supersedesOutputsOf"],
        "target": {"alias": "ffone-client"},
        "source": {
            "alias": declaration["source"]["alias"],
            "build": declaration["source"]["build"],
            "containers": containers,
            "objects": declaration_objects(declaration),
        },
        "transform": {
            "kind": "owner-selected-previous-build-retention",
            "tool": portable(Path(__file__)),
            "toolRevision": head,
            "workingTreeDirty": dirty,
            "toolSha256": {portable(path): sha256(path.read_bytes()) for path in tools},
            "declaration": {"path": portable(args.declaration), "sha256": sha256(args.declaration.read_bytes())},
            "binaries": [
                {"path": portable(path), "sha256": sha256(path.read_bytes())}
                for path in (args.fusionforge, args.pipeline)
            ],
            "workingDirectory": ".",
            "commands": [
                "python tools/legacy-sources/retain-previous-build-assets.py"
                f" --previous-build {portable(args.previous_build)} --work {portable(work)}"
                f" --target-root {portable(target_root)} --receipt {portable(args.receipt)} --apply"
            ],
        },
        "inputs": [
            {"path": portable(path), "bytes": path.stat().st_size, "sha256": sha256(path.read_bytes())}
            for path in intermediates
        ],
        "outputs": [
            {"path": o.path, "bytes": len(o.payload), "sha256": sha256(o.payload),
             "beforeSha256": None if o.before is None else sha256(o.before)}
            for o in outputs
        ],
        "npcRowChanges": row_changes,
        "divergenceFromPrimary": declaration["decision"]["summary"],
        "verification": [
            {"command": "retain-previous-build-assets.py identity checks", "result": "passed",
             "evidence": "every UI texture decodes to its declared PathID pixels; every model texture "
                         "level is byte-identical to its declared native chain; "
                         "NPC rows and mesh row match the raw previous XDT"},
            {"command": "shared texture contract", "result": "passed", "evidence": sharing_notes},
        ],
    }
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print(f"receipt: {portable(args.receipt)}")


def declaration_objects(declaration: dict) -> list[dict]:
    objects = [
        {"relativeContainer": e["container"], "serializedAsset": e["serializedAsset"], "type": "Texture2D",
         "pathId": e["pathId"], "trueName": e["unityName"], "nativeRoute": e["route"]}
        for e in declaration["ui"]
    ]
    objects += [
        {"relativeContainer": e["container"], "serializedAsset": e["serializedAsset"], "type": "GameObject",
         "pathId": e["pathId"], "containerRoute": e["route"], "trueName": e["logicalName"], "nativeRoute": e["path"]}
        for e in declaration["models"]
    ]
    objects += [
        {"relativeContainer": e["container"], "serializedAsset": e["serializedAsset"], "type": "Texture2D",
         "pathId": e["pathId"], "containerRoute": e["containerRoute"], "trueName": e["trueName"], "nativeRoute": e["path"]}
        for e in declaration["npcTextureOverrides"]
    ]
    rows = declaration["npcMeshRows"]
    objects.append(
        {"relativeContainer": rows["container"], "serializedAsset": rows["serializedAsset"], "type": "MonoBehaviour",
         "pathId": rows["pathId"], "trueName": "xdtdatas", "nativeRoute": f"{TABLE_SET}#{rows['table']}"}
    )
    return objects


if __name__ == "__main__":
    main()
