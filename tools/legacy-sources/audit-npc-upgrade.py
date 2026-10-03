"""Read-only primary NPC delta; output is evidence/candidates, never a publication plan."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path

spec = importlib.util.spec_from_file_location("asset_census", Path(__file__).with_name("asset-census.py"))
census = importlib.util.module_from_spec(spec)
spec.loader.exec_module(census)

PROTECTED_MODELS = {"npc_bentennyson", "npc_gwen1", "npc_kevinlevin", "npc_albedo", "npc_max"}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def projection(table, row_index):
    rows = table["m_pNpcData"]
    if row_index >= len(rows):
        return None
    row = rows[row_index]
    name = table["m_pNpcStringData"][row["m_iNpcName"]]["m_strName"]
    mesh = table["m_pNpcMeshData"][row["m_iMesh"]]
    return {"rowIndex": row_index, "name": name, "hnpc": row.get("m_iHNpc", 0),
            "model": mesh["m_pstrMMeshModelString"], "mainTexture": mesh["m_pstrMTextureString"],
            "subTexture": mesh["m_pstrMTextureString2"], "scale": row["m_fScale"],
            "iconRow": row["m_iIcon1"], "height": row["m_iHeight"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--native-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    evidence = json.loads(args.evidence.read_text("utf-8"))
    source, obj = evidence["source"], evidence["object"]
    assert source["alias"] == "primary"
    assert obj["type"] == "MonoBehaviour" and obj["name"] == "xdtdatas"
    assert evidence["triage"]["unresolvedPointerCount"] == 0
    raw = (args.source_root / source["relativeContainer"]).resolve()
    assert raw.is_relative_to(args.source_root.resolve())
    assert raw.stat().st_size == source["bytes"] and sha(raw) == source["sha256"].lower()
    primary = obj["value"]["m_pNpcTable"]
    table_path = args.native_root / "data/tables/table-set.json"
    native_tables = json.loads(table_path.read_text("utf-8"))["tables"]
    choices = [t["value"]["m_pNpcTable"] for t in native_tables if "m_pNpcTable" in t["value"]]
    assert len(choices) == 1, "NPC table must have one native owner"
    native = choices[0]
    registry_path = args.native_root / "_runtime/characters.json"
    registry = json.loads(registry_path.read_text("utf-8"))
    routes = census.character_routes(registry)
    changes = []
    for index, row in enumerate(primary["m_pNpcData"]):
        if not index or row["m_iTeam"] == 2 or row["m_iHNpc"] or row["m_iNpcType"] >= 100:
            continue
        original, installed = projection(primary, index), projection(native, index)
        if original == installed:
            continue
        candidate = routes.get(original["model"])
        changes.append({"primary": original, "native": installed,
                        "policy": "separate-variant-only" if original["model"] in PROTECTED_MODELS else "review",
                        "installedModelCandidate": candidate["id"] if candidate else None,
                        "status": "requires-scoped-model-texture-icon-and-runtime-verification"})
    result = {"schema": "ffone.npc-upgrade-audit.v1", "publicationAllowed": False,
              "source": source, "object": {k: obj[k] for k in ("serializedAsset", "type", "pathId", "rawSha256")},
              "inputs": {"evidenceSha256": sha(args.evidence), "nativeTableSha256": sha(table_path),
                         "registrySha256": sha(registry_path)},
              "counts": {"changedFriendlyNonHumanRows": len(changes),
                         "protectedVariantRows": sum(c["policy"] == "separate-variant-only" for c in changes)},
              "notes": ["Row/model names are candidates; installed model identity does not prove matching payloads.",
                        "Existing NPC rows and placements must be preserved for protected variants.",
                        "HNPC appearance changes require the separate player-rig appearance consumer."],
              "changes": changes}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", "utf-8")
    print(json.dumps(result["counts"]))


if __name__ == "__main__":
    main()
