"""Audit the complete native inventory icon contract, not source-name counts."""
import argparse
import json
from pathlib import Path
from PIL import Image

TABLES = ["Weapon", "Shirts", "Pants", "Shoes", "Hat", "Glass", "Back", "General", "Chest", "Vehicle"]
ROUTES = {0: ("items/weapons", "wpnicon"), 3: ("items/cosmetics", "cosicon"),
          7: ("items/general", "generalitemicon"), 8: ("entities/mobs", "mobicon"),
          12: ("items/vehicles", "vehicle")}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--target-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    document = json.loads((args.target_root / "data/tables/table-set.json").read_text(encoding="utf-8"))
    tables = next(table["value"] for table in document["tables"] if table["name"] == "npc_imports_consolidated")
    checked = {}; rows = 0; missing = []; invalid = []
    for name in TABLES:
        key = f"m_p{name}ItemTable"; table = tables[key]
        for item_id, item in enumerate(table["m_pItemData"][1:], 1):
            rows += 1
            icon = table["m_pItemIconData"][item["m_iIcon"]]
            folder, prefix = ROUTES[icon["m_iIconType"]]
            path = f"icons/{folder}/{prefix}_{icon['m_iIconNumber']:02}.png"
            if path not in checked:
                file = args.target_root / path
                checked[path] = file.is_file()
                if checked[path]:
                    try:
                        with Image.open(file) as image:
                            image.verify()
                    except Exception as error:
                        invalid.append({"path": path, "error": str(error)})
            if not checked[path]:
                missing.append({"table": key, "itemRow": item_id, "path": path})
    report = {"schema": "fusionforge.native-item-icon-audit.v1", "itemRows": rows,
              "uniqueIconPaths": len(checked), "missing": missing, "invalid": invalid,
              "excluded": "Empty row zero, character face/head styles and legacy-null quest icons are not inventory item icons."}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"{rows} items, {len(checked)} icons, {len(missing)} missing references, {len(invalid)} invalid PNGs")
    if missing or invalid:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
