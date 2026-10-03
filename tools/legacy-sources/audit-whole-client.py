"""Read-only, whole-tree discovery audit; lexical candidates are NEVER parity proof.

Run from FusionForge after exact assembly extraction and FFSpy decompilation.
All generated inventories stay in --out under Editor work/. No native writes.
"""
from __future__ import annotations
import argparse, collections, csv, hashlib, json, re, struct
from pathlib import Path

EDITOR = Path(__file__).resolve().parents[2]

def digest(p):
    h = hashlib.sha256()
    with p.open('rb') as f:
        while b := f.read(1024 * 1024): h.update(b)
    return h.hexdigest()

def read(p): return json.loads(p.read_text(encoding='utf-8-sig'))

def dump(p, value):
    p.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')

def tsv(p, rows, fields):
    with p.open('w', newline='', encoding='utf-8') as f:
        w = csv.DictWriter(f, fields, delimiter='\t', extrasaction='ignore')
        w.writeheader()
        for row in rows:
            w.writerow({k: json.dumps(v, ensure_ascii=False) if isinstance(v, (list,dict)) else v for k,v in row.items()})

# Routing only. A type assigned to a domain has NOT been semantically compared.
DOMAINS = [
 ('protocol', r'^(sP_|csSocket|csDefines|sPacket|cnNetwork|NetworkStats)'),
 ('tables', r'(Table|Element|^XDT$|^Xdt|^BaseTable)'),
 ('inventory-equipment', r'(Equip|Inventory|OC.*Slot|Unequip|GumPopup|TuringPopup|ItemDisplay|ItemInfo|UserSlot|UserClothes|HatOption|CustomCosmetic)'),
 ('missions-guides', r'(Mission|Quest|Guide|Reward|Journal|Task)'),
 ('nano-skills', r'(Nano|Skill|Skil|Buff|Status|Attrib|SpecialState|Condition)'),
 ('combat', r'(Attack|Bullet|WarHead|Damage|DeadMotion|Weapon|Wpn|SwordTrail|MegaAttack|Overheat)'),
 ('transport-ep', r'(Transport|Trans$|Bus|Zipline|^Ep|^EP|GameFrameEp|Launcher|Waypoint)'),
 ('social-chat', r'(Chat|Buddy|Group|Slang|NameColor|PrintName)'),
 ('economy-trade', r'(Trade|Pc2pc|Cashmall|Vendor|Bank|Store|Combi|Enchant|CacuPopup|Barber)'),
 ('mail', r'(Email)'),
 ('race', r'(Race|sEP)'),
 ('tutorial-events', r'(tutorial|Tutorial|DexterShip|CutScene|Event|FirstUse|Halloween|Knishmas)'),
 ('audio-video', r'(Sound|Music|Voice|Movie|Autoplay)'),
 ('render-effects', r'(Effect|Particle|Glow|Water|Blur|Grayscale|Sepia|Twirl|Vortex|Billboard|VisibleSwitch|ColorCorrection|Contrast|EdgeDetect|Noise|Shadow)'),
 ('world-streaming', r'(Dong|World|Terrain|Asset|Resource|Caching|Culling|Environment|MiniMap|MapAttribute)'),
 ('actor-animation-movement', r'(Actor|Avatar|Move|Npc|NPC|UserContainer|Animation|Skin|Combiner|CombineChildren|ObjectTracker|FloatingPoint|Oni)'),
 ('camera-input-options', r'(Camera|Mouse|Input|Option|ScreenPivot|Coord|Mathfx|^eLang|^localized)'),
 ('login-character', r'(Login|CharCreation|Character|CharSelection|CharName|NameCreation|SubmitName|NameString|ServerSelection|VirtualServer)'),
 ('ui-shell', r'(GUI|Gui|Popup|SystemMessage|Help|Rule|Quit|Upsell|Resurrect|QuickSlot|TextManager|GameFrame|GameMode|MainGame|GlobalManager|GameCondition|SearchList|SkinFont)'),
]

def domain(name):
    for d, pat in DOMAINS:
        if re.search(pat, name, re.I): return d
    return 'unclassified-review-required'

def main():
    ap = argparse.ArgumentParser(__doc__)
    ap.add_argument('--source', required=True, type=Path)
    ap.add_argument('--native', required=True, type=Path)
    ap.add_argument('--index', required=True, type=Path)
    ap.add_argument('--main-decompiled', required=True, type=Path)
    ap.add_argument('--firstpass-decompiled', required=True, type=Path)
    ap.add_argument('--out', required=True, type=Path)
    args = ap.parse_args()
    out=args.out.resolve(); out.relative_to(EDITOR/'work'); out.mkdir(parents=True, exist_ok=True)
    native=args.native.resolve(); source=args.source.resolve(); index=read(args.index)
    summary={'schema':'fusionforge.whole-client-discovery.v1','role':'primary',
      'limitations':['All name/token matches are navigation candidates, not ownership or behavioral equivalence.',
      'Object counts come from the checked index; fresh hashes bind current raw bytes but do not re-prove each indexed object.',
      'No runtime/GPU parity claim. Source closure reachability and shader intentional changes require case-level review.']}

    # Read every native Rust file; retain explicit holes separately from symbol references.
    nfiles=[]; native_text={}; refs=collections.defaultdict(list); holes=[]
    holepat=re.compile(r'not yet|not implemented|not.*wired|OWNER_GAP|HTTP_GAP|remains fail.closed|ProductionOwnerBridgeUnavailable|no (?:native|production|typed).*owner', re.I)
    for p in sorted((native/'crates').rglob('*.rs')):
        text=p.read_text(encoding='utf-8-sig'); rel=p.relative_to(native).as_posix(); native_text[rel]=text
        nfiles.append({'path':rel,'bytes':p.stat().st_size,'sha256':digest(p),'lines':len(text.splitlines())})
        for line_no,line in enumerate(text.splitlines(),1):
            if holepat.search(line): holes.append({'path':rel,'line':line_no,'text':line.strip()[:700],'status':'candidate-review-required'})
        for token in set(re.findall(r'\b[A-Za-z_][A-Za-z_0-9]*\b',text)): refs[token].append(rel)
    tsv(out/'native-files.tsv',nfiles,['path','bytes','sha256','lines'])
    tsv(out/'native-explicit-gap-candidates.tsv',holes,['path','line','text','status'])
    print('native source read:',len(nfiles),flush=True)

    managed=[]; methods=[]; types=[]; source_packets=collections.defaultdict(list)
    for assembly,root in [('main',args.main_decompiled),('firstpass',args.firstpass_decompiled)]:
        for p in sorted(root.rglob('*.cs')):
            text=p.read_text(encoding='utf-8-sig'); name=p.stem; rel=p.relative_to(root).as_posix()
            decls=re.findall(r'\b(class|struct|enum|interface|delegate)\s+([A-Za-z_][A-Za-z_0-9]*)',text)
            d=domain(name); links=refs.get(name,[])
            lifecycle=sorted(set(re.findall(r'\b(Awake|Start|Update|FixedUpdate|LateUpdate|OnGUI|OnEnable|OnDisable|OnDestroy|OnTriggerEnter|OnTriggerExit)\s*\(',text)))
            packets=sorted(set(re.findall(r'\bP_[A-Z][A-Z0-9_]+\b',text)))
            for token in packets: source_packets[token].append(assembly+'/'+rel)
            row={'assembly':assembly,'path':rel,'sha256':digest(p),'bytes':p.stat().st_size,'lines':len(text.splitlines()),
                 'domain':d,'typeDeclarations':decls,'lifecycle':lifecycle,'nativeLiteralCandidates':links,'packetTokens':packets,
                 'status':'lexical-candidate' if links else 'no-exact-type-name-reference; semantic-review-required'}
            managed.append(row)
            for kind,typename in decls: types.append({'assembly':assembly,'file':rel,'kind':kind,'type':typename,'domain':d})
            # C# declaration discovery, including generated iterator methods; not an IL call graph.
            for no,line in enumerate(text.splitlines(),1):
                if re.match(r'\s*(public|private|protected|internal)\s+',line) and '(' in line and not re.search(r'\b(if|return|new)\b',line):
                    methods.append({'assembly':assembly,'file':rel,'line':no,'declaration':line.strip(),'domain':d,'status':'signature-candidate'})
    tsv(out/'managed-files.tsv',managed,['assembly','path','sha256','bytes','lines','domain','typeDeclarations','lifecycle','nativeLiteralCandidates','packetTokens','status'])
    tsv(out/'managed-types.tsv',types,['assembly','file','kind','type','domain'])
    tsv(out/'managed-method-candidates.tsv',methods,['assembly','file','line','declaration','domain','status'])
    packets=[]
    for token,locations in sorted(source_packets.items()):
        found=refs.get(token,[])
        packets.append({'packet':token,'sourceFiles':locations,'nativeReferences':found,
                        'outsideProtocolReferences':[f for f in found if not f.startswith('crates/ffone-protocol/')],
                        'status':'references-only-not-handler-proof'})
    tsv(out/'packets.tsv',packets,['packet','sourceFiles','nativeReferences','outsideProtocolReferences','status'])
    summary['managed']={'files':len(managed),'byAssembly':dict(collections.Counter(r['assembly'] for r in managed)),
                        'typeDeclarations':len(types),'methodCandidates':len(methods),'byDomain':dict(collections.Counter(r['domain'] for r in managed)),
                        'noExactNativeTypeReference':sum(not r['nativeLiteralCandidates'] for r in managed)}
    summary['packets']={'sourceTokens':len(packets),'withoutAnyNativeReference':[r['packet'] for r in packets if not r['nativeReferences']],
                         'withoutOutsideProtocolReference':[r['packet'] for r in packets if not r['outsideProtocolReferences']]}
    summary['nativeCode']={'files':len(nfiles),'lines':sum(r['lines'] for r in nfiles),'gapCandidates':len(holes)}
    print('managed source read:',len(managed),flush=True)

    # Inventory-driven raw read, never recursive legacy discovery.
    inv=list(csv.DictReader((EDITOR/'tools/legacy-sources/inventories/primary.tsv').open(encoding='utf-8-sig'),delimiter='\t'))
    raw=[]; indexed={b['name']:b for b in index['bundles']}; routes=[]; counts=collections.Counter()
    for row in inv:
        rel=row['relative_path']; p=(source/rel).resolve(); p.relative_to(source)
        if not p.is_file(): raw.append({'path':rel,'status':'missing'}); continue
        size=p.stat().st_size; b=indexed.get(rel)
        raw.append({'path':rel,'bytes':size,'sha256':digest(p),'inventoryBytes':int(row['bytes']),
                    'indexBytes':b['size'] if b else None,'status':'size-match' if size==int(row['bytes']) else 'inventory-stale',
                    'indexErrors':b.get('errors',[]) if b else []})
        if b:
            for asset in b.get('assets',[]):
                counts.update(asset.get('typeCounts',{}))
                for route in asset.get('containerPaths',[]):
                    routes.append({'container':rel,'serializedAsset':asset['name'],'route':route,'status':'indexed-navigation'})
    tsv(out/'source-containers.tsv',raw,['path','bytes','sha256','inventoryBytes','indexBytes','status','indexErrors'])
    tsv(out/'source-routes.tsv',routes,['container','serializedAsset','route','status'])
    summary['source']={'inventoryFiles':len(inv),'indexedBundles':len(index['bundles']),'rawBytes':sum(r.get('bytes',0) for r in raw),
       'types':dict(counts),'objects':sum(counts.values()),'routeRecords':len(routes),'indexSha256':digest(args.index),
       'anomalies':[r for r in raw if r['status']!='size-match' or r.get('indexErrors') or (r.get('indexBytes') is not None and r['bytes']!=r['indexBytes'])]}
    print('source raw containers hashed:',len(raw),flush=True)

    root=native/'assets/game'; files=[]; missing=[]; uri_refs=[]; schemas=collections.Counter(); behaviour=collections.Counter(); behaviour_samples=[]
    for p in sorted(root.rglob('*')):
        if not p.is_file(): continue
        rel=p.relative_to(root).as_posix(); size=p.stat().st_size
        files.append({'path':rel,'bytes':size,'sha256':digest(p),'extension':p.suffix.lower()})
        if p.suffix.lower()=='.glb':
            with p.open('rb') as f:
                hdr=f.read(20)
                if len(hdr)!=20 or hdr[:4]!=b'glTF': missing.append({'owner':rel,'issue':'bad-glb-header'}); continue
                length,kind=struct.unpack('<II',hdr[12:20]); data=json.loads(f.read(length))
            for image in data.get('images',[]):
                uri=image.get('uri')
                if not uri or uri.startswith('data:'): continue
                target=(p.parent/uri).resolve()
                try: target_rel=target.relative_to(root).as_posix()
                except ValueError: missing.append({'owner':rel,'uri':uri,'issue':'uri-outside-assets'}); continue
                exists=target.is_file(); uri_refs.append({'owner':rel,'uri':uri,'target':target_rel,'exists':exists})
                if not exists: missing.append({'owner':rel,'uri':uri,'target':target_rel,'issue':'missing-image'})
        elif p.suffix.lower()=='.json':
            try: data=read(p)
            except Exception as e: missing.append({'owner':rel,'issue':'json-parse','error':str(e)}); continue
            if isinstance(data,dict):
                schemas[data.get('schema','no-schema')]+=1
                if 'behaviour' in p.name:
                    behaviour_samples.append({'path':rel,'keys':list(data),'schema':data.get('schema')})
                    for k,v in data.items():
                        if isinstance(v,list): behaviour[k]+=len(v)
    tsv(out/'native-assets.tsv',files,['path','bytes','sha256','extension'])
    tsv(out/'native-glb-image-refs.tsv',uri_refs,['owner','uri','target','exists'])
    dump(out/'native-reference-issues.json',missing)
    dump(out/'native-behaviour-shape.json',behaviour_samples)
    summary['nativeAssets']={'files':len(files),'bytes':sum(r['bytes'] for r in files),'byExtension':dict(collections.Counter(r['extension'] for r in files)),
        'jsonSchemas':dict(schemas),'glbImageReferences':len(uri_refs),'issues':missing,'behaviourListTotals':dict(behaviour)}
    print('native assets hashed:',len(files),flush=True)

    audio=read(root/'_runtime/audio.json'); names=collections.defaultdict(list)
    for entry in audio['assets']: names[entry['trueName']].append(entry['logicalKey'])
    ar=[]
    for clip in index.get('audioClips',[]):
        ar.append({'container':clip['container'],'serializedAsset':clip['asset'],'pathId':clip['pathId'],'name':clip['name'],
                   'nativeExactNameCandidates':names.get(clip['name'],[]),'status':'name-match-not-payload-equivalence' if clip['name'] in names else 'unresolved-name-not-proven-missing'})
    tsv(out/'audio-candidates.tsv',ar,['container','serializedAsset','pathId','name','nativeExactNameCandidates','status'])
    audio_missing=[]
    for entry in audio['assets']:
        for file in entry['files']:
            if not (root/file['path']).is_file(): audio_missing.append({'key':entry['logicalKey'],'file':file['path']})
    summary['audio']={'indexedClips':len(ar),'nativeEntries':len(audio['assets']),'exactNameCandidates':sum(bool(r['nativeExactNameCandidates']) for r in ar),
       'unresolvedUniqueNames':sorted(set(r['name'] for r in ar if not r['nativeExactNameCandidates'])),'missingCatalogFiles':audio_missing,
       'nativeCategories':dict(collections.Counter(e['category'] for e in audio['assets']))}

    effects=read(root/'map/shared/effects/catalog.json'); projectiles=read(root/'map/shared/projectiles/catalog.json')
    source_ids={int(m.group(1)) for r in routes if r['container']=='Effects.resourceFile' for m in [re.fullmatch(r'prefabs/particle/effectscripts/es\[(\d+)\]\.prefab',r['route'])] if m}
    native_ids={e['effectId'] for e in effects['effects']}|{e['effectId'] for e in projectiles['particleEffects']}
    er=[]
    for effect_id in sorted(source_ids|native_ids):
        er.append({'id':effect_id,'sourceRoute':effect_id in source_ids,'nativeClosureEntry':effect_id in native_ids,'status':'closure-presence-only'})
    tsv(out/'effects.tsv',er,['id','sourceRoute','nativeClosureEntry','status'])
    summary['effects']={'sourceIds':len(source_ids),'nativeIds':len(native_ids),'missingNativeIds':sorted(source_ids-native_ids),
        'nativeOnlyIds':sorted(native_ids-source_ids),'runtimeReachability':'not established by route presence'}
    loc_en=read(root/'localization/en.json'); loc_ru=read(root/'localization/ru.json')
    summary['localizationShape']={'enKeys':list(loc_en)[:8],'ruKeys':list(loc_ru)[:8]}
    dump(out/'summary.json',summary)
    print(json.dumps({k:v for k,v in summary.items() if k in ['managed','nativeCode','effects']},ensure_ascii=False,indent=2))

if __name__=='__main__': main()
