"""Record the verified NPC repair as a canonical Editor-owned publication receipt."""
import hashlib,json
from pathlib import Path
def read(p):return json.loads(p.read_bytes())
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def write(p,d):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(d,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
def main():
 w=Path('work/legacy-sources/npc-material-followup-20260905');native=Path('../FFOneClient/assets/game');record=Path('recipes/native/characters/npc-material-followup-20260905.receipt.json');assert not record.exists(),'accepted receipts are append-only'
 verified=read(w/'verification.json');assert verified['nativeOutputs']==verified['replayedOutputs']==67
 plan=Path('recipes/native/characters/npc-material-followup-20260905.plan.json');write(plan,read(w/'plan.json'))
 eyeplan=Path('recipes/native/characters/npc-material-followup-20260905.eyes-plan.json');write(eyeplan,read(w/'eyes-source-plan.json'))
 sources=Path('recipes/native/characters/npc-material-followup-20260905.sources.json');s=read(sources);primary=next(m for m in s['models'] if m['route']=='mob/npc_kimchi.kfm');objects=[dict(serializedAsset=x['assetName'],type=x['objectType'],pathId=x['pathId'],fileId=0,containerRoute=x['exactRoute'],trueName='npc_kimchi') for x in primary['targets']]
 paths={e['path'] for f in ['publication-applied.json','eyes-publication-applied.json'] for e in read(w/f)['outputs']};outputs=[dict(path=p,bytes=(native/p).stat().st_size,sha256=sha(native/p)) for p in sorted(paths)]
 input_paths=[plan,eyeplan,sources,Path('recipes/native/characters/npc-repair-20260905.receipt.json')]
 for m in s['models']:input_paths.append(Path(m['input']['path']))
 input_paths += list((w/'recovery/textures').glob('*/exact.json'))+list((w/'eyes-source-recovery/textures').glob('*/exact.json'))
 for e in read(w/'stage-icons.json'):
  if e['path'] in paths:input_paths.append(Path(e['source']))
 tools=['recover-npc-assets.py','stage-npc-material-followup.py','stage-fusion-kimchi-eyes.py','publish-npc-material-followup.py','audit-npc-material-followup.py','record-npc-material-followup.py','probe-npc-repair.py']
 input_paths += [Path('tools/legacy-sources')/p for p in tools]
 input_paths += [Path('crates/ffone-asset-pipeline/src/legacy_shader_state.rs'),Path('crates/ffone-asset-pipeline/src/logical_model_publish.rs')]
 inputs=[dict(path=p.as_posix(),bytes=p.stat().st_size,sha256=sha(p)) for p in input_paths]
 tool_revision=hashlib.sha256(''.join(x['sha256'] for x in inputs if x['path'].startswith('tools/')).encode()).hexdigest()
 base=' --work work/legacy-sources/npc-material-followup-20260905 --stage work/legacy-sources/npc-material-followup-20260905/stage/native'
 receipt=dict(schema='fusionforge.native-publication.v1',id='npc-material-followup-20260905',status='accepted',supersedes=[],target=dict(alias='native-game-assets'),source={**primary['source'],'objects':objects},transform=dict(kind='scoped-native-npc-material-and-icon-repair',tool='tools/legacy-sources/stage-npc-material-followup.py + stage-fusion-kimchi-eyes.py',toolRevision='sha256:'+tool_revision,workingDirectory='.',commands=[
  'python -X utf8 tools/legacy-sources/recover-npc-assets.py --plan '+plan.as_posix()+' --work work/legacy-sources/npc-material-followup-20260905/recovery --lane models',
  'python -X utf8 tools/legacy-sources/recover-npc-assets.py --plan '+plan.as_posix()+' --work work/legacy-sources/npc-material-followup-20260905/recovery --lane textures',
  'python -X utf8 tools/legacy-sources/recover-npc-assets.py --plan '+eyeplan.as_posix()+' --work work/legacy-sources/npc-material-followup-20260905/eyes-source-recovery --lane textures',
  'python -X utf8 tools/legacy-sources/stage-npc-material-followup.py'+base,
  'python -X utf8 tools/legacy-sources/stage-fusion-kimchi-eyes.py'+base,
  'python -X utf8 tools/legacy-sources/audit-npc-material-followup.py --work work/legacy-sources/npc-material-followup-20260905 --native ../FFOneClient/assets/game'
 ]),inputs=inputs,outputs=outputs,divergenceFromPrimary=dict(reason='Owner-requested appearance exceptions and repair of the previously imported Fusion Kimchi green variant.',details=['Kevin 2226/3380 retain previous Retrobution appearance; protected Ben/Gwen/Kevin/Albedo/Grandpa Max icon identities are unchanged.','Fusion Kimchi uses a native appearance fork: 58 of the existing 660 body-mesh triangles retain the exact primary vehicle_Kimchi atlas; the other 602 retain the existing green XDT texture. Vertex, normal, UV, skin, animation and source index data are unchanged; one disjoint material draw is added.','The native character comic lighting remains the independently authored runtime style. This receipt verifies bindings and source material contracts; it does not claim a pixel-identical Unity renderer.']),verification=[
 dict(command='python -X utf8 tools/legacy-sources/audit-npc-material-followup.py --work work/legacy-sources/npc-material-followup-20260905 --native ../FFOneClient/assets/game',result='passed: 67 published/replayed outputs, 500 decoded icons, 3490 NPC rows, 537 referenced icon paths, protected identities unchanged',evidence=(w/'verification.json').as_posix()),
 dict(command='cargo test -p ffone-client --lib production_ -- --skip production_ui_updates_text_only_through_the_localization_apply_system --skip production_npc_rows_have_complete_native_game_icon_effect_coverage',result='passed: 55 tests, 1 ignored',evidence='../FFOneClient/target/npc-material-production-tests-final.log'),
 dict(command='cargo test -p ffone-client --lib production_',result='broader initial run exposed independent equipment-localization source-scanner failure and missing overhead effect ES670 on five previously added Sweeper rows; these two checks remain unresolved and were explicitly excluded from the final focused run',evidence='../FFOneClient/target/npc-material-production-tests.log'),
 dict(command='cargo test -p ffone-asset-pipeline --lib legacy_shader_state',result='passed: 23 tests, 1 ignored',evidence=(w/'pipeline-tests.log').as_posix()),
 dict(command='cargo build -p ffone-client --bin ffone-client --bin ffone-editor --example logical_model_gpu_preview',result='passed',evidence='../FFOneClient/target/npc-material-build.log'),
 dict(command='python -X utf8 tools/legacy-sources/probe-npc-repair.py --assets ../FFOneClient/assets/game --out work/legacy-sources/npc-material-followup-20260905/installed --ids 252 981 2854 3256 3414 701 728 3380 3460 --camera-view reverse',result='passed; front/reverse images visually inspected, plus dedicated front view of restored Kimchi eyes',evidence=(w/'installed').as_posix()),
 dict(command='ffone-editor --asset-root ../FFOneClient/assets/game --npc <701|728|3414|3460> --capture <output.png>',result='passed: four actual editor captures ready, no material errors; images inspected',evidence=(w/'editor-captures').as_posix()),
 dict(command='python -X utf8 tools/legacy-sources/sync-openfusion-xdt.py ../OpenFusion/{tdata,bin/tdata} --table-set ../FFOneClient/assets/game/data/tables/table-set.json --report <report.json> --apply',result='passed in both directories: only row 3380 mesh changed, 49230 references checked, zero broken',evidence=(w/'server-bin-tdata.json').as_posix()),
 dict(command='node tools/audit-render-duplicates.mjs',result='completed read-only; no FPS improvement inferred',evidence='../FFOneClient/target/performance/render-duplicates.json')])
 write(record,receipt);print(record,len(outputs))
if __name__=='__main__':main()
