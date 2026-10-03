"""Recover primary Nano status particles and meshes into native skill effects."""
import argparse, base64, copy, hashlib, json, re, shutil, subprocess, struct, os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DOMAIN = 'skills'
ASSETS = ['CustomAssetBundle-fa9dbcf4604f64024b06ff1d5e375918',
          'CustomAssetBundle-b4f543c102ded400fbc6f1da25d9679a']
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def write(p, data):
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(data, indent=2)+'\n', encoding='utf-8')

def material_curve_surface_name(doc, node_index):
    """Keep the semantic curve binding on Bevy's primitive renderer name too."""
    node = doc['nodes'][node_index]
    name = node.get('name', 'surface')
    if 'mesh' not in node:
        return name  # A source material curve on a non-renderer is a no-op.
    if not name.endswith(f'-surface-{node_index}'):
        name += f'-surface-{node_index}'
    node['name'] = name
    mesh_index = node['mesh']
    if sum(n.get('mesh') == mesh_index for n in doc['nodes']) > 1:
        # Preserve each placement's material-animation owner. Geometry,
        # accessors, materials and primitive ordering remain unchanged.
        doc['meshes'].append(copy.deepcopy(doc['meshes'][mesh_index]))
        mesh_index = len(doc['meshes']) - 1
        node['mesh'] = mesh_index
    doc['meshes'][mesh_index]['name'] = name
    return name

def finalize_native_models(stage, plans):
    props={}
    for source,prefix in [('_Color','Base'),('_AmbColor','Ambient'),('_Emission','Emission')]:
        for suffix,color in zip('rgba',['Red','Green','Blue','Alpha']): props[source+'.'+suffix]=prefix+color
    props.update({'_MainTex.scale.x':'UvScaleX','_MainTex.scale.y':'UvScaleY','_MainTex.offset.x':'UvOffsetX','_MainTex.offset.y':'UvOffsetY','_MainTex.rotation':'UvRotationDegrees'})
    textures=stage/f'effects/{DOMAIN}/textures';textures.mkdir(parents=True,exist_ok=True)
    admitted={}
    for plan in plans:
        if not plan['mesh']: continue
        p=stage/plan['mesh'];b=p.read_bytes();length=struct.unpack_from('<I',b,12)[0]
        doc=json.loads(b[20:20+length]);curves=[];duration=0.0
        animations=[a.get('extras',{}) for a in doc.get('animations',[])]+doc.get('extras',{}).get('ffone',{}).get('metadataOnlyAnimations',[])
        for animation in animations:
            duration=max(duration,animation.get('duration',0))
            for c in animation.get('metadata',animation.get('nonTrs',{})).get('floatCurves',[]):
                assert c['classId']==21 and c['script']==dict(fileId=0,pathId=0)
                if c['property'].startswith('_SpecColor.'): continue
                prop=props[c['property']]
                # Material curves own exact renderer nodes, including repeated Plane names.
                name=material_curve_surface_name(doc,c['targetNode'])
                keys=[list(k) for k in zip(c['times'],c['values'],c['inTangents'],c['outTangents'])]
                curves.append(dict(node_name=name,property=prop,keys=keys))
        if curves: plan['material_animation']=dict(duration=duration,curves=curves)
        replacements={}
        def collect(x):
            if isinstance(x,dict):
                for k,v in x.items():
                    if k=='uri' and isinstance(v,str) and v.endswith('.png'): replacements[v]=None
                    else: collect(v)
            elif isinstance(x,list):
                for v in x:collect(v)
        collect(doc)
        for uri in [u for u in replacements if '.mips/' not in u]:
            source=p.parent/uri
            mip_prefix=uri.removesuffix('.png')+'.mips/'
            chain=[uri]+sorted(u for u in replacements if u.startswith(mip_prefix))
            # Compare the complete chain and binding contract before reusing it.
            # The strict native loader derives mip URIs from the base URI.
            bindings=[b for m in doc.get('materials',[]) for b in m.get('extras',{}).get('ffone',{}).get('textureBindings',[]) if b.get('uri')==uri]
            signature=json.dumps(([sha(p.parent/u) for u in chain],bindings),sort_keys=True)
            digest=hashlib.sha256(signature.encode()).hexdigest()
            dest=admitted.get(signature)
            if dest is None:
                dest=textures/(source.stem+'-'+digest[:12]+'.png')
                admitted[signature]=dest
                for old in chain:
                    target=dest if old==uri else dest.parent/(dest.stem+'.mips')/Path(old).name
                    target.parent.mkdir(parents=True,exist_ok=True)
                    shutil.copyfile(p.parent/old,target)
            for old in chain:
                target=dest if old==uri else dest.parent/(dest.stem+'.mips')/Path(old).name
                replacements[old]=os.path.relpath(target,p.parent).replace('\\','/')
        assert all(replacements.values()),replacements
        def clean(x):
            if isinstance(x,dict):
                if x.get('script')==dict(fileId=0,pathId=0): del x['script']
                for k,v in list(x.items()):
                    if k=='uri' and isinstance(v,str) and v in replacements:x[k]=replacements[v]
                    else:clean(v)
            elif isinstance(x,list):
                for v in x:clean(v)
        clean(doc)
        # Float animation now lives in the native skill catalog. Retain GLTF
        # transform channels and renderer indices; strip redundant source metadata.
        doc.get('extras',{}).get('ffone',{}).pop('metadataOnlyAnimations',None)
        for a in doc.get('animations',[]):
            a.get('extras',{}).pop('nonTrs',None)
        payload=json.dumps(doc,separators=(',',':'),ensure_ascii=False).encode();payload+=b' '*((-len(payload))%4)
        tail=b[20+length:];out=b'glTF'+struct.pack('<II',2,20+len(payload)+len(tail))+struct.pack('<II',len(payload),0x4e4f534a)+payload+tail
        p.write_bytes(out)
        for uri in replacements:
            old=p.parent/uri
            if old.is_file() and old.resolve().is_relative_to(stage.resolve()):old.unlink()
        assert not re.search(rb'CustomAssetBundle|pathId|fileId|retrobution|[A-Z]:\\\\',payload),p

def main():
    global DOMAIN
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--preset", choices=["nano-status", "npc-special", "skill-hits"], default="nano-status")
    parser.add_argument('--source-root',type=Path,required=True)
    parser.add_argument('--navigation',type=Path,required=True)
    parser.add_argument('--work',type=Path,required=True)
    parser.add_argument('--target-root',type=Path)
    args=parser.parse_args(); DOMAIN = {'npc-special':'npc-skills','skill-hits':'skill-hits'}.get(args.preset,'skills'); w=args.work; w.mkdir(parents=True,exist_ok=True)
    stage=w/'native'; stage.mkdir(exist_ok=True)
    previous=w/'outputs.json'
    if previous.is_file():
        for entry in json.loads(previous.read_text())['outputs']:
            old=(stage/entry['path']).resolve()
            assert old.is_relative_to(stage.resolve())
            if old.is_file():old.unlink()
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
        if not dump.is_file(): run('dump-object',raw,'all',dump)
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
                 'OneInvsrccolor':'OneOneMinusSrcColor','ZeroInvsrcalpha':'ZeroOneMinusSrcAlpha','InvsrccolorInvsrccolor':'OneMinusSrcColorOneMinusSrcColor','InvdestcolorOne':'OneMinusDstColorOne','InvsrccolorDestalpha':'OneMinusSrcColorDstAlpha'}
    selected_effects = [(368,11471,'damage-protection'),(429,9659,'jump-target'),(447,11076,'health-protection'),(431,10445,'passive-regeneration'),(411,9127,'protection-target'),(434,8998,'protection-target-alt'),(370,11510,'stun-target'),(714,8908,'jump-boost'),(436,8757,'run-boost'),(412,8901,'reward-boost'),(805,10069,'freedom'),(435,9489,'sleep-target'),(441,11468,'drain-state')]
    if args.preset == 'npc-special':
        selected_effects = [(530,11088,'corruption-a'),(531,11278,'corruption-b'),(532,9147,'corruption-c'),(592,10068,'fusion-corruption'),(766,9840,'eruption')]
    if args.preset == 'skill-hits':
        selected_effects = [(54,11547,'healing-tick'),(407,10369,'healing-impact'),
            (827,11455,'nano-healing-projectile'),(828,10143,'nano-debuff-projectile'),
            (438,10291,'debuff-impact'),(502,9362,'egg-damage-projectile'),(500,9245,'egg-damage-impact')]
    for eid,root,name in selected_effects:
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
            route=f'effects/{DOMAIN}/textures/'+t['name'].lower()+'.png'
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
            mesh=None))
    if args.preset == 'npc-special':
        textures[(0,349)] = ('effects/npc-skills/eruption-mark.png', objs[0][349])
    for ai in [0,1]:
        selected=[dict(name=str(pid),serializedAsset=ASSETS[ai],pathId=pid) for asset,pid in textures if asset==ai]
        if not selected:continue
        req=w/f'textures-{ai}.json';write(req,selected);out=w/f'textures-{ai}-{sha(req)[:12]}'
        if any(not (out/(row['name']+'.exact.json')).is_file() for row in selected): run('export-exact-texture-batch',args.source_root/['Effects.resourceFile','Tutorial.resourceFile'][ai],req,out)
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
    mesh_names={429:'jump_target_e02',411:'protection_target_e01',434:'protection_target_e02',714:'nano_rd_e01',412:'k_rewardcash_self1',805:'freedom_self',435:'sleep_target_e01'}
    if args.preset == 'npc-special':
        mesh_names = {766:'mega'}
    if args.preset == 'skill-hits': mesh_names = {}
    for eid,name in mesh_names.items():
        source_path=w/f'mesh-{eid}.source.json'
        run('export-logical-model-source',args.source_root/'Effects.resourceFile',f'prefabs/particle/effectscripts/es[{eid}].prefab',source_path,args.navigation)
        out=w/f'mesh-{eid}'
        if not (out/'models/effect'/(name+'.glb')).is_file():
            subprocess.run([str(ROOT/'target/debug/fusionforge.exe'),'native','publish-logical-model',str(source_path),'effect',str(out)],check=True)
        model=out/'models/effect'
        assert (model/(name+'.glb')).is_file(),list(model.iterdir())
        dst=stage/f'effects/{DOMAIN}'/name;dst.mkdir(parents=True,exist_ok=True)
        for p in model.rglob('*'):
            if p.is_file() and p.suffix in ('.glb','.png'):
                dest=dst/p.relative_to(model);dest.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,dest)
        next(p for p in plans if p['id']==eid)['mesh']=f'effects/{DOMAIN}/{name}/{name}.glb'
    finalize_native_models(stage,plans)
    write(stage/f'effects/{DOMAIN}/catalog.json',dict(schema='ffone.native-particle-effects.v1',effects=plans))
    if args.preset == 'skill-hits':
        rows=objs[0][14153]['value']['m_pBulletData']
        projectiles=[]
        for identifier in [88,92,97,105]:
            row=rows[identifier]
            assert row['m_iFireScript'] == -1 and row['m_iCancelScript'] == 0
            projectiles.append(dict(id=identifier, particle=row['m_iParticleScript'], impact=row['m_iSuccScript'],
                scale=row['m_fBulletModelScale'],impact_scale=row['m_fSuccModelScale'],
                hide_seconds=row['m_fHideTime'],duration_seconds=row['m_fMaxTimer'],
                source_link=row['m_strFireLink'].replace('"',''),target_link=row['m_strSuccLink'].replace('"',''),
                impact_sound='audio/sfx/combat/megaattackareahittype.ogg' if identifier==105 else None))
        write(stage/f'effects/{DOMAIN}/projectiles.json',dict(schema='ffone.native-skill-projectiles.v1',projectiles=projectiles))
    outputs=[dict(path=p.relative_to(stage).as_posix(),bytes=p.stat().st_size,sha256=sha(p)) for p in sorted(stage.rglob('*')) if p.is_file()]
    write(w/'outputs.json',dict(sources=source,outputs=outputs))
    if args.target_root:
        for o in outputs:
            p=args.target_root/o['path'];p.parent.mkdir(parents=True,exist_ok=True)
            temp=p.with_name(p.name+'.installing');shutil.copyfile(stage/o['path'],temp);temp.replace(p)

if __name__=='__main__':main()
