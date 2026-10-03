"""Read-only focused NPC resolution and primary-version delta audit."""
import json
import importlib.util
from pathlib import Path

def read(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))

def main():
    spec = importlib.util.spec_from_file_location("census", Path(__file__).with_name("asset-census.py"))
    census = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(census)
    assets = Path("../FFOneClient/assets/game")
    native = next(x["value"]["m_pNpcTable"] for x in read(assets / "data/tables/table-set.json")["tables"] if "m_pNpcTable" in x["value"])
    primary = read("work/legacy-sources/mobs-retrobution-20260821.xdt.object-evidence.json")["object"]["value"]["m_pNpcTable"]
    old = read("work/legacy-sources/retrobution-20260821-npc-review/old-xdt.json")["m_pNpcTable"]
    routes = census.character_routes(read(assets / "_runtime/characters.json"))
    textures = {x["trueName"]: x for x in read(assets / "data/tables/npc_texture_overrides.json")["textures"]}
    terms = ["ice king", "four door", "blossom door", "m-199", "infiltration", "runty", "guide changer", "spidermonkey", "chupacabra", "hippie hop", "exterminator tent", "fusion rex", "fusion johnny", "paradox", "azmuth", "fusion kimchi", "fusion finn", "otto", "chowder", "dee dee", "dexter", "courage", "numbuh two"]
    result = []
    for row in native["m_pNpcData"]:
        name = native["m_pNpcStringData"][row["m_iNpcName"]]["m_strName"]
        number = row["m_iNpcNumber"]
        if number != 2226 and not any(q in name.lower() for q in terms):
            continue
        mesh = native["m_pNpcMeshData"][row["m_iMesh"]]
        model = routes.get(mesh["m_pstrMMeshModelString"])
        prior = primary["m_pNpcData"][number] if number < len(primary["m_pNpcData"]) else None
        result.append(dict(id=number, name=name, row=row, mesh=mesh, model=model,
            textures={mesh[k]: textures.get(mesh[k]) for k in ("m_pstrMTextureString", "m_pstrMTextureString2")},
            icon=native["m_pNpcIconData"][row["m_iIcon1"]],
            primaryRow=prior, primaryMesh=primary["m_pNpcMeshData"][prior["m_iMesh"]] if prior else None))
    out = Path("work/legacy-sources/npc-repair-20260905/audit.json")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    seen = set()
    for x in result:
        key = (x["name"], json.dumps(x["mesh"]))
        if key in seen:
            continue
        seen.add(key)
        print(x["id"], x["name"], "HNPC", x["row"].get("m_iHNpc"), "model", x["mesh"]["m_pstrMMeshModelString"], "resolved", x["model"]["id"] if x["model"] else None, "textures", [(k, v is not None) for k, v in x["textures"].items()], "icon", x["icon"])
        print(" primary", x["primaryMesh"])
    print("COUNTS", {"old": len(old["m_pNpcData"]), "primary": len(primary["m_pNpcData"]), "native": len(native["m_pNpcData"])})
    for i, row in enumerate(primary["m_pNpcData"]):
        name = primary["m_pNpcStringData"][row["m_iNpcName"]]["m_strName"]
        old_name = old["m_pNpcStringData"][old["m_pNpcData"][i]["m_iNpcName"]]["m_strName"] if i < len(old["m_pNpcData"]) else None
        if name != old_name:
            print("NAME DELTA", i, repr(old_name), "=>", repr(name))

if __name__ == "__main__":
    main()
