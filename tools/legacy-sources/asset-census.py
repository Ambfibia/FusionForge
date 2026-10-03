"""Definitive asset census for FFOneClient against the clean primary build.

Resolution rules follow the runtime, not filename similarity:

* characters  -- an XDT `m_pNpcMeshData.m_pstrMMeshModelString` resolves against
                 `_runtime/characters.json` `logicalName` OR any `legacyAliases`
                 entry (`network_world_runtime::by_logical_name`). Rows whose NPC
                 is an HNPC (`m_iHNpc != 0`) name a pose, not a model.
* equipment   -- read from the generated `data/character_creation/avatar_items.json`,
                 which is what actually resolves an equipped item: it records a
                 `modelStatus` per gender slot plus the `exactRoute` (`wear/*.nif`)
                 of every resolved model. Do NOT compare XDT mesh strings against
                 the player-item catalog's `trueName`: one NIF can hold several
                 named parts, so 150 published models carry a `trueName` that is
                 no NIF file stem at all, and that comparison invents hundreds of
                 false gaps. A `missing` slot with no `sourceModelTrueName` means
                 the table names no model for that gender -- a boys-only shoe has
                 no female model -- which is correct, not debt.
* tiles       -- `Map_XX_YY.unity3d` against `map/catalog.json` tile grid.

For every unresolved name the census records whether the clean primary build
actually ships a model for it, so an XDT row for content the client never had is
never reported as publishable work.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path, PurePosixPath

EDITOR = Path(__file__).resolve().parents[2]
FFONE = EDITOR.parent / "FFOneClient"
BUILDS = EDITOR.parent / "builds"
PRIMARY_BUILD = "retrobution-20260821"
PRIMARY_DIR = BUILDS / PRIMARY_BUILD
PRIMARY_INDEX = (
    EDITOR
    / "work/projects/retrobution-ui-20260821.ffclient/cache/bundle-index.json"
)
DONOR_INDEX = BUILDS / "6543a2bb-d154-4087-b9ee-3c8aa778580a.ffclient/cache/bundle-index.json"

ITEM_TABLES = [
    "m_pWeaponItemTable",
    "m_pShirtsItemTable",
    "m_pPantsItemTable",
    "m_pShoesItemTable",
    "m_pHatItemTable",
    "m_pGlassItemTable",
    "m_pBackItemTable",
    "m_pHeadItemTable",
    "m_pFaceItemTable",
    "m_pVehicleItemTable",
]


def routes(index_path: Path, prefix: str, suffix: str) -> dict[str, list[tuple[str, str]]]:
    data = json.loads(index_path.read_text("utf-8"))
    out: dict[str, list[tuple[str, str]]] = {}
    for bundle in data["bundles"]:
        for asset in bundle.get("assets", []):
            for route in asset.get("containerPaths") or []:
                if route.startswith(prefix) and route.endswith(suffix):
                    out.setdefault(route[len(prefix) : -len(suffix)].lower(), []).append(
                        (bundle["name"], route)
                    )
    return out


# A container route existing is not proof that a publishable payload exists.
# Each entry here was proved empty by actually running the exporter; the value is
# the evidence that settled it.
PROVEN_EMPTY_ROUTES = {
    "rxcom": (
        "fusionforge export-logical-model-source rejects mob/rxcom.kfm with "
        "'contains no publishable mesh'; its NPC is the invisible Recall Point marker"
    ),
}


def classify(name: str, primary, donor) -> str:
    key = name.lower()
    if key in PROVEN_EMPTY_ROUTES:
        return "no-publishable-payload"
    if key in primary:
        return "primary"
    if key in donor:
        return "donor-only"
    return "absent"


def character_routes(registry: dict) -> dict[str, dict | None]:
    """Mirror parse_character_registry/select_registry_model, including case.

    An installed alternate package may own only its aliases. Its logical root
    is not permission to replace the canonical NPC. Multiple categories select
    a unique NPC candidate; contradictory entries in one category are invalid.
    """
    candidates: dict[str, list[dict]] = {}
    for model in registry["models"]:
        category = model["category"]
        if category in ("nano", "player", "tutorial"):
            continue
        logical = model["logicalName"]
        aliases = model.get("legacyAliases", [])
        glb = PurePosixPath(model["glb"])
        if glb.stem != logical:
            raise ValueError(f"GLB root does not match logicalName: {model['id']}")
        names = ([logical] if glb.parent.name == logical or not aliases else []) + aliases
        for name in names:
            group = candidates.setdefault(name, [])
            if any(m["category"] == category and m["glb"] != model["glb"] for m in group):
                raise ValueError(f"Contradictory registry route: {name}/{category}")
            if not any(m["category"] == category and m["glb"] == model["glb"] for m in group):
                group.append(model)
    resolved = {}
    for name, group in candidates.items():
        preferred = group if len(group) == 1 else [m for m in group if m["category"] == "npc"]
        resolved[name] = preferred[0] if len(preferred) == 1 else None
    return resolved


def census_characters(tables, primary, donor, registry=None) -> dict:
    npc_table = tables["m_pNpcTable"]
    mesh_rows = npc_table["m_pNpcMeshData"]
    npc_rows = npc_table["m_pNpcData"]
    name_rows = npc_table["m_pNpcStringData"]

    if registry is None:
        registry = json.loads((FFONE / "assets/game/_runtime/characters.json").read_text("utf-8"))
    resolution = character_routes(registry)
    resolvable = {name for name, model in resolution.items() if model is not None}

    referenced: dict[str, dict] = {}
    for npc_id, row in enumerate(npc_rows):
        if npc_id == 0 or row.get("m_iNpcType", 0) >= 100:
            continue
        index = row.get("m_iMesh", 0)
        if not (0 <= index < len(mesh_rows)):
            continue
        name = mesh_rows[index].get("m_pstrMMeshModelString") or ""
        if not name or name.lower() == "null":
            continue
        entry = referenced.setdefault(name, {"name": name, "npcs": [], "hnpc": 0})
        if row.get("m_iHNpc", 0):
            entry["hnpc"] += 1
        else:
            entry["npcs"].append(npc_id)

    model_refs = {k: v for k, v in referenced.items() if v["npcs"]}
    unresolved = {k: v for k, v in model_refs.items() if k not in resolvable}

    def npc_name(npc_id: int) -> str:
        index = npc_rows[npc_id].get("m_iNpcName", 0)
        return (name_rows[index].get("m_strName") or "").strip() if 0 <= index < len(name_rows) else ""

    rows = []
    for key, entry in sorted(unresolved.items()):
        origin = classify(key, primary, donor)
        rows.append(
            {
                "name": entry["name"],
                "npcCount": len(entry["npcs"]),
                "sampleNpc": npc_name(entry["npcs"][0]),
                "origin": origin,
                "bundle": primary.get(key.lower(), [("", "")])[0][0] if origin == "primary" else "",
                "resolution": "ambiguous" if key in resolution else "missing-exact-route",
            }
        )
    return {
        "registryEntries": len(registry["models"]),
        "resolvableNames": len(resolvable),
        "xdtModelReferences": len(model_refs),
        "hnpcPoseNames": len(referenced) - len(model_refs),
        "unresolved": rows,
    }


def census_equipment(primary, donor) -> dict:
    items = json.loads(
        (FFONE / "assets/game/data/character_creation/avatar_items.json").read_text("utf-8")
    )
    catalog = json.loads(
        (FFONE / "assets/game/characters/player/items/catalog.json").read_text("utf-8")
    )

    status: dict[str, int] = {}
    no_model_slots = 0
    unresolved: dict[str, dict] = {}
    for item in items["items"]:
        for gender in ("male", "female"):
            slot = item.get(gender)
            if not isinstance(slot, dict) or "modelStatus" not in slot:
                continue
            state = slot["modelStatus"]
            status[state] = status.get(state, 0) + 1
            if state != "missing":
                continue
            source = slot.get("sourceModelTrueName")
            if not source:
                no_model_slots += 1
                continue
            entry = unresolved.setdefault(
                source.lower(), {"name": source, "slots": 0, "items": []}
            )
            entry["slots"] += 1
            if len(entry["items"]) < 4:
                entry["items"].append(
                    {
                        "category": item["category"],
                        "itemNumber": item["itemNumber"],
                        "name": item.get("name"),
                        "gender": gender,
                    }
                )

    rows = []
    for key, entry in sorted(unresolved.items()):
        origin = classify(key, primary, donor)
        rows.append(
            {
                "name": entry["name"],
                "slots": entry["slots"],
                "items": entry["items"],
                "origin": origin,
                "bundle": primary.get(key, [("", "")])[0][0] if origin == "primary" else "",
            }
        )
    return {
        "modelReferences": items["counts"]["modelReferences"],
        "resolvedModels": items["counts"]["resolvedModels"],
        "modelStatus": status,
        "slotsWithNoModelInTable": no_model_slots,
        "catalogModels": len(catalog["models"]),
        "unresolved": rows,
    }


def census_tiles() -> dict:
    """A `Map_XX_YY.unity3d` is only a real tile when its `DongResources_XX_YY`
    ships that tile's own terrain.

    Three grid positions have a `Map_*` bundle whose `DongResources_*` is an
    8 KB stub holding nothing but tile 00_09's `TerrainData`, lightmap, splat
    and sound. They are placeholders, not world content, and publishing them
    would duplicate tile 00_09 at three unrelated coordinates.
    """
    catalog = json.loads((FFONE / "assets/game/map/catalog.json").read_text("utf-8"))
    published = {t["tileId"].split("_", 1)[1] for t in catalog["tiles"]}
    source = {p.stem.split("_", 1)[1] for p in PRIMARY_DIR.glob("Map_*.unity3d")}

    index = json.loads(PRIMARY_INDEX.read_text("utf-8"))
    dong_routes: dict[str, set[str]] = {}
    for bundle in index["bundles"]:
        name = bundle["name"]
        if not name.startswith("DongResources_") or not name.endswith(".resourceFile"):
            continue
        grid = name[len("DongResources_") : -len(".resourceFile")]
        routes = dong_routes.setdefault(grid, set())
        for asset in bundle.get("assets", []):
            routes.update(asset.get("containerPaths") or [])

    placeholders = []
    for grid in sorted(source - published):
        routes = dong_routes.get(grid, set())
        own = f"generateddongs/terraindata_{grid}.asset"
        if own in routes:
            continue
        borrowed = sorted(
            route[len("generateddongs/terraindata_") : -len(".asset")]
            for route in routes
            if route.startswith("generateddongs/terraindata_")
            and route.endswith(".asset")
            and not route.endswith("_lm.asset")
        )
        placeholders.append({"tile": grid, "terrainBorrowedFrom": borrowed})

    placeholder_tiles = {entry["tile"] for entry in placeholders}
    return {
        "sourceTiles": len(source),
        "publishedTiles": len(published),
        "unpublished": sorted(source - published),
        "placeholderTilesWithoutOwnTerrain": placeholders,
        "publishable": sorted(source - published - placeholder_tiles),
        "publishedWithoutSource": sorted(published - source),
    }


def main() -> int:
    tables = json.loads((FFONE / "assets/game/data/tables/table-set.json").read_text("utf-8"))
    tables = tables["tables"][0]["value"]

    kfm_primary = routes(PRIMARY_INDEX, "mob/", ".kfm")
    kfm_donor = routes(DONOR_INDEX, "mob/", ".kfm")
    nif_primary = routes(PRIMARY_INDEX, "wear/", ".nif")
    nif_donor = routes(DONOR_INDEX, "wear/", ".nif")

    report = {
        "schema": "ffone.asset-census.v1",
        "primaryBuild": PRIMARY_BUILD,
        "characters": census_characters(tables, kfm_primary, kfm_donor),
        "equipment": census_equipment(nif_primary, nif_donor),
        "tiles": census_tiles(),
        "sourceModelCounts": {
            "primaryMobKfm": len(kfm_primary),
            "primaryWearNif": len(nif_primary),
        },
    }

    for domain in ("characters", "equipment"):
        section = report[domain]
        rows = section["unresolved"]
        by_origin = {
            origin: [r for r in rows if r["origin"] == origin]
            for origin in ("primary", "donor-only", "absent", "no-publishable-payload")
        }
        print(f"== {domain} ==")
        for key, value in section.items():
            if key != "unresolved":
                print(f"  {key:24}: {value}")
        print(f"  {'unresolved total':24}: {len(rows)}")
        for origin, items in by_origin.items():
            print(f"    {origin:22}: {len(items)}")
        print("  publishable from primary:")
        count_key = "npcCount" if domain == "characters" else "slots"
        for row in sorted(by_origin["primary"], key=lambda r: -r[count_key]):
            print(f"    {row['name'][:38]:38} uses={row[count_key]:3d}  {row['bundle']}")
        print()

    tiles = report["tiles"]
    print("== tiles ==")
    for key, value in tiles.items():
        print(f"  {key:24}: {value}")

    if len(sys.argv) > 1:
        Path(sys.argv[1]).write_text(json.dumps(report, indent=1, ensure_ascii=False), "utf-8")
        print(f"\nwrote {sys.argv[1]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
