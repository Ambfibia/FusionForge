"""Inventory every native GLB material/animation contract without changing assets."""
import collections,csv,json,struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];OUT=ROOT/'work/legacy-sources/whole-client-audit-20260905';ASSETS=ROOT.parent/'FFOneClient/assets/game'
materials=[];animations=[];dynamic=[];counts=collections.Counter();issues=[]
for p in sorted(ASSETS.rglob('*.glb')):
    rel=p.relative_to(ASSETS).as_posix()
    with p.open('rb') as f:
        hdr=f.read(20);length,kind=struct.unpack('<II',hdr[12:20]);d=json.loads(f.read(length))
    counts['glbs']+=1;counts['meshes']+=len(d.get('meshes',[]));counts['skins']+=len(d.get('skins',[]))
    for i,m in enumerate(d.get('materials',[])):
        e=m.get('extras',{}).get('ffone',{});shader=e.get('declaredShaderName',e.get('legacyShaderName','no-native-shader-contract'))
        materials.append({'file':rel,'index':i,'shader':shader,'queue':e.get('renderQueue'),'passes':len(e.get('passes',[])),'textureBindings':len(e.get('textureBindings',[]))})
        for t in e.get('textureBindings',[]):
            if t.get('dynamicTexture'):dynamic.append({'file':rel,'material':i,'slot':t.get('slot'),'kind':t['dynamicTexture'].get('objectType'),'shader':shader})
            for mip in t.get('mipLevels') or []:
                if isinstance(mip,dict) and mip.get('uri') and not (p.parent/mip['uri']).is_file():issues.append({'file':rel,'material':i,'uri':mip['uri'],'kind':'missing-mip'})
    for i,a in enumerate(d.get('animations',[])):
        animations.append({'file':rel,'index':i,'name':a.get('name',''),'channels':len(a.get('channels',[])),'samplers':len(a.get('samplers',[]))})
for name,rows,fields in [('native-materials.tsv',materials,['file','index','shader','queue','passes','textureBindings']),('native-animation-clips.tsv',animations,['file','index','name','channels','samplers']),('native-dynamic-textures.tsv',dynamic,['file','material','slot','kind','shader'])]:
    with (OUT/name).open('w',encoding='utf-8',newline='') as f:
        w=csv.DictWriter(f,fields,delimiter='\t');w.writeheader();w.writerows(rows)
report={'schema':'fusionforge.native-render-contract-inventory.v1',**dict(counts),'materials':len(materials),'shaderCounts':dict(collections.Counter(m['shader'] for m in materials)),'animationClips':len(animations),'dynamicTextures':dynamic,'mipIssues':issues,
        'limitations':['Native counts are not source coverage; shared objects and material passes change the counting unit.','No per-frame pose, pixel or pass-order equality claim.']}
(OUT/'render-summary.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps(report,ensure_ascii=False,indent=2))
