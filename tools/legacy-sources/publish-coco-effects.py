"""Recover primary Coco particles, pickup mesh and reward art into native data."""
import argparse, base64, hashlib, json, re, shutil, subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ASSETS = ['CustomAssetBundle-fa9dbcf4604f64024b06ff1d5e375918',
          'CustomAssetBundle-b4f543c102ded400fbc6f1da25d9679a']
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def write(p, data):
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(data, indent=2)+'\n', encoding='utf-8')

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-root',type=Path,required=True)
    parser.add_argument('--navigation',type=Path,required=True)
    parser.add_argument('--work',type=Path,required=True)
    parser.add_argument('--target-root',type=Path)
    args=parser.parse_args(); w=args.work; w.mkdir(parents=True,exist_ok=True)
    stage=w/'native'; stage.mkdir(exist_ok=True)
    forge=ROOT/'target/debug/fusionforge.exe'
    def run(*a): subprocess.run([str(forge),*map(str,a)],check=True,cwd=ROOT)
    expected={
        'Effects.resourceFile':'2aa08e85b7aefa8f7e22705ee2f1581befa83cc5708b32116ce8a46a273595e9',
        'Tutorial.resourceFile':'729e9aad557709d4a75937b7478b6784b82f202c000d13127d1074213991f916',
        'main.unity3d':'01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef'}
    for name,digest in expected.items(): assert sha(args.source_root/name)==digest,name
    source=[]; objs=[]
    for i,name in enumerate(['Effects.resourceFile','Tutorial.resourceFile']):
        raw=args.source_root/name; dump=w/(name+'.json')
        run('dump-object',raw,'all',dump)
        a=json.loads(dump.read_text(encoding='utf-8'))
        assert {o['asset'] for o in a}=={ASSETS[i]}
        objs.append({o['pathId']:o for o in a})
        source.append(dict(alias='primary',container=name,bytes=raw.stat().st_size,sha256=sha(raw)))
    # The controller evidence proves fa9 external slot 1 is b4f; never flatten IDs.
    run('dump-object-evidence','primary',args.source_root,'Effects.resourceFile',17601,
        '--serialized-asset',ASSETS[0],'--type','MonoBehaviour','--allow-unresolved-pointers',
        '--out',w/'controller.evidence.json')
    proof=json.loads((w/'controller.evidence.json').read_text(encoding='utf-8'))
    assert any(e['fileId']==1 and e['filePath'].lower()==ASSETS[1].lower() for e in proof['externalFiles'])
    def ptr(asset,p,kind):
        slot=p['fileId']; target=asset if slot==0 else 1 if asset==0 and slot==1 else None
        assert target is not None, (asset,p)
        o=objs[target][p['pathId']]; assert o['type']==kind,(p,kind,o['type'])
        return target,o
    def component(go,kind,script=None):
        found=[o for o in objs[0].values() if o['type']==kind and o['value'].get('m_GameObject')==dict(fileId=0,pathId=go)]
        if script:
            found=[o for o in found if ptr(0,o['value']['m_Script'],'MonoScript')[1]['value']['m_ClassName']==script]
        assert len(found)==1,(go,kind,len(found));return found[0]['value']
    def vec(v):return [v[k] for k in 'xyz']
    def native(v):x,y,z=vec(v);return [-x,y,z]
    def curve(v):
        assert v['m_PreInfinity']==v['m_PostInfinity']==2
        return [[k['time'],k['value'],k['inSlope'],k['outSlope']] for k in v['m_Curve']]
    textures={}; plans=[]
    blend_names={'SrccolorOne':'SrcColorOne','OneOne':'OneOne','SrcalphaOne':'SrcAlphaOne',
                 'SrcalphaInvsrccolor':'SrcAlphaOneMinusSrcColor','InvsrcalphaOne':'OneMinusSrcAlphaOne',
                 'OneInvsrccolor':'OneOneMinusSrcColor','ZeroInvsrcalpha':'ZeroOneMinusSrcAlpha'}
    for eid,root,name in [(397,9344,'pickup'),(398,10965,'item-idle'),(401,9808,'buff-idle')]:
        c=component(root,'MonoBehaviour','EffectEmitterController')
        assert c['conformToScale']==0
        emitters=[]
        for entry in c['particles']:
            _,go=ptr(0,entry['particlePrefab'],'GameObject'); gid=go['pathId']
            e=component(gid,'MonoBehaviour','ParticleEmitterController')
            r=component(gid,'ParticleRenderer'); a=component(gid,'ParticleAnimator')
            assert a['damping']==1 and a['sizeGrow']==0
            assert all(vec(a[k])==[0,0,0] for k in ['rndForce','localRotationAxis','worldRotationAxis'])
            assert len(r['m_Materials'])==1
            ma,m=ptr(0,r['m_Materials'][0],'Material');m=m['value']
            _,shader=ptr(ma,m['m_Shader'],'Shader')
            script=shader['value']['m_Script']; shader_name=re.search(r'Shader\s+"([^"]+)"',script).group(1)
            blend=blend_names[shader_name.removeprefix('particle_blend').removesuffix('_zwriteOff_cullOff')]
            props=m['m_SavedProperties']; tint=next(v for k,v in props['m_Colors'] if k['name']=='_TintColor')
            tex=next(v for k,v in props['m_TexEnvs'] if k['name']=='_MainTex')
            assert tex['m_Scale']==dict(x=1.0,y=1.0) and tex['m_Offset']==dict(x=0.0,y=0.0) and tex['m_Rotation']==0
            ta,t=ptr(ma,tex['m_Texture'],'Texture2D')
            settings=t['value']['m_TextureSettings']
            assert settings['m_FilterMode']==1 and settings['m_WrapMode']==0 and settings['m_MipBias']==0 and settings['m_Aniso']==1
            route='effects/shinies/textures/'+t['name'].lower()+'.png'
            textures[(ta,t['pathId'])]=(route,t)
            keys=[dict(time=k['time'],translation=native(k['translate']),emit=k['genType']!=0) for k in entry['scriptKeys']]
            colors=[]
            for n in range(5):
                color=a[f'colorAnimation[{n}]']['rgba']; colors.append([(color>>s&255)/255 for s in [0,8,16,24]])
            uv=r['UV Animation']
            emitters.append(dict(name=e['particleName'],generation=e['m_iGenType'],
                random_position=e['randomPosition'],random_angle=e['randomAngle'],random_velocity=e['randomVelocity'],
                generator_velocity=vec(e['initVelocity']),generator_plane=vec(e['plane']),
                script_keys=keys,generations_per_second=e['generationsPerSecond'],number_per_generation=e['numberPerGeneration'],
                lifetime=e['lifeTime'],initial_size=e['initialSize'],force=native(a['force']),colors=colors,
                animate_color=a['Does Animate Color?'],uv_tiles=[uv['x Tile'],uv['y Tile']],uv_cycles=uv['cycles'],
                width_curve=curve(r['m_WidthCurve']),height_curve=curve(r['m_HeightCurve']),rotation_curve=curve(r['m_RotationCurve']),
                render_mode=r['m_StretchParticles'],blend_mode=blend,material_tint=[tint[k] for k in 'rgba'],texture=route))
        plans.append(dict(id=eid,name=name,maximum_timer=c['maxTimer'],longest_lifetime=c['longestLifeTime'],
            disable_update=bool(c['disableUpdate']),emitters=emitters,
            mesh='effects/shinies/pickup/shineni_ai.glb' if eid==397 else None))
    for ai in [0,1]:
        selected=[dict(name=str(pid),serializedAsset=ASSETS[ai],pathId=pid) for asset,pid in textures if asset==ai]
        if not selected:continue
        req=w/f'textures-{ai}.json';write(req,selected);out=w/f'textures-{ai}'
        run('export-exact-texture-batch',args.source_root/['Effects.resourceFile','Tutorial.resourceFile'][ai],req,out)
        for row in selected:
            report=json.loads((out/(row['name']+'.exact.json')).read_text(encoding='utf-8'))
            route,t=textures[(ai,row['pathId'])]
            # Full source mip chains are preserved as ordinary editable PNGs.
            assert report['width']==t['value']['m_Width'] and report['height']==t['value']['m_Height']
            p=stage/route;p.parent.mkdir(parents=True,exist_ok=True)
            p.write_bytes(base64.b64decode(report['payload']['dataUrl'].split(',',1)[1]))
            assert sha(p)==report['payload']['sha256']
            
            paths=[]
            for level in report['mipLevels']:
                level_route=route if level['level']==0 else route.removesuffix('.png')+f".mip{level['level']}.png"
                (stage/level_route).write_bytes(base64.b64decode(level['payload']['dataUrl'].split(',',1)[1]))
                paths.append(level_route)
            for plan in plans:
                for emitter in plan['emitters']:
                    if emitter['texture']==route: emitter['texture']=paths
    run('export-logical-model-source',args.source_root/'Effects.resourceFile','prefabs/particle/effectscripts/es[397].prefab',w/'pickup.source.json',args.navigation)
    subprocess.run([str(ROOT/'target/debug/fusionforge.exe'),'native','publish-logical-model',str(w/'pickup.source.json'),'effect',str(w/'model')],check=True)
    model=w/'model/models/effect'; dst=stage/'effects/shinies/pickup';dst.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(model/'shineni_ai.glb',dst/'shineni_ai.glb')
    shutil.copytree(model/'shineni_ai.textures',dst/'shineni_ai.textures',dirs_exist_ok=True)
    write(stage/'effects/shinies/catalog.json',dict(schema='ffone.native-particle-effects.v1',effects=plans))
    req=w/'reward.json';write(req,[dict(name='coco',serializedAsset='sharedassets0.assets',pathId=207)])
    run('export-exact-texture-batch',args.source_root/'main.unity3d',req,w/'reward')
    art=json.loads((w/'reward/coco.exact.json').read_text(encoding='utf-8'))
    p=stage/'ui/gameplay/rewards/coco.png';p.parent.mkdir(parents=True,exist_ok=True)
    p.write_bytes(base64.b64decode(art['payload']['dataUrl'].split(',',1)[1]))
    raw=args.source_root/'main.unity3d'
    source.append(dict(alias='primary',container='main.unity3d',bytes=raw.stat().st_size,sha256=sha(raw)))
    outputs=[dict(path=p.relative_to(stage).as_posix(),bytes=p.stat().st_size,sha256=sha(p)) for p in sorted(stage.rglob('*')) if p.is_file()]
    write(w/'outputs.json',dict(sources=source,outputs=outputs,reward_size=[art['width'],art['height']]))
    if args.target_root:
        for o in outputs:
            p=args.target_root/o['path'];p.parent.mkdir(parents=True,exist_ok=True)
            temp=p.with_name(p.name+'.installing');shutil.copyfile(stage/o['path'],temp);temp.replace(p)

if __name__=='__main__':main()
