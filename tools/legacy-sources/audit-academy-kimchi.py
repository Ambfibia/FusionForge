"""Verify the scoped Academy/original Fusion Kimchi publication and append its receipt."""
import argparse, copy, importlib.util, json
from pathlib import Path
s=importlib.util.spec_from_file_location('p',Path(__file__).with_name('publish-academy-kimchi.py'));p=importlib.util.module_from_spec(s);s.loader.exec_module(p)
h=p.h;c=p.c

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--work',type=Path,required=True);ap.add_argument('--native-root',type=Path,required=True);ap.add_argument('--server-root',type=Path,required=True);ap.add_argument('--record',action='store_true');a=ap.parse_args()
    w=a.work;n=a.native_root;plan=h.read(w/'academy-publication-plan.json');installed=h.read(w/'academy-installation.json');old_path=Path('recipes/native/characters/fusion-kimchi-fusion-eyes-20260906.receipt.json');old=h.read(old_path)
    for e in installed['outputs']:
        assert p.item(n,e['path'])==e
        assert (n/e['path']).read_bytes()==(w/'replay-native'/e['path']).read_bytes()
    carried=[e for e in old['outputs'] if e['path'] not in plan['outputs']]
    for e in carried:assert p.item(n,e['path'])==e,e['path']
    before=w/'accepted-before'
    tables=h.read(before/p.DOCS[1]);p.npc(tables)['m_pNpcMeshData'][plan['meshIndex']]['m_pstrMTextureString']='fusion_kimchi'
    assert tables==h.read(n/p.DOCS[1])
    catalog=h.read(before/p.DOCS[0]);next(m for m in catalog['models'] if m['id']=='npc/fusion_kimchi')['glbBlake3']=c.b3((n/p.REL).read_bytes())
    assert catalog==h.read(n/p.DOCS[0])
    textures=h.read(n/p.DOCS[2]);textures['textures']=[t for t in textures['textures'] if t['trueName']!='fusion_kimchi'];assert textures==h.read(before/p.DOCS[2])
    for name in ['tdata/xdt.json','bin/tdata/xdt.json']:
        expected=h.read(w/'server-before'/name);expected['m_pNpcTable']['m_pNpcMeshData'][plan['meshIndex']]['m_pstrMTextureString']='fusion_kimchi';assert expected==h.read(a.server_root/name)
    src,b=h.glb(w/'strict-recovery/candidate/models/npc/npc_kimchi.glb');out,ob=h.glb(n/p.REL);assert b==ob
    assert src['meshes']==out['meshes'] and src['animations']==out['animations'] and src['skins']==out['skins']
    nodes=copy.deepcopy(src['nodes']);nodes[0]['name']='fusion_kimchi';assert nodes==out['nodes']
    for binding in c.bindings(out):
        for mip in binding['mipLevels']:assert p.digest((n/p.REL).parent/mip['uri'])==mip['pngSha256']
    for forbidden in ['pathId','fileId','CustomAssetBundle','D:/','D:\\\\']:
        assert forbidden not in json.dumps(out),forbidden
    model_root=Path('../builds/6543a2bb-d154-4087-b9ee-3c8aa778580a');recovery=h.read(w/'strict-recovery/models/academy_kimchi/recovery.json');raw_model=model_root/recovery['source']['relativeContainer'];assert p.digest(raw_model)==recovery['source']['sha256']
    texture_evidence=h.read(w/'fusion-kimchi-texture.evidence.json');raw_texture=w/'cdn/beta-20101123/NpcTexture.resourceFile';assert p.digest(raw_texture)==texture_evidence['source']['sha256']
    assert texture_evidence['triage']['unresolvedPointerCount']==0
    editor=h.read(w/'editor-correct.json');assert editor['ready'] and editor['error'] is None
    report=dict(status='passed',replayByteExact=True,unchangedPreviousOutputs=len(carried),nativeTableChangedCells=1,serverChangedCellsEach=1,sourceGeometryAndAnimationsUnchanged=True,sourceEyesRetained=True,sharedMipChainsVerified=True)
    h.write(w/'accepted-verification.json',report)
    if not a.record:print(json.dumps(report));return
    def log(name):
        b=(w/name).read_bytes();return b.decode('utf-16' if b.startswith(b'\xff\xfe') else 'utf-8')
    assert '1 passed; 0 failed' in log('native-test.log') and 'Finished' in log('native-build.log')
    assert '18 passed; 0 failed' in log('packed-pointer-suite.log')
    record=Path('recipes/native/characters/fusion-kimchi-academy-original-atlas-20260906.receipt.json');assert not record.exists()
    tools=[Path('tools/legacy-sources')/f for f in ['publish-academy-kimchi.py','audit-academy-kimchi.py','recover-npc-assets.py','close-npc-native-textures.py','stage-fusion-kimchi-eyes.py']]
    inputs=[raw_model,model_root/'TrainingGrounds.resourceFile',raw_texture,w/'plan.json',w/'fusion-kimchi-texture.json',w/'fusion-kimchi-texture.evidence.json',w/'academy-kimchi.source.json',w/'strict-recovery/candidate/models/npc/npc_kimchi.glb',*tools,Path('src/fusionforge/unity.rs'),Path('src/lib.rs')]
    def entry(path):return dict(path=path.as_posix(),bytes=path.stat().st_size,sha256=p.digest(path))
    receipt=dict(schema='fusionforge.native-publication.v1',id='fusion-kimchi-academy-original-atlas-20260906',status='accepted',supersedes=[old['id']],target=dict(alias='native-game-assets'),
      source=dict(alias='alternate',relativeContainer=recovery['source']['relativeContainer'],containerBytes=recovery['source']['bytes'],containerSha256=recovery['source']['sha256'],objects=[dict(serializedAsset=t['assetName'],type=t['objectType'],pathId=t['pathId'],containerRoute=t['exactRoute']) for t in recovery['targets']]),
      additionalSources=[dict(alias='alternate',version='beta-20101123',downloadUrl='https://cdn.dexlabs.systems/ff/big/beta-20101123/NpcTexture.resourceFile',relativeContainer='NpcTexture.resourceFile',containerBytes=raw_texture.stat().st_size,containerSha256=p.digest(raw_texture),objects=[dict(serializedAsset='CustomAssetBundle-NpcTexture',type='Texture2D',pathId=1238925593,containerRoute='texture/fusion_kimchi.dds')])],
      inputs=[entry(f) for f in inputs],outputs=[*installed['outputs'],*carried],serverOutputs=installed['server'],
      transform=dict(kind='academy-model-with-original-fusion-kimchi-texture',tool=tools[0].as_posix(),toolRevision='sha256:'+p.digest(tools[0]),workingDirectory='.',commands=[
        'python -X utf8 tools/legacy-sources/recover-npc-assets.py --plan '+(w/'plan.json').as_posix()+' --work '+(w/'strict-recovery').as_posix()+' --lane models',
        'target/debug/fusionforge.exe fusionforge export-exact-texture '+raw_texture.as_posix()+' 1238925593 '+(w/'fusion-kimchi-texture.json').as_posix(),
        'python -X utf8 tools/legacy-sources/publish-academy-kimchi.py --work '+w.as_posix()+' --native-root ../FFOneClient/assets/game --stage work/legacy-sources/fusion-kimchi-eye-geometry-20260906/stage/native --apply --server-root ../OpenFusion']),
      divergenceFromPrimary=dict(reason='Owner explicitly requests Academy Kimchi with the original oval red Fusion eyes.',details=['Geometry, skin, UVs, materials and animations come from the specified Academy build. The native root name changes to preserve the separate Fusion Kimchi identity.','Academy XDT selects fusion_kimchi, but its texture is recovered from original beta-20101123 NpcTexture; the Academy NPC texture bundles do not contain it. Exact DXT1 pixels, one source level and repeat/linear sampler are retained.','No borrowed Fusion eye cards, no projected spwaneye, no removed original eye triangles, and no painted replacement. ToonRamp9 reuses an existing fully equal shared chain.']),
      verification=[dict(command='cargo test --lib fusionforge::unity::tests',result='18 passed; packed external ownership and complete object tag checked',evidence=(w/'packed-pointer-suite.log').as_posix()),dict(command='cargo test -p ffone-client --lib production_fusion_kimchi_uses_original_atlas_and_complete_kimchi_mesh',result='passed',evidence=(w/'native-test.log').as_posix()),dict(command='cargo build -p ffone-client --bin ffone-client --bin ffone-editor',result='passed',evidence=(w/'native-build.log').as_posix()),dict(command='ffone-editor --npc 3460 --capture <editor-correct.png>',result='ready, no error; oval red eyes and dark upper lids visually verified',evidence=(w/'editor-correct.json').as_posix()),dict(command='python -X utf8 tools/legacy-sources/audit-academy-kimchi.py --work '+w.as_posix()+' --native-root ../FFOneClient/assets/game --server-root ../OpenFusion',result=report,evidence=(w/'accepted-verification.json').as_posix())])
    h.write(record,receipt);print(record)

if __name__=='__main__':main()
