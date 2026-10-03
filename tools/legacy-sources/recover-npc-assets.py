"""Recover explicitly scoped NPC model/texture candidates into Editor staging."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import subprocess

def read(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--lane", choices=["models", "textures", "icons"], required=True)
    args = parser.parse_args()
    plan = read(args.plan)
    forge = str(Path("target/debug/fusionforge.exe").resolve())
    pipeline = str(Path("target/debug/fusionforge.exe").resolve())
    args.work.mkdir(parents=True, exist_ok=True)
    report = []
    routes_cache = {}
    for entry in plan[args.lane]:
        source = plan["sources"][entry["source"]]
        raw = Path(source["root"]) / entry["container"]
        identity = dict(alias=entry["source"], role=source["role"], relativeContainer=entry["container"], bytes=raw.stat().st_size, sha256=sha(raw))
        name = entry["name"]
        folder = args.work / args.lane / name
        folder.mkdir(parents=True, exist_ok=True)
        def run(command, log):
            result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", errors="replace")
            (folder / log).write_text(result.stdout + result.stderr, encoding="utf-8")
            if result.returncode:
                raise RuntimeError(result.stderr[-2500:] or result.stdout[-2500:])
            return result.stdout
        try:
            if args.lane == "models":
                output = folder / (name + ".source.json")
                run([forge, "fusionforge", "export-logical-model-source", str(raw), entry["route"], str(output), source["project"]], "export.log")
                run([pipeline, "native", "publish-logical-model", str(output), "npc", str(args.work / "candidate")], "publish.log")
                document = read(output)
                evidence = dict(source=identity, route=entry["route"], targets=document["exactContainerTargets"], sourceSha256=sha(output))
            else:
                key = str(raw)
                if key not in routes_cache:
                    rows = run([forge, "fusionforge", "list-assetbundle", str(raw)], "routes.tsv").splitlines()
                    routes_cache[key] = [line.split("\t") for line in rows if len(line.split("\t")) == 4]
                matches = [row for row in routes_cache[key] if row[3].lower() == entry["route"].lower()]
                assert len(matches) == 1, (entry, matches)
                path_id = int(matches[0][0])
                exact_path = folder / "exact.json"
                run([forge, "fusionforge", "export-exact-texture", str(raw), str(path_id), str(exact_path)], "export.log")
                exact = read(exact_path)
                assert exact["id"].endswith(":" + str(path_id))
                run([forge, "fusionforge", "dump-object-evidence", entry["source"], source["root"], entry["container"], str(path_id), "--serialized-asset", exact["id"].rsplit(":", 1)[0], "--type", "Texture2D", "--out", str(folder / "object.evidence.json")], "evidence.log")
                png = base64.b64decode(exact["payload"]["dataUrl"].split("base64,", 1)[1])
                (folder / (name + ".png")).write_bytes(png)
                evidence = dict(source=identity, route=entry["route"], serializedAsset=exact["id"].rsplit(":", 1)[0], type="Texture2D", pathId=path_id, exactSha256=sha(exact_path), pngSha256=sha(folder / (name + ".png")))
            write(folder / "recovery.json", evidence)
            report.append(dict(name=name, status="recovered", **evidence))
            print("recovered", args.lane, name, flush=True)
        except Exception as error:
            report.append(dict(name=name, status="blocked", error=str(error), source=identity))
            print("blocked", args.lane, name, str(error)[-1200:], flush=True)
        write(args.work / (args.lane + ".report.json"), report)

if __name__ == "__main__":
    main()
