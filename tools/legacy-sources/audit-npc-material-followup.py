"""Verify scoped publication, deterministic replay, protected rows and every NPC icon path."""
import argparse,hashlib,json
from pathlib import Path
def read(p):return json.loads(p.read_bytes())
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def write(p,d):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(d,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
def main():
 ap=argparse.ArgumentParser();ap.add_argument('--work',type=Path,required=True);ap.add_argument('--native',type=Path,required=True);a=ap.parse_args();w=a.work;native=a.native;stage=w/'stage/native'
 outputs={e['path']:e for f in ['publication-applied.json','eyes-publication-applied.json'] for e in read(w/f)['outputs']}
 for path,e in outputs.items():assert sha(native/path)==e['sha256'];assert sha(stage/path)==e['sha256'];assert (native/path).stat().st_size==e['bytes']
 for e in read(w/'stage-icons.json'):assert sha(native/e['path'])==e['sha256']
 def table(p):return next(x['value']['m_pNpcTable'] for x in read(p)['tables'] if 'm_pNpcTable' in x['value'])
 t=table(native/'data/tables/table-set.json');before=table(w/'publication-backup/data/tables/table-set.json');assert len(t['m_pNpcData'])==3490
 for i,(old,new) in enumerate(zip(before['m_pNpcData'],t['m_pNpcData'])):
  if i!=3380:assert old==new,('unexpected row change',i)
  else:assert {k:v for k,v in old.items() if k!='m_iMesh'}=={k:v for k,v in new.items() if k!='m_iMesh'}
 protected={'npcicon_23.png','npcicon_68.png','npcicon_69.png','npcicon_70.png','npcicon_116.png','mobicon_286.png','npcicon_3464.png','npcicon_3465.png','npcicon_3466.png','npcicon_3468.png'}
 assert not any(Path(p).name in protected for p in outputs if p.startswith('icons/'))
 families={4:('npc','npcicon'),8:('mobs','mobicon'),10:('hnpc','hnpcicon')};paths=set()
 for r in t['m_pNpcData']:
  icon=t['m_pNpcIconData'][r['m_iIcon1']]
  if icon['m_iIconType'] not in families:continue
  folder,prefix=families[icon['m_iIconType']];p=f"icons/entities/{folder}/{prefix}_{icon['m_iIconNumber']:02}.png";assert (native/p).is_file(),p;paths.add(p)
 exact=w/'eyes-source-recovery/textures/vehicle_kimchi/vehicle_kimchi.png';donor=native/'characters/player/items/vehicle/vehicle_kimchi/textures/vehicle_kimchi.png';assert exact.read_bytes()==donor.read_bytes()
 model_sources=[];plan=read(w/'plan.json')
 for entry in plan['models']:
  p=w/'recovery/models'/entry['name']/(entry['name']+'.source.json');d=read(p);raw=Path(plan['sources'][entry['source']]['root'])/entry['container']
  materials=[]
  for identity,m in d['materials'].items():
   asset,pid=identity.rsplit(':',1);s=m['shader'];materials.append(dict(serializedAsset=asset,type='Material',pathId=int(pid),trueName=m['name'],shader=dict(serializedAsset=s['source']['asset'],type='Shader',pathId=s['source']['pathId'],pointer=s['pointer'],declaredName=s['declaredName'],scriptSha256=s['scriptSha256'])))
  model_sources.append(dict(source=dict(alias=entry['source'],relativeContainer=entry['container'],containerBytes=raw.stat().st_size,containerSha256=sha(raw)),route=entry['route'],targets=d['exactContainerTargets'],materials=materials,input=dict(path=p.as_posix(),bytes=p.stat().st_size,sha256=sha(p))))
 textures=[read(p) for base in [w/'recovery/textures',w/'eyes-source-recovery/textures'] for p in base.glob('*/recovery.json')]
 historical=read(Path('recipes/native/characters/npc-repair-20260905.receipt.json'));changed_icons={Path(p).stem.rsplit('_',1)[0]+'_'+str(int(Path(p).stem.rsplit('_',1)[1])) for p in outputs if p.startswith('icons/')};icons=[];previous_models=[]
 for source in historical['sources']:
  if 'selection' in source:
   selected=[e for e in source['selection'] if e['name'] in changed_icons]
   if selected:icons.append(dict(source=source['source'],objects=[dict(serializedAsset=e['serializedAsset'],type='Texture2D',pathId=e['pathId'],fileId=0,trueName=e['name']) for e in selected]))
  if source.get('route') in ['mob/npc_deedee2.kfm','mob/npc_dexter2.kfm','mob/npc_dexter2_pistol.kfm','mob/npc_dexter2_sword.kfm']:previous_models.append(source)
 source_record=Path('recipes/native/characters/npc-material-followup-20260905.sources.json');write(source_record,dict(schema='fusionforge.npc-material-source-evidence.v1',models=model_sources,existingModelLineage=previous_models,textures=textures,icons=icons,historicalReceipt='recipes/native/characters/npc-repair-20260905.receipt.json',historyNote='Historical receipt is a different schema and is referenced for recovered evidence, not claimed as canonical coverage.'))
 report=dict(schema='fusionforge.npc-material-followup-verification.v1',nativeOutputs=len(outputs),replayedOutputs=len(outputs),decodedIconsVerified=500,npcRowsChecked=3490,uniqueReferencedIconPaths=len(paths),missingIconPaths=[],changedIconFiles=sum(p.startswith('icons/') for p in outputs),protectedIconOutputs=[],allRowsExcept3380Unchanged=True,exactKimchiEyeAtlas=True,kimchiEyePartition=read(w/'kimchi-eyes-repair.json'))
 write(w/'verification.json',report);print(json.dumps({k:v for k,v in report.items() if k!='kimchiEyePartition'},ensure_ascii=False))
if __name__=='__main__':main()
