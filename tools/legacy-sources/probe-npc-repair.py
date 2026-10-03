"""Capture selected installed NPCs with exact table texture overrides."""
import argparse
import importlib.util
import json
from pathlib import Path
import subprocess

def read(path):
    return json.loads(path.read_text(encoding="utf-8"))

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--assets", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--ids", nargs="+", type=int, required=True)
    parser.add_argument("--camera-view", default="primary")
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location("census", Path(__file__).with_name("asset-census.py"))
    census = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(census)
    table = next(t["value"]["m_pNpcTable"] for t in read(args.assets / "data/tables/table-set.json")["tables"] if "m_pNpcTable" in t["value"])
    routes = census.character_routes(read(args.assets / "_runtime/characters.json"))
    textures = {t["trueName"].lower(): t for t in read(args.assets / "data/tables/npc_texture_overrides.json")["textures"]}
    args.out.mkdir(parents=True, exist_ok=True)
    for number in args.ids:
        row = table["m_pNpcData"][number]
        mesh = table["m_pNpcMeshData"][row["m_iMesh"]]
        model = routes.get(mesh["m_pstrMMeshModelString"])
        if model is None:
            print(number, "unresolved model", flush=True)
            continue
        command = ["../FFOneClient/target/debug/examples/logical_model_gpu_preview.exe",
            "--asset-root", str(args.assets.resolve()), "--model", model["glb"],
            "--screenshot", str(args.out / f"{number}.png"), "--report", str(args.out / f"{number}.json"),
            "--frames", "300", "--timeout", "45", "--character-kind", "npc",
            "--true-root", model["logicalName"], "--npc-scale", str(row["m_fScale"])]
        command += ["--camera-view", args.camera_view]
        for field, flag in [("m_pstrMTextureString", "main"), ("m_pstrMTextureString2", "sub")]:
            texture = textures.get(mesh[field].lower())
            if texture:
                command += ["--" + flag + "-texture", texture["path"], "--" + flag + "-sampler", json.dumps(texture["sampler"])]
        if model["animations"]:
            command += ["--animation-name", "stand1" if "stand1" in model["animations"] else model["animations"][0]]
        result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", errors="replace")
        (args.out / f"{number}.log").write_text(result.stdout + result.stderr, encoding="utf-8")
        report = args.out / f"{number}.json"
        if report.exists():
            document = read(report)
            print(number, document.get("status"), document.get("error"), flush=True)
        else:
            print(number, "exit", result.returncode, (result.stdout + result.stderr)[:500], flush=True)

if __name__ == "__main__":
    main()
