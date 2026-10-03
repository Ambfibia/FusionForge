"""Real raw-container replay with filesystem snapshots; outputs JSON to stdout."""
from pathlib import Path
import hashlib, json, os, subprocess, sys
forge=Path(__file__).resolve().parents[1]
base=forge.parent
source=base/"builds/retrobution-20260821"
client=base/"FFOneClient"
exe=forge/"target/debug/fusionforge.exe"

def snapshot(root):
    result={}
    for directory,dirs,files in os.walk(root):
        dirs[:]=[d for d in dirs if d not in {"target",".git","node_modules","__pycache__","vendor"}]
        for name in files:
            path=Path(directory)/name
            stat=path.stat()
            result[path.relative_to(root).as_posix()]=(stat.st_size,stat.st_mtime_ns)
    return result

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()

def command(args,success=True):
    output=subprocess.run([str(exe),*map(str,args)],cwd=forge,capture_output=True,text=True)
    if (output.returncode==0)!=success:raise RuntimeError(output.stdout+output.stderr)
    return {"arguments":list(map(str,args)),"exitCode":output.returncode,"stdout":output.stdout.strip(),"stderr":output.stderr.strip()}

roots={"raw":source,"forge":forge,"native":client/"assets/game"}
before={name:snapshot(root) for name,root in roots.items()}
raw_hashes={name:digest(source/name) for name in ["main.unity3d","FutureNano.resourceFile"]}
outputs=[client/"assets/game/characters/nano/nano_eddy.glb",client/"assets/game/ui/en/gameplay/quit-menu/menu.ffquit.json"]
commands=[]
for _ in range(2):
    commands.append(command(["convert-native-model",source/"FutureNano.resourceFile","nano/nano_eddy.kfm","nano",client/"assets/game/characters"]))
    commands.append(command(["convert-native-ui",source,"quit-menu",client/"assets/game"]))
commands.append(command(["convert-native-ui",source,"unknown-panel",client/"assets/game"],False))
commands.append(command(["convert-native-ui",source,"quit-menu",source],False))
after={name:snapshot(root) for name,root in roots.items()}
changes={name:sorted(path for path in set(before[name])|set(after[name]) if before[name].get(path)!=after[name].get(path)) for name in roots}
assert not any(changes.values()),changes
assert raw_hashes=={name:digest(source/name) for name in raw_hashes}
assert not (forge/"work").exists()
assert not (client/"assets/game/characters/nano/nano_eddy.publish.json").exists()
print(json.dumps({"status":"passed","scope":"Two replays into existing final outputs; all native assets, raw build and supported Forge tree size/mtime snapshots, excluding Cargo/vendor/git. Not a system-wide syscall trace.","changes":changes,"rawSha256":raw_hashes,"outputSha256":{str(p.relative_to(client)):digest(p) for p in outputs},"commands":commands},indent=2))
