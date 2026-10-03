"""Focused read-only table/effect/world follow-up to audit-whole-client.py.
Positional table deltas and numeric/token references remain review candidates.
"""
import collections, csv, hashlib, importlib.util, json, re, struct
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]
OUT=ROOT/'work/legacy-sources/whole-client-audit-20260905'
NATIVE=ROOT.parent/'FFOneClient'
ASSETS=NATIVE/'assets/game'
def read(p): return json.loads(p.read_text(encoding='utf-8-sig'))
def save(name,data): (OUT/name).write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
def tsv(name,rows,fields):
    with (OUT/name).open('w',encoding='utf-8',newline='') as f:
        w=csv.DictWriter(f,fields,delimiter='\t',extrasaction='ignore'); w.writeheader()
        for r in rows:w.writerow({k:json.dumps(v,ensure_ascii=False) if isinstance(v,(dict,list)) else v for k,v in r.items()})

source=read(OUT/'primary-xdt.json')
native=next(t['value'] for t in read(ASSETS/'data/tables/table-set.json')['tables'] if 'm_pNpcTable' in t['value'])
tables=[]; details=[]
def compare(a,b,path,c):
    if type(a) in (int,float) and type(b) in (int,float):
        if a==b:return
        if (isinstance(a,float) or isinstance(b,float)) and struct.pack('<f',a)==struct.pack('<f',b):c['f32Equivalent']+=1;return
    if isinstance(a,dict) and isinstance(b,dict):
        for k in sorted(a.keys()|b.keys()):
            if k not in a: c['nativeAddedFields']+=1; continue
            if k not in b: c['nativeMissingFields']+=1; details.append({'path':path+'/'+k,'kind':'missing-field'});continue
            compare(a[k],b[k],path+'/'+k,c)
    elif isinstance(a,list) and isinstance(b,list):
        if len(a)!=len(b): details.append({'path':path,'kind':'array-length','primary':len(a),'native':len(b)})
        c['nativeAdditionalArrayEntries']+=max(0,len(b)-len(a));c['nativeMissingArrayEntries']+=max(0,len(a)-len(b))
        for i,(x,y) in enumerate(zip(a,b)):compare(x,y,path+'/'+str(i),c)
    elif a!=b:
        c['changedLeaves']+=1
        details.append({'path':path,'kind':'value','primary':a,'native':b})
for key in sorted(k for k in source if k.startswith('m_p')):
    c=collections.Counter()
    if key not in native:c['missingWholeTable']=1
    else:compare(source[key],native[key],key,c)
    tables.append({'table':key,**dict(c),'status':'structural-delta-not-bug-classification'})
tsv('table-deltas.tsv',tables,['table','changedLeaves','f32Equivalent','nativeAddedFields','nativeMissingFields','nativeAdditionalArrayEntries','nativeMissingArrayEntries','missingWholeTable','status'])
save('table-delta-details.json',details)

published={e['effectId'] for e in read(ASSETS/'map/shared/effects/catalog.json')['effects']}|{e['effectId'] for e in read(ASSETS/'map/shared/projectiles/catalog.json')['particleEffects']}
effect_fields=[]
def walk(value,path):
    if isinstance(value,dict):
        for key,v in value.items():
            if 'Effect' in key and isinstance(v,int) and not isinstance(v,bool) and v>0:
                effect_fields.append({'path':path+'/'+key,'id':v,'nativeClosureEntry':v in published,'status':'table-effect-field-candidate; consumer-required'})
            walk(v,path+'/'+key)
    elif isinstance(value,list):
        for i,v in enumerate(value):walk(v,path+'/'+str(i))
walk(source,'')
tsv('table-effect-references.tsv',effect_fields,['path','id','nativeClosureEntry','status'])
skill_buff=source['m_pSkillTable']['m_pSkillBuffData']
buff=[]
for i,r in enumerate(skill_buff):
    for key in ('m_iBuffEffect','m_iBuffEffectInstant'):
        v=r.get(key,0)
        if v>0:buff.append({'row':i,'buffNumber':r.get('m_iBuffNumber'),'field':key,'id':v,'nativeClosureEntry':v in published})
tsv('skill-buff-effect-gaps.tsv',buff,['row','buffNumber','field','id','nativeClosureEntry'])

behaviours=[]; classes=collections.Counter(); dynamics=[]
for p in sorted((ASSETS/'map/tiles').glob('*/behaviour.json')):
    d=read(p)
    counts={k:len(v) for k,v in d.items() if isinstance(v,list) and k!='tile'}
    for r in d.get('triggers',[]):classes[r.get('kind','unknown')]+=1
    for r in d.get('rigidBodies',[]):
        if not r.get('isKinematic',True):dynamics.append({'tile':d['id'],'body':r})
    behaviours.append({'tile':d['id'],**counts})
tsv('world-behaviour-per-tile.tsv',behaviours,['tile','animations','animationClips','billboards','blockers','effectEmitters','effectPrefabClosures','rigidBodies','triggerVolumes','triggers','visibilitySwitches','waypoints'])
save('world-dynamic-body-candidates.json',dynamics)

# Account for runtime constants suffixed _0104 and numeric packet dispatch.
# Neither is sufficient to prove an actual production consumer.
packet_constants={}
cs=(OUT/'main-decompiled/csDefines.cs').read_text(encoding='utf-8')
for name,value in re.findall(r'public const uint (P_[A-Za-z0-9_]+)\s*=\s*([0-9]+)u?;',cs):packet_constants[name]=int(value)
native_sources={str(p.relative_to(NATIVE)).replace('\\','/'):p.read_text(encoding='utf-8-sig') for p in (NATIVE/'crates').rglob('*.rs')}
wire_types=collections.defaultdict(set)
for number,typename in re.findall(r'(0x[0-9a-fA-F_]+)\s*=>\s*([A-Za-z0-9_]+)::SIZE',native_sources['crates/ffone-protocol/src/wire_0104.rs']):
    wire_types[int(number.replace('_',''),16)].add(typename)
tokens=collections.defaultdict(set);nums=collections.defaultdict(set);type_refs=collections.defaultdict(set)
all_types=set(t for ts in wire_types.values() for t in ts)
for rel,text in native_sources.items():
    for token in set(re.findall(r'\b[A-Za-z_][A-Za-z0-9_]*\b',text)) & all_types:type_refs[token].add(rel)
    for token in set(re.findall(r'\bP_[A-Za-z0-9_]+\b',text)):
        tokens[re.sub(r'_0104$','',token).upper()].add(rel)
    for n in set(re.findall(r'\b0x[0-9a-fA-F_]+\b',text)):
        nums[int(n.replace('_',''),16)].add(rel)
    for n in set(re.findall(r'(?<![A-Za-z0-9_])(?:[0-9][0-9_]{7,})(?:u32|u64)?\b',text)):
        value=int(re.sub(r'u(?:32|64)$','',n).replace('_',''))
        nums[value].add(rel)
rows=[]
for name,value in sorted(packet_constants.items()):
    exact=sorted(tokens.get(name.upper(),set())); numeric=sorted(nums.get(value,set()))
    typed=sorted({p for t in wire_types[value] for p in type_refs[t]})
    runtime=sorted({p for p in exact+numeric+typed if not p.startswith('crates/ffone-protocol/')})
    rows.append({'name':name,'id':hex(value),'nativeTokenCandidates':exact,'numericCandidates':numeric,'wireTypes':sorted(wire_types[value]),'typedCandidates':typed,'outsideProtocolCandidates':runtime,'status':'candidate-not-handler-proof'})
tsv('packet-numeric-candidates.tsv',rows,['name','id','nativeTokenCandidates','numericCandidates','wireTypes','typedCandidates','outsideProtocolCandidates','status'])

en=read(ASSETS/'localization/en.json')['entries'];ru=read(ASSETS/'localization/ru.json')['entries']
placeholder=[]
for k in en.keys()&ru.keys():
    if sorted(re.findall(r'\{[^{}]+\}',en[k]))!=sorted(re.findall(r'\{[^{}]+\}',ru[k])):placeholder.append(k)
result={'tables':{'count':len(tables),'missing':[r for r in tables if r.get('missingWholeTable')],
                 'unchanged':[r['table'] for r in tables if not r.get('changedLeaves') and not r.get('nativeMissingFields') and not r.get('nativeMissingArrayEntries') and not r.get('nativeAdditionalArrayEntries') and not r.get('nativeAddedFields')],
                 'changed':[r for r in tables if r.get('changedLeaves') or r.get('nativeMissingArrayEntries') or r.get('nativeAdditionalArrayEntries')]},
 'effectFieldCandidates':len(effect_fields),'missingEffectFieldCandidateIds':sorted({r['id'] for r in effect_fields if not r['nativeClosureEntry']}),
 'skillBuffEffects':{'references':len(buff),'missingReferences':sum(not r['nativeClosureEntry'] for r in buff),'missingIds':sorted({r['id'] for r in buff if not r['nativeClosureEntry']})},
 'world':{'tiles':len(behaviours),'triggerKinds':dict(classes),'dynamicBodyCandidates':len(dynamics)},
 'packetConstants':len(rows),'noOutsideProtocolCandidate':[r['name'] for r in rows if not r['outsideProtocolCandidates']],
 'localization':{'en':len(en),'ru':len(ru),'enOnly':sorted(en.keys()-ru.keys()),'ruOnly':sorted(ru.keys()-en.keys()),'placeholderDifferences':placeholder}}
supplement={}
if (OUT/'tabledata-9.evidence.json').exists():
    primary_rows=read(OUT/'tabledata-9.evidence.json')['object']['value']['m_pElements']
    native_rows=read(ASSETS/'data/missions/client-npc-waypoints.json')['rows'];diff=[]
    for i,(a,b) in enumerate(zip(primary_rows,native_rows)):
        equal=a['m_iType']==b['npcType'] and all(struct.pack('<f',a['kPos'][axis])==struct.pack('<f',v) for axis,v in zip(('x','y','z'),b['clientPosition']))
        if not equal:diff.append({'row':i,'primary':a,'native':b})
    supplement['clientNpcWaypoints']={'primaryRows':len(primary_rows),'nativeRows':len(native_rows),'changedCommonRows':len(diff),'diff':diff}
    pfirst={};nfirst={}
    for r in primary_rows:pfirst.setdefault(r['m_iType'],tuple(r['kPos'][k] for k in ('x','y','z')))
    for r in native_rows:nfirst.setdefault(r['npcType'],tuple(r['clientPosition']))
    supplement['clientNpcWaypoints']['firstMatchChanges']=[{'npcType':k,'primary':pfirst[k],'native':nfirst.get(k)} for k in sorted(pfirst) if k not in nfirst or any(struct.pack('<f',a)!=struct.pack('<f',b) for a,b in zip(pfirst[k],nfirst[k]))]
    world=read(OUT/'tabledata-8.evidence.json')['object']['value']['m_pWorldNameData']
    text=(NATIVE/'crates/ffone-client/src/legacy_world_location.rs').read_text(encoding='utf-8')
    literal=re.findall(r'LegacyWorldLocation::new\(\s*([-\d.]+),\s*([-\d.]+),\s*([-\d.]+),\s*([-\d.]+),\s*"([^"]+)"\s*,?\s*\)',text,re.S)
    primary_locations=[(r['Area']['x'],r['Area']['y'],r['Area']['width'],r['Area']['height'],r['DongName']) for r in world if r['DongName']!='null' and r['Area']['width']>0 and r['Area']['height']>0]
    native_locations=[tuple(float(x) for x in r[:4])+(r[4],) for r in literal]
    supplement['worldNames']={'primaryNonemptyAreas':len(primary_locations),'nativeLiteralRows':len(native_locations),'exactOrderedEqual':primary_locations==native_locations,
        'changedCommonRows':[{'row':i,'primary':a,'native':b} for i,(a,b) in enumerate(zip(primary_locations,native_locations)) if a!=b]}
    primary_hnpc=read(OUT/'tabledata-6.evidence.json')['object']['value']['TableElement'];native_hnpc=read(ASSETS/'data/hnpc/catalog.json')['appearances']
    fields={'iHairColor':'hairColor','iHeight':'height','iShape':'shape','iSkinColor':'skinColor','iType':'legacyType'};hnpc_diffs=[]
    for i,(a,b) in enumerate(zip(primary_hnpc,native_hnpc)):
        for x,y in fields.items():
            if a[x]!=b[y]:hnpc_diffs.append({'row':i,'field':x,'primary':a[x],'native':b[y]})
    supplement['hnpcNumericAttributes']={'primaryRows':len(primary_hnpc),'nativeRows':len(native_hnpc),'changedCommonFields':hnpc_diffs,'parts':'semantic mesh/texture resolution requires separate ownership comparison'}
result['supplementalTables']=supplement
audio=read(ASSETS/'_runtime/audio.json')['assets']
index=read(ROOT/'work/projects/retrobution-ui-20260821.ffclient/cache/bundle-index.json')
audio_names=collections.defaultdict(list)
for a in audio:audio_names[a['trueName'].lower()].append(a['logicalKey'])
audio_candidates=[{'container':a['container'],'serializedAsset':a['asset'],'pathId':a['pathId'],'name':a['name'],'nativeCandidates':audio_names.get(a['name'].lower(),[]),'status':'runtime-folded-name-candidate-not-byte-proof'} for a in index['audioClips']]
tsv('audio-runtime-name-candidates.tsv',audio_candidates,['container','serializedAsset','pathId','name','nativeCandidates','status'])
result['audioRuntimeFoldedNames']={'sourceRecords':len(audio_candidates),'unresolvedRecords':sum(not a['nativeCandidates'] for a in audio_candidates),'unresolvedUniqueFoldedNames':len({a['name'].lower() for a in audio_candidates if not a['nativeCandidates']})}
save('contracts-summary.json',result)
print(json.dumps(result,ensure_ascii=False,indent=2))
