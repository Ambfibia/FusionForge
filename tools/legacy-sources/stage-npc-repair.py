"""Stage the owner-requested NPC repairs; write only to an explicit Editor work tree."""
import argparse
import base64
import copy
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

import previous_build_retention

def read(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))

def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".next")
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    temporary.replace(path)

def put(path, payload):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".next")
    temporary.write_bytes(payload)
    temporary.replace(path)

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def sampler(exact, name):
    settings = exact["importSettings"]["m_TextureSettings"]
    wrap = "repeat" if settings["m_WrapMode"] == 0 else "clampToEdge"
    return dict(name=name, minFilter="nearest" if settings["m_FilterMode"] == 0 else "linear",
        magFilter="nearest" if settings["m_FilterMode"] == 0 else "linear", wrapS=wrap, wrapT=wrap,
        legacyFilterMode=settings["m_FilterMode"], legacyWrapMode=settings["m_WrapMode"],
        anisotropyLevel=settings["m_Aniso"], mipMapBias=settings["m_MipBias"])

SLOTS = [
    ("face", "Face", ["FaceTexture"], 3), ("hair", "Hair", ["HairTexture"], 4),
    ("shirt", "Shirt", ["ShirtTexture"], 2), ("pants", "Pants", ["PantsTexture"], 1),
    ("shoes", "Shoes", ["ShoesTexture"], 0), ("hat", "Hat", ["HatTextureM", "HatTextureS"], None),
    ("glasses", "Glass", ["GlassTextureM", "GlassTextureS"], None),
    ("back", "Back", ["BackTextureM", "BackTextureS"], None),
    ("rightWeapon", "RHandWpn", ["RHandWpnTextureM", "RHandWpnTextureS"], None),
]
RUSSIAN = {
    "Ranger Lindsey Lenses": "Рейнджер Линдси Линзес", "Upper Catacombs": "Верхние катакомбы",
    "Lower Catacombs": "Нижние катакомбы", "Magic Krab": "Мэджик Краб", "Numbuh Mach 2": "Номер Мах 2",
    "big dog": "Большой пёс",
    "My mom's an eye doctor so if your friends need any glasses, tell them to come by!": "Моя мама — глазной врач. Если твоим друзьям нужны очки, пусть заглядывают!",
    "Need stylin'? We're providin'!": "Нужен новый стиль? Мы поможем!",
    "We've been trained by the finest Barber in all of Stormalong!": "Нас обучал лучший парикмахер во всём Штормалонге!",
    "We've been trained by the finest barber in all of Stormalong!": "Нас обучал лучший парикмахер во всём Штормалонге!",
    "We're the apprentices of Dr. Barber, ever heard of him?": "Мы ученики доктора Парикмахера. Слышали о нём?",
    "It's nice to be out of the rain. You wouldn't believe the weather in Stormalong!": "Хорошо наконец укрыться от дождя. Вы бы знали, какая погода в Штормалонге!",
    "If you see the sweeper in Orchid Bay, you'll be able to change even more about yourself!": "Найди уборщика в Орхидейной бухте — он поможет ещё сильнее изменить твой облик!",
    "I'm just a temporary fill-in for Dr. Barber but I can do all the things he can!": "Я лишь временно заменяю доктора Парикмахера, но умею всё то же, что и он!",
    "Hey, you've got any tips on stories?": "Эй, есть наводки на интересные истории?",
    "Looking for a vehicle?": "Ищешь транспорт?", "Ribbit.": "Ква.",
}
for english, russian in [("Johnny", "Джонни"), ("Leon", "Леон"), ("Charlie", "Чарли"), ("Randal", "Рэндал"), ("Jay", "Джей")]:
    for suffix in ["", "2", "3"]:
        RUSSIAN["Sweeper " + english + suffix] = "Уборщик " + russian + (" " + suffix if suffix else "")

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--stage", type=Path, required=True)
    parser.add_argument("--lane", choices=["models", "textures", "tables", "hnpc", "icons", "icon-rows"], required=True)
    args = parser.parse_args()
    assert args.stage.resolve().is_relative_to(Path("work").resolve())
    work, stage = args.work, args.stage
    if args.lane == "models":
        for model in sorted((work / "recovery-a/candidate/models/npc").glob("*.glb")):
            evidence = work / "models-gpu/models/npc" / (model.stem + ".gpu.json")
            subprocess.run(["target/debug/fusionforge.exe", "native", "install-runtime-character-model", str(stage),
                "npc/" + model.stem, str(model), str(evidence)], check=True)
        return
    if args.lane == "textures":
        catalog = read(stage / "data/tables/npc_texture_overrides.json")
        entries = {t["trueName"]: t for t in catalog["textures"]}
        retained = previous_build_retention.retained_npc_textures()
        for recovery in [work / "recovery-b/textures", work / "extra-recovery/textures"]:
            for folder in sorted(recovery.iterdir()):
                if not (folder / "recovery.json").exists() or folder.name in retained:
                    continue
                exact = read(folder / "exact.json")
                name = folder.name
                path = f"textures/npc/{name}.png"
                put(stage / path, (folder / (name + ".png")).read_bytes())
                entries[name] = dict(trueName=name, path=path, sha256=sha(stage / path), sampler=sampler(exact, name))
        # Otto's authored publication embeds his exact texture; register that same payload.
        otto = stage / "characters/npcs/npc_otto/npc_otto.textures/npc_otto.png"
        if otto.exists():
            entries["npc_otto"] = dict(trueName="npc_otto", path=otto.relative_to(stage).as_posix(), sha256=sha(otto),
                sampler=dict(name="npc_otto", minFilter="linear", magFilter="linear", wrapS="repeat", wrapT="repeat",
                    legacyFilterMode=1, legacyWrapMode=0, anisotropyLevel=1, mipMapBias=0.0))
        catalog["textures"] = sorted(entries.values(), key=lambda t: t["trueName"])
        write(stage / "data/tables/npc_texture_overrides.json", catalog)
        return
    if args.lane == "icons":
        for folder in ["icons-exact", "retro-icons-exact", "icons-repaired-exact"]:
            for path in sorted((work / folder).glob("*.exact.json")):
                exact = read(path)
                name = path.name.removesuffix(".exact.json")
                prefix, number = name.rsplit("_", 1)
                family = {"npcicon": "npc", "mobicon": "mobs", "hnpcicon": "hnpc"}[prefix]
                payload = base64.b64decode(exact["payload"]["dataUrl"].split("base64,", 1)[1])
                put(stage / f"icons/entities/{family}/{prefix}_{int(number):02}.png", payload)
        return
    if args.lane == "icon-rows":
        doc = read(stage / "data/tables/table-set.json")
        native = next(x["value"]["m_pNpcTable"] for x in doc["tables"] if "m_pNpcTable" in x["value"])
        primary = read("work/legacy-sources/mobs-retrobution-20260821.xdt.object-evidence.json")["object"]["value"]["m_pNpcTable"]
        changes = []
        for number, row in enumerate(native["m_pNpcData"][:3430]):
            source = primary["m_pNpcData"][number]
            name = native["m_pNpcStringData"][row["m_iNpcName"]]["m_strName"]
            if number == 2226 or name in ["Ben Tennyson", "Gwen Tennyson", "Kevin Levin", "Kevin", "Albedo", "Grandpa Max"]:
                continue
            if name != primary["m_pNpcStringData"][source["m_iNpcName"]]["m_strName"]:
                continue
            icon = primary["m_pNpcIconData"][source["m_iIcon1"]]
            if icon == native["m_pNpcIconData"][row["m_iIcon1"]]:
                continue
            family, prefix = {4: ("npc", "npcicon"), 8: ("mobs", "mobicon"), 10: ("hnpc", "hnpcicon")}[icon["m_iIconType"]]
            assert (stage / f"icons/entities/{family}/{prefix}_{icon['m_iIconNumber']:02}.png").exists(), (number, icon)
            if icon not in native["m_pNpcIconData"]:
                native["m_pNpcIconData"].append(copy.deepcopy(icon))
            row["m_iIcon1"] = native["m_pNpcIconData"].index(icon)
            changes.append(dict(id=number, name=name, icon=icon))
        write(stage / "data/tables/table-set.json", doc)
        write(work / "icon-row-changes.json", changes)
        return
    if args.lane == "tables":
        doc = read(stage / "data/tables/table-set.json")
        native = next(x["value"]["m_pNpcTable"] for x in doc["tables"] if "m_pNpcTable" in x["value"])
        primary = read("work/legacy-sources/mobs-retrobution-20260821.xdt.object-evidence.json")["object"]["value"]["m_pNpcTable"]
        old = read("work/legacy-sources/retrobution-20260821-npc-review/old-xdt.json")["m_pNpcTable"]
        locales = {lang: read(stage / f"localization/{lang}.json") for lang in ["en", "ru"]}
        translations = {value: locales["ru"]["entries"][key] for key, value in locales["en"]["entries"].items() if key in locales["ru"]["entries"]}
        translations.update(RUSSIAN)
        def localize(key, en):
            assert en in translations or not en.strip(), (key, en)
            locales["en"]["entries"][key] = en
            locales["ru"]["entries"][key] = translations.get(en, en)
        changes = []
        def append(section, value):
            if value in native[section]:
                return native[section].index(value)
            native[section].append(copy.deepcopy(value))
            return len(native[section]) - 1
        # Isolate mesh-row changes from other NPCs that shared a previous mesh row.
        names = {"Dee Dee", "Dexter", "Courage", "Numbuh Two"}
        # Rows the owner kept at the previous build (e.g. Dee Dee 701) keep their mesh row.
        retained_rows = previous_build_retention.retained_npc_rows()
        for number, row in enumerate(native["m_pNpcData"][:3430]):
            name = native["m_pNpcStringData"][row["m_iNpcName"]]["m_strName"]
            if (name not in names and number not in [2226, 3036]) or row["m_iHNpc"] or number in retained_rows:
                continue
            source = old if number == 2226 else primary
            source_row = source["m_pNpcData"][number]
            mesh = source["m_pNpcMeshData"][source_row["m_iMesh"]]
            row["m_iMesh"] = append("m_pNpcMeshData", mesh)
            for key in ["m_fScale", "m_iHeight", "m_fAnimationSpeed", "m_fWalkAnimationSpeed", "m_fRunAnimationSpeed"]:
                row[key] = source_row[key]
            changes.append(number)
        assert len(native["m_pNpcData"]) == 3469, "expected preserved owner NPCs through 3468"
        mapping = []
        for original in range(3430, len(primary["m_pNpcData"])):
            row = copy.deepcopy(primary["m_pNpcData"][original])
            number = len(native["m_pNpcData"])
            for field, section in [("m_iNpcName", "m_pNpcStringData"), ("m_iComment", "m_pNpcStringData"),
                ("m_iMesh", "m_pNpcMeshData"), ("m_iIcon1", "m_pNpcIconData"),
                ("m_iBarkerNumber", "m_pNpcBarkerData"), ("m_iServiceNumber", "m_pNpcServiceData")]:
                source_index = row[field]
                row[field] = append(section, primary[section][source_index])
                if section in ["m_pNpcStringData", "m_pNpcBarkerData"]:
                    domain = "npc_string" if section == "m_pNpcStringData" else "npc_barker"
                    for key, value in primary[section][source_index].items():
                        if key.startswith("m_str") and value and (key != "m_strComment2" or domain == "npc_barker"):
                            slug = re.sub(r"([a-z])([A-Z])", r"\1_\2", key[2:]).lower()
                            localize(f"content.tabledata.npc.{domain}.{row[field]}.{slug}", value)
            row["m_iNpcNumber"] = number
            native["m_pNpcData"].append(row)
            name = native["m_pNpcStringData"][row["m_iNpcName"]]["m_strName"]
            localize(f"content.npc.{number}.name", name)
            mapping.append(dict(sourceId=original, nativeId=number, name=name))
        assert locales["en"]["entries"].keys() == locales["ru"]["entries"].keys()
        write(stage / "data/tables/table-set.json", doc)
        for lang, locale in locales.items():
            write(stage / f"localization/{lang}.json", locale)
        write(work / "table-changes.json", dict(updated=changes, added=mapping))
        return
    if args.lane == "hnpc":
        native = read(stage / "data/hnpc/catalog.json")
        primary = read(work / "hnpc.evidence.json")["object"]["value"]["TableElement"]
        item_catalog = read(stage / "characters/player/items/catalog.json")["models"]
        source_models = read(work / "equipment-candidate/equipment-logical-model-batch-report.json")["models"]
        aliases = {m["exactRoute"]: m["trueName"] for m in source_models}
        certified = {(gender, model["exactRoute"]): model for item in read(stage / "data/character_creation/avatar_items.json")["items"]
            for gender in ["male", "female"] for model in item.get(gender, {}).get("models", [])}
        parts = {p["exactRoute"]: p for a in native["appearances"] for p in a["parts"]}
        textures = {}
        for texture in native["textures"]:
            source = texture["source"]
            wrap = "clampToEdge" if source["wrapMode"] == 1 else "repeat"
            textures[texture["trueName"].lower()] = dict(trueName=texture["trueName"], path=texture["path"],
                bytes=texture["bytes"], sha256=texture["sha256"], width=source["width"], height=source["height"],
                sampler=dict(name=texture["trueName"], minFilter="nearest" if source["filterMode"] == 0 else "linear",
                    magFilter="nearest" if source["filterMode"] == 0 else "linear", wrapS=wrap, wrapT=wrap,
                    legacyFilterMode=source["filterMode"], legacyWrapMode=source["wrapMode"],
                    anisotropyLevel=source["anisotropyLevel"], mipMapBias=0.0))
        runtime_textures = {t["trueName"].lower(): t for t in read(stage / "data/character_creation/runtime_textures.json")["textures"]}
        unresolved = []
        for index, row in enumerate(primary[185:], 185):
            appearance = dict(index=index, legacyType=row["iType"], gender="female" if row["iType"] % 2 == 0 else "male",
                hairColor=row["iHairColor"], height=row["iHeight"], shape=row["iShape"], skinColor=row["iSkinColor"], parts=[])
            for kind, slot, texture_fields, clothes in SLOTS:
                mesh = row["str" + slot + "Mesh"]
                if mesh in ["", "null", "0"]:
                    continue
                route = "wear/" + mesh.lower()
                if route in parts:
                    part = copy.deepcopy(parts[route])
                else:
                    matches = [m for m in item_catalog if m["trueName"] == aliases.get(route, mesh[:-4])]
                    assert len(matches) == 1, (route, len(matches))
                    model = matches[0]
                    part = dict(kind=kind, exactRoute=route, sourceRoute=model["sourceRoute"], resourceSet=model["resourceSet"],
                        trueName=model["trueName"], nativeAsset=model["model"])
                    if clothes is not None:
                        part["actorSkinCombinerClothesIndex"] = clothes
                existing = certified.get((appearance["gender"], route))
                if existing and (existing["trueName"] != part["trueName"] or existing["nativeAsset"] != part["nativeAsset"]):
                    # Native route isolation preserves both independently certified models.
                    part["exactRoute"] = "wear/hnpc_" + part["trueName"] + ".nif"
                    unresolved.append(dict(appearance=index, route=route, nativeRoute=part["exactRoute"], resolution="distinct-native-model-owner"))
                part["textures"] = []
                for field in texture_fields:
                    name = row["str" + field]
                    if name in ["", "null", "0"]:
                        continue
                    name = name.removesuffix(".dds")
                    key = name.lower()
                    if key not in textures:
                        if key in runtime_textures:
                            tex = runtime_textures[key]
                            textures[key] = dict(trueName=name, path=tex["nativeAsset"]["path"], bytes=tex["nativeAsset"]["bytes"],
                                sha256=tex["nativePngSha256"], width=tex["source"]["width"], height=tex["source"]["height"], sampler=tex["sampler"])
                        else:
                            folder = work / "hnpc-recovery/textures" / name
                            if not (folder / "exact.json").exists():
                                assert index == 203 and kind == "hair" and name == "m_head_002", (index, kind, name)
                                # The clean resource lookup is null. ActorSkinCombiner still
                                # assigns it, so preserve the native shader-default state.
                                unresolved.append(dict(appearance=index, texture=name, resolution="source-null-texture-shader-default"))
                                continue
                            exact = read(folder / "exact.json")
                            path = f"textures/hnpc/{name}.png"
                            put(stage / path, (folder / (name + ".png")).read_bytes())
                            textures[key] = dict(trueName=name, path=path, bytes=(stage / path).stat().st_size, sha256=sha(stage / path),
                                width=exact["width"], height=exact["height"], sampler=sampler(exact, name))
                    part["textures"].append(name)
                appearance["parts"].append(part)
            native["appearances"].append(appearance)
        native["schema"] = "ffone.hnpc-runtime-catalog.v2"
        native.pop("provenance")
        native["textures"] = sorted(textures.values(), key=lambda t: t["trueName"])
        write(stage / "data/hnpc/catalog.json", native)
        write(work / "hnpc-changes.json", dict(appearances=len(native["appearances"]), textures=len(textures), sourceDefects=unresolved))

if __name__ == "__main__":
    main()
