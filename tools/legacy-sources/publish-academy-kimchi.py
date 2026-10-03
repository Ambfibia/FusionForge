"""Stage/replay the Academy Kimchi model with the recovered original Fusion atlas."""
import argparse, base64, copy, hashlib, importlib.util, json, os
from pathlib import Path

def module(name, file):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(file))
    result = importlib.util.module_from_spec(spec); spec.loader.exec_module(result); return result
h = module('kimchi_glb', 'stage-fusion-kimchi-eyes.py')
c = module('kimchi_closure', 'close-npc-native-textures.py')
REL = 'characters/npcs/fusion_kimchi/fusion_kimchi.glb'
DOCS = ['_runtime/characters.json', 'data/tables/table-set.json', 'data/tables/npc_texture_overrides.json']
TEX = 'textures/npc/fusion_kimchi.png'
def digest(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def item(root, name):
    p = root/name
    return dict(path=name, bytes=p.stat().st_size, sha256=digest(p))
def npc(d): return next(t['value']['m_pNpcTable'] for t in d['tables'] if 'm_pNpcTable' in t['value'])

def stage(w, native, target):
    assert target.resolve().is_relative_to(Path('work').resolve())
    source = w/'strict-recovery/candidate/models/npc/npc_kimchi.glb'
    d, binary = h.glb(source)
    assert len(d['meshes']) == 4 and len(d['animations']) == 3
    assert all(len(m['primitives']) == 1 for m in d['meshes'])
    # Reuse a shared chain only after comparing the full pixel/sampler contract.
    source_bindings = c.bindings(d); chosen = {}; extra = []
    for candidate in sorted((native/'characters').rglob('*.glb')):
        for binding in c.bindings(c.glb(candidate)):
            for src in source_bindings:
                key = c.signature(src)
                if key in chosen or key != c.signature(binding): continue
                refs = [(candidate.parent/m['uri']).resolve() for m in binding['mipLevels']]
                if not all(p.is_relative_to(native.resolve()) and '/textures/shared/' in p.as_posix() for p in refs): continue
                payload = [(source.parent/m['uri']).read_bytes() for m in src['mipLevels']]
                if payload == [p.read_bytes() for p in refs]: chosen[key] = refs
        if len(chosen) == len(source_bindings): break
    redirects = {}
    for src in source_bindings:
        key = c.signature(src)
        if key not in chosen:
            stem = 'toonramp9_' + hashlib.sha256(repr(key).encode()).hexdigest()[:12]
            refs = [native/(f'textures/shared/{stem}.png' if i == 0 else f'textures/shared/{stem}.mips/mip-{i:02}.png') for i in range(len(src['mipLevels']))]
            for level, p in zip(src['mipLevels'], refs):
                name = p.relative_to(native).as_posix(); payload = (source.parent/level['uri']).read_bytes()
                h.put(target/name, payload); extra.append(name)
        else: refs = chosen[key]
        for level, p in zip(src['mipLevels'], refs):
            assert hashlib.sha256((source.parent/level['uri']).read_bytes()).hexdigest() == level['pngSha256']
            redirects[level['uri']] = os.path.relpath(p, (native/REL).parent).replace('\\', '/')
            if p.exists(): h.put(target/p.relative_to(native.resolve()), p.read_bytes())
    for node in c.walk(d):
        for k, v in list(node.items()):
            if isinstance(v, str) and v in redirects: node[k] = redirects[v]
    d['nodes'][0]['name'] = 'fusion_kimchi'
    payload = h.pack(d, binary); h.put(target/REL, payload)
    exact = h.read(w/'fusion-kimchi-texture.json')
    assert exact['name'] == 'fusion_kimchi' and exact['mipCount'] == 1 and exact['width'] == exact['height'] == 256
    png = base64.b64decode(exact['payload']['dataUrl'].split(',', 1)[1]); h.put(target/TEX, png)
    for name in DOCS: h.put(target/name, (native/name).read_bytes())
    catalog = h.read(target/DOCS[0]); entry = next(m for m in catalog['models'] if m['id'] == 'npc/fusion_kimchi')
    entry['glbBlake3'] = c.b3(payload); h.write(target/DOCS[0], catalog)
    tables = h.read(target/DOCS[1]); table = npc(tables); mesh_index = table['m_pNpcData'][3460]['m_iMesh']
    mesh = table['m_pNpcMeshData'][mesh_index]; assert mesh['m_pstrMMeshModelString'] == 'npc_3460_kimchi'
    mesh['m_pstrMTextureString'] = 'fusion_kimchi'; h.write(target/DOCS[1], tables)
    textures = h.read(target/DOCS[2]); textures['textures'] = [t for t in textures['textures'] if t['trueName'] != 'fusion_kimchi']
    textures['textures'].append(dict(trueName='fusion_kimchi',path=TEX,sha256=c.sha(png),sampler=dict(name='fusion_kimchi',minFilter='linear',magFilter='linear',wrapS='repeat',wrapT='repeat',legacyFilterMode=1,legacyWrapMode=0,anisotropyLevel=1,mipMapBias=0.0)))
    h.write(target/DOCS[2], textures)
    return dict(outputs=[REL,TEX,*DOCS,*extra],meshIndex=mesh_index,sourceGlbSha256=digest(source),geometryBinarySha256=c.sha(binary),textureSha256=c.sha(png),sharedRedirects=redirects)

def main():
    ap=argparse.ArgumentParser(description=__doc__); ap.add_argument('--work',type=Path,required=True);ap.add_argument('--native-root',type=Path,required=True);ap.add_argument('--stage',type=Path,required=True);ap.add_argument('--apply',action='store_true');ap.add_argument('--server-root',type=Path);a=ap.parse_args()
    w=a.work;native=a.native_root.resolve();result=stage(w,native,a.stage)
    replay=w/'replay-native';second=stage(w,native,replay);assert result==second
    for name in result['outputs']:assert (a.stage/name).read_bytes()==(replay/name).read_bytes()
    h.write(w/'academy-publication-plan.json',result)
    if not a.apply:return
    before=w/'accepted-before'
    for name in result['outputs']:
        if (native/name).exists() and not (before/name).exists():h.put(before/name,(native/name).read_bytes())
        h.put(native/name,(a.stage/name).read_bytes())
    server=[]
    if a.server_root:
        for name in ['tdata/xdt.json','bin/tdata/xdt.json']:
            p=a.server_root/name;old=h.read(p);new=copy.deepcopy(old)
            t=new['m_pNpcTable'];i=t['m_pNpcData'][3460]['m_iMesh'];assert i==result['meshIndex']
            row=t['m_pNpcMeshData'][i];assert row['m_pstrMMeshModelString']=='npc_3460_kimchi'
            row['m_pstrMTextureString']='fusion_kimchi'
            if not (w/'server-before'/name).exists():h.put(w/'server-before'/name,p.read_bytes())
            h.write(p,new);server.append(item(a.server_root,name))
    h.write(w/'academy-installation.json',dict(status='installed',replayByteExact=True,outputs=[item(native,n) for n in result['outputs']],server=server))

if __name__=='__main__':main()
