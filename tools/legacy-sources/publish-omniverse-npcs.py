"""Stage additive Omniverse NPC types; never create or change world placements.

Inputs are scoped FusionForge exports and GPU captures under Editor work/. This
tool installs only declared native outputs, after a second identical staging run.
See recipes/native/characters/omniverse-npcs-20260905.json for raw replay commands.
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess


VARIANTS = [
    # Stable new type, original type, semantic route, exact model, main texture, icon.
    (3464, 732, "omniverse_ben", "npc_bentennyson", "npc_ben", 68,
     "Ben Tennyson (Omniverse)", "Бен Теннисон (Омниверс)"),
    (3465, 738, "omniverse_gwen", "npc_gwen1", "npc_gwen", 69,
     "Gwen Tennyson (Omniverse)", "Гвен Теннисон (Омниверс)"),
    (3466, 749, "omniverse_kevin", "npc_kevinlevin", "npc_kevin", 70,
     "Kevin Levin (Omniverse)", "Кевин Левин (Омниверс)"),
    (3467, 697, "omniverse_albedo", "npc_albedo", "npc_albedo2", None,
     "Albedo (Omniverse)", "Альбедо (Омниверс)"),
    (3468, 725, "omniverse_max", "npc_max", "npc_max", 23,
     "Grandpa Max (Omniverse)", "Дедушка Макс (Омниверс)"),
]
DOCUMENTS = ["_runtime/characters.json", "data/tables/table-set.json",
             "data/tables/npc_texture_overrides.json", "localization/en.json",
             "localization/ru.json"]


def read(p):
    return json.loads(p.read_text(encoding="utf-8"))


def write(p, value):
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()


def write_server(path, value, original):
    raw = original.read_bytes()
    indent = 4 if b'\n    "NPCs"' in raw else 2
    body = json.dumps(value, ensure_ascii=False, indent=indent) + "\n"
    if b"\r\n" in raw:
        body = body.replace("\n", "\r\n")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(body.encode("utf-8"))


def native_npcs(document):
    tables = [t["value"]["m_pNpcTable"] for t in document["tables"]
              if "m_pNpcTable" in t["value"]]
    assert len(tables) == 1
    return tables[0]


def add_rows(native, primary, locales):
    before = copy.deepcopy(native)
    assert len(native["m_pNpcData"]) == 3464, "unexpected NPC append point"
    for number, original, route, model, texture, icon, en, ru in VARIANTS:
        row = copy.deepcopy(before["m_pNpcData"][original])
        for key in ("m_fScale", "m_iHeight", "m_fAnimationSpeed",
                    "m_fWalkAnimationSpeed", "m_fRunAnimationSpeed"):
            row[key] = primary["m_pNpcData"][original][key]
        string_index = len(native["m_pNpcStringData"])
        string = copy.deepcopy(before["m_pNpcStringData"][row["m_iNpcName"]])
        original_string = row["m_iNpcName"]
        string["m_strName"] = en
        native["m_pNpcStringData"].append(string)
        row.update(m_iNpcNumber=number, m_iNpcName=string_index,
                   m_iComment=string_index, m_iNpcType=3, m_iServiceNumber=0,
                   m_iMapIcon=18, m_iMesh=len(native["m_pNpcMeshData"]))
        # The original NPC retains guide/services and all mission ownership.
        mesh = copy.deepcopy(before["m_pNpcMeshData"][before["m_pNpcData"][original]["m_iMesh"]])
        mesh.update(m_pstrMMeshModelString=route,
                    m_pstrMTextureString=route + "_body", m_pstrMTextureString2="null",
                    m_pstrFMeshModelString="null", m_pstrFTextureString="null",
                    m_pstrFTextureString2="null")
        # Face is the exact embedded primary texture in Ben/Albedo's GLB.
        native["m_pNpcMeshData"].append(mesh)
        if icon is not None:
            row["m_iIcon1"] = len(native["m_pNpcIconData"])
            native["m_pNpcIconData"].append({"m_iIconNumber": number, "m_iIconType": 4})
        native["m_pNpcData"].append(row)
        for locale, label in (("en", en), ("ru", ru)):
            entries = locales[locale]["entries"]
            entries[f"content.npc.{number}.name"] = label
            old_prefix = f"content.tabledata.npc.npc_string.{original_string}."
            new_prefix = f"content.tabledata.npc.npc_string.{string_index}."
            for key, value in list(entries.items()):
                if key.startswith(old_prefix):
                    entries[new_prefix + key[len(old_prefix):]] = value
            entries[new_prefix + "str_name"] = label
    for key, value in before.items():
        if isinstance(value, list):
            assert native[key][:len(value)] == value, f"changed existing {key}"
    assert locales["en"]["entries"].keys() == locales["ru"]["entries"].keys()


def sampler(exact):
    settings = exact["importSettings"]["m_TextureSettings"]
    assert exact["mipCount"] == 1 and settings["m_FilterMode"] == 1
    wrap = "repeat" if settings["m_WrapMode"] == 0 else "clampToEdge"
    return {"name": exact["name"], "minFilter": "linear", "magFilter": "linear",
            "wrapS": wrap, "wrapT": wrap, "legacyFilterMode": 1,
            "legacyWrapMode": settings["m_WrapMode"],
            "anisotropyLevel": settings["m_Aniso"], "mipMapBias": settings["m_MipBias"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ("work", "native-root", "server-root", "stage", "source-root", "primary-evidence"):
        parser.add_argument("--" + flag, required=True, type=Path)
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()
    assert not args.stage.exists(), "stage must be fresh"
    evidence = read(args.primary_evidence)
    raw = args.source_root / evidence["source"]["relativeContainer"]
    assert sha(raw) == evidence["source"]["sha256"]
    primary = evidence["object"]["value"]["m_pNpcTable"]
    staged = args.stage / "native"
    for relative in DOCUMENTS:
        path = staged / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(args.native_root / relative, path)
    (staged / "characters/npcs").mkdir(parents=True)
    pipeline = Path("target/debug/fusionforge.exe").resolve()
    textures = read(staged / DOCUMENTS[2])
    for number, original, route, model, texture, icon, *_ in VARIANTS:
        candidate = "candidate-f32" if model == "npc_bentennyson" else "candidate"
        gpu = "gpu-f32" if model == "npc_bentennyson" else "gpu"
        smoke = read(args.work / "dressed-sampled" / f"{model}.json")
        assert smoke["status"] == "success" and smoke["xdtTextureOverrides"]["mainStatus"] == "bound"
        command = [str(pipeline), 'native', "install-runtime-character-model", str(staged),
                   "npc/" + route, str(args.work / candidate / "models/npc" / f"{model}.glb"),
                   str(args.work / gpu / "models/npc" / f"{model}.gpu.json")]
        result = subprocess.run(command, capture_output=True, text=True, check=True)
        (args.stage / f"{route}.install.log").write_text(result.stdout, encoding="utf-8")
        exact = read(args.work / "textures" / f"{texture}.exact.json")
        relative = f"characters/npcs/{route}/body.png"
        shutil.copyfile(args.work / "candidate/textures" / f"{texture}.png", staged / relative)
        assert not any(t["trueName"] == route + "_body" for t in textures["textures"])
        native_sampler = sampler(exact)
        native_sampler["name"] = route + "_body"
        textures["textures"].append({"trueName": route + "_body", "path": relative,
                                     "sha256": sha(staged / relative), "sampler": native_sampler})
        if icon is not None:
            path = staged / f"icons/entities/npc/npcicon_{number}.png"
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(args.work / "icons" / f"npcicon_{icon}.png", path)
    write(staged / DOCUMENTS[2], textures)
    tables = read(staged / DOCUMENTS[1])
    locales = {lang: read(staged / f"localization/{lang}.json") for lang in ("en", "ru")}
    add_rows(native_npcs(tables), primary, locales)
    write(staged / DOCUMENTS[1], tables)
    for locale, value in locales.items():
        write(staged / f"localization/{locale}.json", value)
    outputs = []
    for path in sorted(staged.rglob("*")):
        if path.is_file():
            relative = path.relative_to(staged).as_posix()
            target = args.native_root / relative
            assert relative in DOCUMENTS or not target.exists(), f"existing payload: {relative}"
            outputs.append({"path": relative, "bytes": path.stat().st_size,
                            "sha256": sha(path), "before": sha(target) if target.exists() else None})
    server_outputs = []
    for directory in ("tdata", "bin/tdata"):
        relative = directory + "/xdt.json"
        target = args.server_root / relative
        server_tables = read(target)
        for key, rows in native_npcs(tables).items():
            if isinstance(rows, list):
                old = server_tables["m_pNpcTable"][key]
                assert rows[:len(old)] == old, f"server NPC data differs: {directory}/{key}"
                server_tables["m_pNpcTable"][key] = rows
        out = args.stage / "server" / relative
        write_server(out, server_tables, target)
        server_outputs.append({"path": relative, "before": sha(target), "sha256": sha(out)})
    report = {"schema": "ffone.omniverse-extension-stage.v1", "outputs": outputs,
              "serverOutputs": server_outputs, "applied": False,
              "placements": "owner-managed; no placement files read or written",
              "limitations": ["Albedo retains the existing native portrait: primary has no npcicon_116 route."]}
    write(args.stage / "plan.json", report)
    if args.apply:
        # Require an independently reproduced stage with identical accepted bytes.
        previous = read(args.work / "accepted-stage-plan.json")
        assert previous["outputs"] == outputs and previous["serverOutputs"] == server_outputs
        for root, entries, stage_root in ((args.native_root, outputs, staged),
                                         (args.server_root, server_outputs, args.stage / "server")):
            for entry in entries:
                target = root / entry["path"]
                assert (sha(target) if target.exists() else None) == entry["before"]
                backup = args.stage / "before" / ("native" if root == args.native_root else "server") / entry["path"]
                if target.exists():
                    backup.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(target, backup)
            for entry in entries:
                target = root / entry["path"]
                target.parent.mkdir(parents=True, exist_ok=True)
                temporary = target.with_name(target.name + ".omniverse-next")
                assert not temporary.exists()
                shutil.copyfile(stage_root / entry["path"], temporary)
                os.replace(temporary, target)
        report["applied"] = True
        write(args.stage / "plan.json", report)
    print(json.dumps({"nativeFiles": len(outputs), "npcTypesAdded": 5, "placementsAdded": 0,
                      "applied": report["applied"]}))


if __name__ == "__main__":
    main()
