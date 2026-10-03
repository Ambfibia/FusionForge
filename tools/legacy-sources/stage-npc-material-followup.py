"""Stage the scoped September NPC material repair; never write the runtime directly."""
import argparse, base64, copy, importlib.util, json, os, struct
from pathlib import Path
import blake3

def read(p): return json.loads(p.read_bytes())
def put(p,b):
    p.parent.mkdir(parents=True,exist_ok=True)
    t=p.with_name(p.name+'.next');t.write_bytes(b);t.replace(p)
def write(p,d): put(p,(json.dumps(d,ensure_ascii=False,indent=2)+'\n').encode())
def glb(p):
    b=p.read_bytes(); n=struct.unpack_from('<I',b,12)[0]
    return json.loads(b[20:20+n]),b[20+n:]
def pack(d,tail):
    j=json.dumps(d,ensure_ascii=False,separators=(',',':')).encode();j+=b' '*(-len(j)%4)
    return b'glTF'+struct.pack('<IIII',2,20+len(j)+len(tail),len(j),0x4e4f534a)+j+tail

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--work',type=Path,required=True);ap.add_argument('--stage',type=Path,required=True);a=ap.parse_args()
    root=a.stage.resolve();assert root.is_relative_to(Path('work').resolve())
    spec=importlib.util.spec_from_file_location('closure',Path(__file__).with_name('close-npc-native-textures.py'));c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
    spec=importlib.util.spec_from_file_location('stage',Path(__file__).with_name('stage-npc-repair.py'));s=importlib.util.module_from_spec(spec);spec.loader.exec_module(s)
    registry=read(root/'_runtime/characters.json'); targets=[]
    for source in sorted((a.work/'recovery/candidate/models/npc').glob('*.glb')):
        if source.stem.startswith('kimchi-') or source.stem=='npc_kimchi':continue
        model=next(m for m in registry['models'] if m['logicalName']==source.stem)
        targets.append((root/model['glb'],source))
    for model in registry['models']:
        if model['id'] in ['npc/npc_deedee2','npc/npc_dexter2','npc/npc_dexter2_pistol','npc/npc_dexter2_sword']:
            p=root/model['glb'];targets.append((p,p))
    # Certify immutable shared chains by full sampler/color/mip contract AND bytes.
    index={}
    for p in sorted((root/'characters').rglob('*.glb')):
        for b in c.bindings(c.glb(p)):
            refs=[(p.parent/m['uri']).resolve() for m in b['mipLevels']]
            if all(r.is_file() and ('textures/shared/' in r.relative_to(root).as_posix() or 'player/rendering/textures/' in r.relative_to(root).as_posix()) for r in refs):
                payload=[r.read_bytes() for r in refs]
                assert all(c.sha(x)==m['pngSha256'] for x,m in zip(payload,b['mipLevels']))
                index.setdefault(c.signature(b),(refs,payload))
    changes=[]
    for target,source in targets:
        d,tail=glb(source); redirects={}
        for b in c.bindings(d):
            payload=[(source.parent/m['uri']).read_bytes() for m in b['mipLevels']]
            assert all(c.sha(x)==m['pngSha256'] for x,m in zip(payload,b['mipLevels']))
            if c.signature(b) in index:
                refs,existing=index[c.signature(b)];assert payload==existing
            else:
                refs=[(target.parent/m['uri']).resolve() for m in b['mipLevels']]
                for r,x in zip(refs,payload):assert r.is_relative_to(root);put(r,x)
            for m,r in zip(b['mipLevels'],refs): redirects[m['uri']]=os.path.relpath(r,target.parent).replace('\\','/')
        for node in c.walk(d):
            for k,v in list(node.items()):
                if isinstance(v,str) and v in redirects:node[k]=redirects[v]
        # SetupNPC operates on Renderer.material: only slot zero is writable.
        for mesh in d['meshes']:
            for slot,primitive in enumerate(mesh['primitives']):
                primitive.setdefault('extras',{})['npcTableTextureWritable']=slot==0
        before=c.sha(target.read_bytes());after=pack(d,tail);put(target,after)
        model=next(m for m in registry['models'] if m['glb']==target.relative_to(root).as_posix())
        model['glbBlake3']=blake3.blake3(after).hexdigest()
        changes.append(dict(path=model['glb'],beforeSha256=before,sha256=c.sha(after),source=str(source),redirects=redirects))
    write(root/'_runtime/characters.json',registry)
    catalog=read(root/'data/tables/npc_texture_overrides.json');entries={t['trueName']:t for t in catalog['textures']}
    retained=s.previous_build_retention.retained_npc_textures()
    for folder in sorted((a.work/'recovery/textures').iterdir()):
        if not (folder/'recovery.json').exists() or folder.name in retained:continue
        name=folder.name;exact=read(folder/'exact.json');path=f'textures/npc/{name}.png'
        put(root/path,(folder/(name+'.png')).read_bytes())
        entries[name]=dict(trueName=name,path=path,sha256=c.sha((root/path).read_bytes()),sampler=s.sampler(exact,name))
    catalog['textures']=sorted(entries.values(),key=lambda x:x['trueName']);write(root/'data/tables/npc_texture_overrides.json',catalog)
    doc=read(root/'data/tables/table-set.json');table=next(t['value']['m_pNpcTable'] for t in doc['tables'] if 'm_pNpcTable' in t['value'])
    old=read(Path('work/legacy-sources/retrobution-20260821-npc-review/old-xdt.json'))['m_pNpcTable']
    row=table['m_pNpcData'][3380];src=old['m_pNpcData'][3380];mesh=old['m_pNpcMeshData'][src['m_iMesh']]
    if mesh not in table['m_pNpcMeshData']:table['m_pNpcMeshData'].append(copy.deepcopy(mesh))
    row['m_iMesh']=table['m_pNpcMeshData'].index(mesh)
    for key in ['m_fScale','m_iHeight','m_fAnimationSpeed','m_fWalkAnimationSpeed','m_fRunAnimationSpeed']:row[key]=src[key]
    write(root/'data/tables/table-set.json',doc)
    protected={'npcicon_23','npcicon_68','npcicon_69','npcicon_70','npcicon_116','mobicon_286'}
    icons=[]
    oldwork=Path('work/legacy-sources/npc-repair-20260905')
    for folder in ['icons-exact','retro-icons-exact','icons-repaired-exact']:
        for p in sorted((oldwork/folder).glob('*.exact.json')):
            name=p.name.removesuffix('.exact.json');prefix,number=name.rsplit('_',1)
            if f'{prefix}_{int(number)}' in protected:continue
            family={'npcicon':'npc','mobicon':'mobs','hnpcicon':'hnpc'}[prefix]
            path=f'icons/entities/{family}/{prefix}_{int(number):02}.png'
            exact=read(p);payload=base64.b64decode(exact['payload']['dataUrl'].split('base64,',1)[1]);put(root/path,payload)
            icons.append(dict(path=path,source=str(p),sha256=c.sha(payload)))
    write(a.work/'stage-models.json',changes);write(a.work/'stage-icons.json',icons)
    print('staged models',len(changes),'icons',len(icons),flush=True)
if __name__=='__main__':main()
