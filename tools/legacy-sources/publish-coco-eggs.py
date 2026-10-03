"""Replay the primary shiny models into staging and optionally install verified outputs."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-root', type=Path, required=True)
    parser.add_argument('--navigation', type=Path, required=True)
    parser.add_argument('--work', type=Path, required=True)
    parser.add_argument('--target-root', type=Path)
    parser.add_argument('--receipt', type=Path, required=True)
    args = parser.parse_args()
    receipt = json.loads(args.receipt.read_text(encoding='utf-8'))
    raw = args.source_root / 'Tutorial.resourceFile'
    assert sha(raw) == receipt['source']['containerSha256'], 'source hash changed'
    args.work.mkdir(parents=True, exist_ok=True)
    forge = Path('target/debug/fusionforge.exe').resolve()
    pipeline = Path('target/debug/fusionforge.exe').resolve()
    staged = {}
    for name in ['shineni_Item', 'shineni_buff']:
        source = args.work / (name + '.source.json')
        candidate = args.work / name
        subprocess.run([str(forge), 'export-logical-model-source', str(raw),
                        'mob/' + name.lower() + '.kfm', str(source), str(args.navigation)], check=True)
        subprocess.run([str(pipeline), 'native', 'publish-logical-model', str(source), 'npc', str(candidate)], check=True)
        folder = candidate / 'models/npc'
        staged['characters/shinies/' + name + '/' + name + '.glb'] = folder / (name + '.glb')
        for texture in (folder / (name + '.textures')).rglob('*.png'):
            relative = texture.relative_to(folder).as_posix()
            staged['characters/shinies/' + name + '/' + relative] = texture
    expected = {entry['path']: entry for entry in receipt['outputs']}
    assert set(staged) == set(expected), 'output set changed'
    for relative, path in staged.items():
        assert sha(path) == expected[relative]['sha256'], relative
        assert path.stat().st_size == expected[relative]['bytes'], relative
    if args.target_root:
        assert receipt['status'] == 'accepted', 'receipt is not accepted'
        for relative, source in staged.items():
            target = args.target_root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            temporary = target.with_name(target.name + '.installing')
            shutil.copyfile(source, temporary)
            temporary.replace(target)
        # Insert only the new semantic routes; preserve every unrelated table byte.
        table_path = args.target_root / 'data/tables/table-set.json'
        text = table_path.read_bytes().decode('utf-8')
        document = json.loads(text)
        routes = next(t['value']['m_pCharacterModelData'] for t in document['tables']
                      if t['name'] == 'native_asset_routes')
        additions = []
        for name in ['shineni_Item', 'shineni_buff']:
            row = dict(id='shiny/' + name.lower(), logicalName=name, category='shiny',
                       glb='characters/shinies/' + name + '/' + name + '.glb', animations=['stand1'])
            existing = [r for r in routes if r['logicalName'] == name]
            if existing:
                assert existing == [row], 'conflicting native route'
            else:
                additions.append(row)
        if additions:
            newline = '\r\n' if '\r\n' in text else '\n'
            marker = '"m_pCharacterModelData": ['
            assert text.count(marker) == 1
            insertion = (',' + newline).join(
                newline.join('          ' + line for line in json.dumps(row, indent=2).splitlines())
                for row in additions)
            text = text.replace(marker, marker + newline + insertion + ',', 1)
            json.loads(text)
            table_path.write_bytes(text.encode('utf-8'))
    print('Verified', len(staged), 'native outputs; installed:', bool(args.target_root))


if __name__ == '__main__':
    main()
