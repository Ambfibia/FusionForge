"""Publish reviewed follow-up outputs with scoped JSON merges and pinned preimages."""
import argparse,hashlib,importlib.util,json,shutil
from pathlib import Path
def read(p):return json.loads(p.read_bytes())
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest() if p.is_file() else None
def put(p,b):
 p.parent.mkdir(parents=True,exist_ok=True);t=p.with_name(p.name+'.next');t.write_bytes(b);t.replace(p)
def write(p,d):put(p,(json.dumps(d,ensure_ascii=False,indent=2)+'\n').encode())
def main():
 ap=argparse.ArgumentParser();ap.add_argument('--work',type=Path,required=True);ap.add_argument('--stage',type=Path,required=True);ap.add_argument('--native',type=Path,required=True);ap.add_argument('--apply',action='store_true');ap.add_argument('--eyes',action='store_true');a=ap.parse_args();w=a.work;root=a.stage.resolve();native=a.native.resolve();assert root.is_relative_to(Path('work').resolve()) and root!=native
 plan=w/('eyes-publication-plan.json' if a.eyes else 'publication-plan.json')
 if not a.apply:
  spec=importlib.util.spec_from_file_location('closure',Path(__file__).with_name('close-npc-native-textures.py'));c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
  if a.eyes:
   targets=[read(w/'kimchi-eyes-repair.json')];paths={'_runtime/characters.json'}
   current=read(native/'_runtime/characters.json');staged=read(root/'_runtime/characters.json');new=next(x for x in staged['models'] if x['id']=='npc/fusion_kimchi')
   for model in current['models']:
    if model['logicalName']=='npc_kimchi':model['legacyAliases']=[x for x in model.get('legacyAliases',[]) if x!='npc_3460_kimchi']
   assert not any(x['id']==new['id'] for x in current['models']);current['models'].append(new);write(root/'_runtime/characters.json',current)
   for e in targets:
    p=root/e['path'];paths.add(e['path'])
    for binding in c.bindings(c.glb(p)):
     for m in binding['mipLevels']:
      ref=(p.parent/m['uri']).resolve();assert ref.is_relative_to(root);paths.add(ref.relative_to(root).as_posix());assert sha(ref)==m['pngSha256']
   outputs=[dict(path=p,bytes=(root/p).stat().st_size,sha256=sha(root/p),beforeSha256=sha(native/p)) for p in sorted(paths) if sha(root/p)!=sha(native/p)]
   write(plan,dict(schema='fusionforge.npc-material-followup-plan.v1',outputs=outputs));print('pinned eye outputs',len(outputs));return
  targets=read(w/'stage-models.json');paths={'_runtime/characters.json','data/tables/npc_texture_overrides.json','data/tables/table-set.json'}
  current=read(native/'_runtime/characters.json');staged=read(root/'_runtime/characters.json');models={x['glb']:x for x in staged['models']};changed={x['path'] for x in targets}
  for i,m in enumerate(current['models']):
   if m['glb'] in changed:current['models'][i]=models[m['glb']]
  write(root/'_runtime/characters.json',current)
  current=read(native/'data/tables/npc_texture_overrides.json');staged=read(root/'data/tables/npc_texture_overrides.json');names={p.name for p in (w/'recovery/textures').iterdir() if (p/'recovery.json').is_file()};entries={x['trueName']:x for x in current['textures']}
  # Owner-retained previous-build textures (retain-previous-build-assets.py) are never republished.
  spec=importlib.util.spec_from_file_location('retention',Path(__file__).with_name('previous_build_retention.py'));r=importlib.util.module_from_spec(spec);spec.loader.exec_module(r);names-=r.retained_npc_textures()
  for x in staged['textures']:
   if x['trueName'] in names:entries[x['trueName']]=x;paths.add(x['path'])
  current['textures']=sorted(entries.values(),key=lambda x:x['trueName']);write(root/'data/tables/npc_texture_overrides.json',current)
  current=read(native/'data/tables/table-set.json');staged=read(root/'data/tables/table-set.json');t=next(x['value']['m_pNpcTable'] for x in current['tables'] if 'm_pNpcTable' in x['value']);s=next(x['value']['m_pNpcTable'] for x in staged['tables'] if 'm_pNpcTable' in x['value']);r=s['m_pNpcData'][3380];mesh=s['m_pNpcMeshData'][r['m_iMesh']]
  if mesh not in t['m_pNpcMeshData']:t['m_pNpcMeshData'].append(mesh)
  target=t['m_pNpcData'][3380];target['m_iMesh']=t['m_pNpcMeshData'].index(mesh)
  for key in ['m_fScale','m_iHeight','m_fAnimationSpeed','m_fWalkAnimationSpeed','m_fRunAnimationSpeed']:target[key]=r[key]
  write(root/'data/tables/table-set.json',current)
  for e in targets:
   p=root/e['path'];paths.add(e['path'])
   for binding in c.bindings(c.glb(p)):
    for m in binding['mipLevels']:
     ref=(p.parent/m['uri']).resolve();assert ref.is_relative_to(root);paths.add(ref.relative_to(root).as_posix());assert sha(ref)==m['pngSha256']
  paths.update(e['path'] for e in read(w/'stage-icons.json'))
  outputs=[]
  for path in sorted(paths):
   assert (root/path).is_file(),path
   if sha(root/path)!=sha(native/path):outputs.append(dict(path=path,bytes=(root/path).stat().st_size,sha256=sha(root/path),beforeSha256=sha(native/path)))
  write(plan,dict(schema='fusionforge.npc-material-followup-plan.v1',outputs=outputs));print('pinned outputs',len(outputs));return
 d=read(plan)
 for e in d['outputs']:
  assert (native/e['path']).resolve().is_relative_to(native)
  assert sha(native/e['path'])==e['beforeSha256'],('destination changed',e['path'])
  assert sha(root/e['path'])==e['sha256'],('staged output changed',e['path'])
 for e in d['outputs']:
  p=native/e['path']
  if p.exists():put(w/('eyes-publication-backup' if a.eyes else 'publication-backup')/e['path'],p.read_bytes())
  put(p,(root/e['path']).read_bytes());assert sha(p)==e['sha256']
 write(w/('eyes-publication-applied.json' if a.eyes else 'publication-applied.json'),d);print('published and verified',len(d['outputs']))
if __name__=='__main__':main()
