"""Close staged NPC/equipment PNG references using exact complete native chains."""
import argparse, json, struct, os, hashlib, collections, re
from pathlib import Path
import blake3
def read(p): return json.loads(p.read_bytes())
def sha(b): return hashlib.sha256(b).hexdigest()
def b3(b): return blake3.blake3(b).hexdigest()
def put(p,b):
    p.parent.mkdir(parents=True,exist_ok=True);t=p.with_name(p.name+'.next');t.write_bytes(b);t.replace(p)
def walk(d):
    if isinstance(d,dict):
        yield d
        for v in d.values():yield from walk(v)
    elif isinstance(d,list):
        for v in d:yield from walk(v)
def glb(p):
    with p.open('rb') as f:
        header=f.read(20);assert header[:4]==b'glTF';return json.loads(f.read(struct.unpack_from('<I',header,12)[0]))
def bindings(d):
    return [b for m in d.get('materials',[]) for b in m.get('extras',{}).get('ffone',{}).get('textureBindings',[]) if b.get('mipLevels') and b.get('sampler') and b.get('uri')]
def signature(b):
    return (b['colorSpace'],json.dumps({k:v for k,v in b['sampler']['descriptor'].items() if k!='name'},sort_keys=True),tuple((m['width'],m['height'],m['pngSha256']) for m in b['mipLevels']))
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--work',type=Path,required=True);ap.add_argument('--stage',type=Path,required=True);a=ap.parse_args()
    root=a.stage.resolve();work=a.work;assert root.is_relative_to(Path('work').resolve())
    index={}
    for p in sorted(root.joinpath('characters').rglob('*.glb')):
        for binding in bindings(glb(p)):
            refs=[(p.parent/m['uri']).resolve() for m in binding['mipLevels']]
            if all(r.is_file() and ('textures/shared/' in r.relative_to(root).as_posix() or 'player/rendering/textures/' in r.relative_to(root).as_posix()) for r in refs):
                payload=[r.read_bytes() for r in refs]
                assert all(sha(x)==m['pngSha256'] for x,m in zip(payload,binding['mipLevels']))
                index.setdefault(signature(binding),(refs,payload))
    print('certified shared chains',len(index),flush=True)
    targets=[]
    items=read(root/'characters/player/items/catalog.json')['models']
    equipment=read(work/'equipment-new-candidate/equipment-logical-model-batch-report.json')['models']
    for m in equipment:
        installed=next(x for x in items if x['trueName']==m['trueName'])
        targets.append((root/installed['model']['path'],work/'equipment-new-candidate'/m['outputGlb']))
    characters=read(root/'_runtime/characters.json')['models']
    for folder in ['recovery-a','future-recovery','extra-recovery']:
        for source in (work/folder/'candidate/models/npc').glob('*.glb'):
            installed=next(x for x in characters if x['id']=='npc/'+source.stem)
            targets.append((root/installed['glb'],source))
    candidates=collections.defaultdict(list)
    for _,source in targets:
        for binding in bindings(glb(source)):
            candidates[signature(binding)].append((source,binding))
    for key,members in candidates.items():
        if key in index or len(members)<2:continue
        source,binding=members[0]
        payload=[(source.parent/m['uri']).read_bytes() for m in binding['mipLevels']]
        for other,b in members:
            assert payload==[(other.parent/m['uri']).read_bytes() for m in b['mipLevels']]
        name=re.sub('[^a-z0-9_]+','_',Path(binding['uri']).stem.lower())
        fingerprint=sha(repr(key).encode())[:12]
        base=root/f'textures/shared/{name}_{fingerprint}.png'
        refs=[base if i==0 else base.with_suffix('.mips')/f'mip-{i:02}.png' for i in range(len(payload))]
        for r,b in zip(refs,payload):
            if r.exists():assert r.read_bytes()==b
            else:put(r,b)
        index[key]=(refs,payload)
    hashes={};sizes={};report=[];removed=[]
    for p,source in targets:
        d=glb(p);src=glb(source);redirects={}
        for binding in bindings(d):
            key=signature(binding);source_binding=next(b for b in bindings(src) if signature(b)==key)
            payload=[(source.parent/m['uri']).read_bytes() for m in source_binding['mipLevels']]
            assert all(sha(x)==m['pngSha256'] for x,m in zip(payload,binding['mipLevels']))
            if key in index:
                refs,existing=index[key];assert payload==existing
            else:
                refs=[(p.parent/m['uri']).resolve() for m in binding['mipLevels']]
                for r,b in zip(refs,payload):assert r.is_relative_to(root);put(r,b)
            for old,r in zip(binding['mipLevels'],refs):redirects[old['uri']]=os.path.relpath(r,p.parent).replace('\\','/')
            for level,r,b in zip(source_binding['mipLevels'],refs,payload):
                local=(p.parent/level['uri']).resolve()
                if local!=r and local.is_relative_to(p.parent) and local.is_file():
                    assert local.read_bytes()==b
                    local.unlink();removed.append(local.relative_to(root).as_posix())
        for node in walk(d):
            for k,v in list(node.items()):
                if isinstance(v,str) and v in redirects:node[k]=redirects[v]
        before=p.read_bytes();n=struct.unpack_from('<I',before,12)[0];suffix=before[20+n:]
        j=json.dumps(d,ensure_ascii=False,separators=(',',':')).encode();j+=b' '*(-len(j)%4)
        after=b'glTF'+struct.pack('<IIII',2,20+len(j)+len(suffix),len(j),0x4e4f534a)+j+suffix
        if before!=after:
            put(p,after)
            for fn in [sha,b3]:hashes[fn(before)]=fn(after)
            sizes[p.relative_to(root).as_posix()]=len(after)
        report.append(dict(path=p.relative_to(root).as_posix(),beforeSha256=sha(before),sha256=sha(after),redirects=redirects,binSha256=sha(suffix)))
    # Propagate native ownership checksums through item -> set -> catalog -> HNPC.
    docs={p:read(p) for directory in ['characters/player/items','data/hnpc','_runtime'] for p in (root/directory).rglob('*.json')}
    for iteration in range(12):
        changed=False
        for p,d in docs.items():
            dirty=False
            for node in walk(d):
                for k,v in list(node.items()):
                    if isinstance(v,str) and v in hashes and hashes[v]!=v:node[k]=hashes[v];dirty=True
                if node.get('path') in sizes and 'bytes' in node and node['bytes']!=sizes[node['path']]:node['bytes']=sizes[node['path']];dirty=True
            if dirty:
                old=p.read_bytes();new=(json.dumps(d,ensure_ascii=False,indent=2)+'\n').encode();put(p,new)
                sizes[p.relative_to(root).as_posix()]=len(new)
                for fn in [sha,b3]:hashes[fn(old)]=fn(new)
                changed=True
        if not changed:break
        for h in hashes:
            while hashes[h] in hashes and hashes[hashes[h]]!=hashes[h]:hashes[h]=hashes[hashes[h]]
    else:raise ValueError('native hash graph failed to converge')
    put(work/'native-texture-closure.json',(json.dumps(report,indent=2)+'\n').encode())
    print('closed models',len(targets),flush=True)
    put(work/'native-texture-removed.json',(json.dumps(removed,indent=2)+'\n').encode())
if __name__=='__main__':main()
