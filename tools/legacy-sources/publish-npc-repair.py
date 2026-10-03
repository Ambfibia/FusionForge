"""Publish only the reviewed NPC repair assets, with pinned preimages and backups."""
import argparse, json, hashlib, os, shutil
from pathlib import Path

def read(p):return json.loads(p.read_bytes())
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest() if p.is_file() else None
def put(p,b):
    p.parent.mkdir(parents=True,exist_ok=True);t=p.with_name(p.name+'.next');t.write_bytes(b);t.replace(p)
def write(p,d):put(p,(json.dumps(d,ensure_ascii=False,indent=2)+'\n').encode())

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--work',type=Path,required=True);ap.add_argument('--stage',type=Path,required=True);ap.add_argument('--native',type=Path,required=True);ap.add_argument('--apply',action='store_true');a=ap.parse_args()
    work=a.work.resolve();stage=a.stage.resolve();native=a.native.resolve()
    assert stage.is_relative_to(Path('work').resolve()) and native!=stage
    plan_path=work/'publication-plan.json'
    if not a.apply:
        # Keep concurrently edited UI translations; this repair only adds content keys.
        for lang in ['en','ru']:
            p=f'localization/{lang}.json';current=read(native/p);new=read(stage/p)
            for key,value in new['entries'].items():
                if key not in current['entries']:
                    assert key.startswith('content.'),key
                    current['entries'][key]=value
            write(stage/p,current)
        current=read(native/'data/tables/table-set.json');staged=read(stage/'data/tables/table-set.json')
        table=next(t for t in current['tables'] if 'm_pNpcTable' in t['value'])
        new=next(t for t in staged['tables'] if 'm_pNpcTable' in t['value'])
        table['value']['m_pNpcTable']=new['value']['m_pNpcTable'];write(stage/'data/tables/table-set.json',current)
        paths=set(['_runtime/characters.json','characters/player/items/catalog.json','data/hnpc/catalog.json','data/tables/table-set.json','data/tables/npc_texture_overrides.json','localization/en.json','localization/ru.json'])
        for entry in read(work/'native-texture-closure.json'):
            model=stage/entry['path'];directory=model.parent
            if entry['path'].startswith('characters/player/items/'):
                directory=directory.parent.parent
            paths.update(p.relative_to(stage).as_posix() for p in directory.rglob('*') if p.is_file())
        for folder in ['icons-exact','retro-icons-exact','icons-repaired-exact']:
            for p in (work/folder).glob('*.exact.json'):
                name=p.name.removesuffix('.exact.json');prefix,number=name.rsplit('_',1);family={'npcicon':'npc','mobicon':'mobs','hnpcicon':'hnpc'}[prefix]
                paths.add(f'icons/entities/{family}/{prefix}_{int(number):02}.png')
        for p in (stage/'textures/shared').rglob('*.png'):
            rel=p.relative_to(stage).as_posix()
            if not (native/rel).exists():paths.add(rel)
        for texture in read(stage/'data/tables/npc_texture_overrides.json')['textures']:
            paths.add(texture['path'])
        for texture in read(stage/'data/hnpc/catalog.json')['textures']:
            paths.add(texture['path'])
        outputs=[]
        for path in sorted(paths):
            assert (stage/path).is_file(),path
            before=sha(native/path);after=sha(stage/path)
            if before!=after:outputs.append(dict(path=path,beforeSha256=before,sha256=after,bytes=(stage/path).stat().st_size))
        write(plan_path,dict(schema='fusionforge.npc-repair-publication.v1',outputs=outputs))
        print('reviewed outputs',len(outputs),'bytes',sum(x['bytes'] for x in outputs));return
    plan=read(plan_path)
    for e in plan['outputs']:
        assert (native/e['path']).resolve().is_relative_to(native)
        assert sha(native/e['path'])==e['beforeSha256'],('changed destination',e['path'])
        assert sha(stage/e['path'])==e['sha256'],('changed staged output',e['path'])
    for e in plan['outputs']:
        p=native/e['path']
        if p.exists():
            backup=work/'publication-backup'/e['path'];backup.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,backup)
        put(p,(stage/e['path']).read_bytes())
        assert sha(p)==e['sha256']
    write(work/'publication-applied.json',dict(schema=plan['schema'],verifiedOutputs=len(plan['outputs']),outputs=plan['outputs']))
    print('published and verified',len(plan['outputs']))
if __name__=='__main__':main()
