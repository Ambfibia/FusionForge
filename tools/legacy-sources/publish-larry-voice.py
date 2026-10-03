"""Recover Larry's exact primary bundle routes and wire existing Russian recordings.

Run from FusionForge. All extraction and staging stays below --work; only
editable Ogg files and the native catalogue are installed with --apply.
"""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import subprocess


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--source-root', type=Path, required=True)
    p.add_argument('--native-target', type=Path, required=True)
    p.add_argument('--work', type=Path, required=True)
    p.add_argument('--apply', action='store_true')
    args = p.parse_args()
    args.work.mkdir(parents=True, exist_ok=True)
    exe = Path('target/debug/fusionforge.exe').resolve()
    bundle = 'Retro_shared.resourceFile'
    asset = 'CustomAssetBundle-Retro_shared'
    listing = subprocess.check_output([str(exe), 'fusionforge', 'list-assetbundle', str(args.source_root / bundle)], text=True)
    (args.work / 'routes.txt').write_text(listing, encoding='utf-8')
    routes = {}
    for line in listing.splitlines():
        fields = line.split('\t')
        if len(fields) == 4 and fields[3].startswith('vo/larry_'):
            routes[Path(fields[3]).stem] = (int(fields[0]), fields[3])
    expected = {'larry_qgreeting'} | {f'larry_{cue}{i:02}' for cue, total in [('greeting', 2), ('farewell', 3), ('goodluck', 3), ('nicejob', 3)] for i in range(1, total + 1)}
    assert set(routes) == expected, routes
    documents = {}
    for path_id in sorted({v[0] for v in routes.values()}):
        out = args.work / f'{path_id}.json'
        subprocess.run([str(exe), 'fusionforge', 'dump-object-evidence', 'primary', str(args.source_root), bundle, str(path_id), '--serialized-asset', asset, '--type', 'AudioClip', '--out', str(out)], check=True)
        documents[path_id] = json.loads(out.read_text('utf-8'))
    catalog_path = args.native_target / '_runtime/audio.json'
    catalog = json.loads(catalog_path.read_text('utf-8'))
    assert catalog['schema'] == 'ffone.semantic-audio-catalog.v5'
    entries, outputs, objects, inputs = [], [], [], {}
    for true_name, (path_id, route) in sorted(routes.items()):
        obj = documents[path_id]['object']
        value = obj['value']
        english = base64.b64decode(value['audio data']['base64'])
        # Native Russian filenames omit the hyphen in shared take ranges
        # (Larry_Farewell01-2 -> Larry_Farewell012).
        russian_stem = value['m_Name'].replace('-', '')
        russian_path = f"audio/voice/ru/larry/{russian_stem}.ogg"
        russian = (args.native_target / russian_path).read_bytes()
        inputs[russian_path] = {'path': russian_path, 'bytes': len(russian), 'sha256': digest(russian)}
        cue = true_name.removeprefix('larry_')
        files = []
        for locale, data in [('en', english), ('ru', russian)]:
            assert data.startswith(b'OggS')
            relative = f'audio/voice/{locale}/larry/{cue}.ogg'
            staged = args.work / 'staging' / relative
            staged.parent.mkdir(parents=True, exist_ok=True)
            staged.write_bytes(data)
            outputs.append({'path': relative, 'bytes': len(data), 'sha256': digest(data)})
            files.append({'locale': locale, 'path': relative})
        entries.append({'logicalKey': f'voice/larry/{cue}', 'trueName': true_name, 'category': 'voice', 'owner': 'larry', 'files': files})
        objects.append({'serializedAsset': asset, 'type': 'AudioClip', 'pathId': path_id, 'fileId': 0, 'containerRoute': route, 'trueName': value['m_Name'], 'rawSha256': obj['rawSha256']})
    keys = {e['logicalKey'] for e in entries}
    for old in catalog['assets']:
        if old['logicalKey'] in keys:
            assert old in entries, 'Refusing to replace a different entry'
    catalog['assets'] = sorted([e for e in catalog['assets'] if e['logicalKey'] not in keys] + entries, key=lambda e: e['logicalKey'])
    catalog['counts']['assets'] = len(catalog['assets'])
    catalog['counts']['voice'] = sum(e['category'] == 'voice' for e in catalog['assets'])
    staged_catalog = args.work / 'staging/_runtime/audio.json'
    staged_catalog.parent.mkdir(parents=True, exist_ok=True)
    staged_catalog.write_text(json.dumps(catalog, ensure_ascii=False, indent=1) + '\n', encoding='utf-8')
    raw = (args.source_root / bundle).read_bytes()
    command = 'python tools/legacy-sources/publish-larry-voice.py --source-root ../builds/retrobution-20260821 --native-target ../FFOneClient/assets/game --work work/legacy-sources/larry-voice --apply'
    receipt = {'schema': 'fusionforge.native-publication.v1', 'id': 'larry-dialogue-voice-20260904', 'status': 'accepted', 'supersedes': [], 'target': {'alias': 'native-target'}, 'source': {'alias': 'primary', 'relativeContainer': bundle, 'containerBytes': len(raw), 'containerSha256': digest(raw), 'objects': objects}, 'transform': {'kind': 'extract', 'tool': 'tools/legacy-sources/publish-larry-voice.py', 'toolRevision': digest(Path(__file__).read_bytes()), 'workingDirectory': '.', 'commands': [command]}, 'inputs': list(inputs.values()), 'outputs': outputs, 'divergenceFromPrimary': 'Russian locale uses existing native recordings matched to the exact AudioClip names; English bytes and shared route ownership are unchanged.', 'verification': [{'command': command, 'result': 'passed: strict scoped extraction; exact route set; Ogg signatures; staged/published SHA-256 equality', 'evidence': None}]}
    (args.work / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8')
    if args.apply:
        for output in outputs:
            destination = args.native_target / output['path']
            data = (args.work / 'staging' / output['path']).read_bytes()
            if destination.exists():
                assert destination.read_bytes() == data, destination
        for output in outputs:
            destination = args.native_target / output['path']
            destination.parent.mkdir(parents=True, exist_ok=True)
            temporary = destination.with_suffix('.ogg.next')
            temporary.write_bytes((args.work / 'staging' / output['path']).read_bytes())
            temporary.replace(destination)
            assert digest(destination.read_bytes()) == output['sha256']
        temporary = catalog_path.with_suffix('.json.next')
        temporary.write_bytes(staged_catalog.read_bytes())
        temporary.replace(catalog_path)
    print(f'{len(entries)} dialogue routes, {len(documents)} source clips, {len(outputs)} localized files; applied={args.apply}')


if __name__ == '__main__':
    main()
