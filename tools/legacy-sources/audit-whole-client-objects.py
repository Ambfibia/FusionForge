"""Inventory ALL checked primary containers using FusionForge's raw object reader.
Scoped object identity is preserved from asset headers or a unique indexed asset.
This is object discovery, not pointer-graph or runtime equivalence proof.
"""
import collections, concurrent.futures, csv, hashlib, json, subprocess, time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
OUT=ROOT/'work/legacy-sources/whole-client-audit-20260905'
INDEX=json.loads((ROOT/'work/projects/retrobution-ui-20260821.ffclient/cache/bundle-index.json').read_text(encoding='utf-8'))
SOURCE=ROOT.parent/'builds/retrobution-20260821'
EXE=ROOT/'target/debug/fusionforge.exe'
LIST=OUT/'raw-object-listings'; LIST.mkdir(parents=True,exist_ok=True)
def one(bundle):
    dst=LIST/(bundle['name']+'.tsv');err=LIST/(bundle['name']+'.stderr.txt')
    start=time.monotonic()
    with dst.open('wb') as f,err.open('wb') as e:
        result=subprocess.run([str(EXE),'fusionforge','list-contents',str(SOURCE/bundle['name'])],cwd=ROOT,stdout=f,stderr=e,timeout=180)
    return {'container':bundle['name'],'exitCode':result.returncode,'seconds':round(time.monotonic()-start,3),
            'listingSha256':hashlib.sha256(dst.read_bytes()).hexdigest(),'bytes':dst.stat().st_size}
results=[]
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
    futures={pool.submit(one,b):b for b in INDEX['bundles']}
    for f in concurrent.futures.as_completed(futures):
        b=futures[f]
        try:results.append(f.result())
        except Exception as e:results.append({'container':b['name'],'error':str(e)})
        if len(results)%20==0:print(f'raw containers listed: {len(results)}/{len(futures)}',flush=True)
counts=collections.Counter();scripts=collections.Counter();shader=collections.Counter();uncertain=[];total=0
fields=['container','serializedAsset','type','pathId','name','identityStatus']
with (OUT/'source-objects.tsv').open('w',encoding='utf-8',newline='') as f,(OUT/'source-script-shader-movie-objects.tsv').open('w',encoding='utf-8',newline='') as narrow:
    writer=csv.DictWriter(f,fields,delimiter='\t');writer.writeheader();small=csv.DictWriter(narrow,fields,delimiter='\t');small.writeheader()
    for b in INDEX['bundles']:
        p=LIST/(b['name']+'.tsv')
        if not p.is_file():continue
        possible=[a['name'] for a in b['assets'] if a.get('objectCount',0)>0]
        asset=possible[0] if len(possible)==1 else None
        for line in p.read_text(encoding='utf-8-sig',errors='strict').splitlines():
            if line.startswith('# '):asset=line[2:];continue
            parts=line.split('\t',3)
            if len(parts)!=4 or not parts[0].lstrip('-').isdigit():
                if line.strip():uncertain.append({'container':b['name'],'line':line[:300]})
                continue
            pid,typeid,kind,name=parts
            row=dict(zip(fields,[b['name'],asset or '',kind,pid,name,'scoped' if asset else 'ambiguous-serialized-asset']))
            writer.writerow(row);counts[kind]+=1;total+=1
            if kind in ['MonoScript','Shader','MovieTexture','Font','TextAsset']:small.writerow(row)
            if kind=='MonoScript':scripts[name]+=1
            if kind=='Shader':shader[name]+=1
            if not asset:uncertain.append(row)
report={'schema':'fusionforge.whole-client-raw-objects.v1','role':'primary','producer':'tools/legacy-sources/audit-whole-client-objects.py',
        'containers':sorted(results,key=lambda x:x['container']),'objects':total,'typeCounts':dict(counts),
        'monoScriptNames':dict(scripts),'shaderNames':dict(shader),'unresolved':uncertain,
        'limitations':['Names are not semantic equivalence. Full pointer ownership, material state and runtime acceptance remain separate.']}
(OUT/'raw-objects-summary.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'containers':len(results),'objects':total,'errors':sum(r.get('exitCode',1)!=0 for r in results),'unresolved':len(uncertain),'scriptNames':len(scripts),'shaderNames':len(shader)}),flush=True)
