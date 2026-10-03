"""Recover the audited primary NPC dialogue routes, with scoped raw evidence.

The plan pins source bytes and routes. Existing Russian recordings are matched
by exact clip identity, or by byte-identical English catalogue counterparts.
Missing translations retain the native catalogue's English fallback.
"""
import argparse
import base64
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import subprocess


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-root', type=Path, required=True)
    parser.add_argument('--native-target', type=Path, required=True)
    parser.add_argument('--work', type=Path, required=True)
    parser.add_argument('--apply', action='store_true')
    args = parser.parse_args()
    plan_path = Path('recipes/native/audio/npc-dialogue-gaps-20260904.plan.json')
    plan = json.loads(plan_path.read_text('utf-8'))
    raw = (args.source_root / plan['container']).read_bytes()
    assert sha(raw) == plan['sha256'], 'Source changed'
    args.work.mkdir(parents=True, exist_ok=True)
    exe = str(Path('target/debug/fusionforge.exe').resolve())
    evidence_hashes = {e['pathId']: e['evidenceSha256'] for e in plan['entries']}

    def extract(key):
        path_id, kind = key
        path = args.work / f'{path_id}.json'
        if not path.exists() or path_id not in evidence_hashes:
            subprocess.run([exe, 'fusionforge', 'dump-object-evidence', 'primary', str(args.source_root), plan['container'], str(path_id), '--serialized-asset', plan['serializedAsset'], '--type', kind, '--out', str(path)], check=True, stdout=subprocess.DEVNULL)
        if path_id in evidence_hashes:
            assert sha(path.read_bytes()) == evidence_hashes[path_id], ('Evidence changed', path)
        doc = json.loads(path.read_text('utf-8'))
        assert doc['source']['sha256'] == plan['sha256']
        assert doc['object']['serializedAsset'] == plan['serializedAsset']
        assert doc['object']['pathId'] == path_id and doc['object']['type'] == kind
        assert doc['triage']['unresolvedPointerCount'] == 0
        return path_id, doc

    keys = sorted({(e['pathId'], e['type']) for e in plan['entries']})
    with ThreadPoolExecutor(max_workers=3) as pool:
        documents = dict(pool.map(extract, keys))
    print(f'Validated {len(documents)} source objects', flush=True)
    # AudioSource entries need explicit pointer resolution; never decode them
    # as clips or silently substitute a similarly named recording.
    for entry in plan['entries']:
        if entry['type'] == 'AudioSource':
            doc = documents[entry['pathId']]
            # These primary bundle objects are labelled class 82 but carry
            # an AudioClip-shaped body including embedded Vorbis data. Keep
            # the original declared type in evidence and validate the bytes.
            if doc['object']['value'].get('audio data', {}).get('base64'):
                continue
            pointer = doc['object']['value']['m_audioClip']
            assert pointer['fileID'] == 0, 'External clip requires a separate source plan'
            clip_id = pointer['pathID']
            if clip_id not in documents:
                _, documents[clip_id] = extract((clip_id, 'AudioClip'))
            entry['clipId'] = clip_id
    catalog_path = args.native_target / '_runtime/audio.json'
    before = catalog_path.read_bytes()
    catalog = json.loads(before)
    assert catalog['schema'] == 'ffone.semantic-audio-catalog.v5'
    translated_by_hash = {}
    translated_by_name = {}
    for old in catalog['assets']:
        files = {f.get('locale'): f['path'] for f in old['files']}
        if 'en' in files and 'ru' in files:
            english_hash = sha((args.native_target / files['en']).read_bytes())
            translated_by_hash.setdefault(english_hash, set()).add(files['ru'])
            translated_by_name.setdefault((old['trueName'].lower(), english_hash), set()).add(files['ru'])
    ru_files = list((args.native_target / 'audio/voice/ru').rglob('*.ogg'))
    outputs, additions, inputs, unresolved = [], [], {}, []
    for entry in plan['entries']:
        obj = documents[entry.get('clipId', entry['pathId'])]['object']
        value = obj['value']
        payload = value.get('audio data', {}).get('base64')
        if not payload:
            unresolved.append({'entry': entry, 'reason': 'AudioClip has no embedded payload'})
            continue
        data = base64.b64decode(payload)
        assert data.startswith(b'OggS'), entry
        name = value.get('m_Name', '')
        exact_ru = {str(f.relative_to(args.native_target)).replace('\\', '/') for f in ru_files if f.stem.lower() == name.lower() and name}
        # A source recording can have distinct localized adaptations for
        # dialogue and the communicator. Prefer the exact semantic name;
        # byte equality alone cannot choose between those adaptations.
        ru_candidates = exact_ru | translated_by_name.get((name.lower(), sha(data)), set())
        if not ru_candidates:
            ru_candidates = translated_by_hash.get(sha(data), set())
        ru_hashes = {sha((args.native_target / p).read_bytes()) for p in ru_candidates}
        assert len(ru_hashes) <= 1, ('Ambiguous Russian recordings', entry, ru_candidates)
        owner = entry['owner']
        cue = entry['trueName'][len(owner)+1:]
        files = []
        locales = [('en', data)]
        if ru_candidates:
            ru = sorted(ru_candidates)[0]
            ru_data = (args.native_target / ru).read_bytes()
            inputs[ru] = {'path': ru, 'bytes': len(ru_data), 'sha256': sha(ru_data)}
            locales.append(('ru', ru_data))
        for locale, payload_bytes in locales:
            path = f'audio/voice/{locale}/{owner}/{cue}.ogg'
            staged = args.work / 'staging' / path
            staged.parent.mkdir(parents=True, exist_ok=True)
            staged.write_bytes(payload_bytes)
            outputs.append({'path': path, 'bytes': len(payload_bytes), 'sha256': sha(payload_bytes)})
            files.append({'locale': locale, 'path': path})
        additions.append({'logicalKey': f'voice/{owner}/{cue}', 'trueName': entry['trueName'], 'category': 'voice', 'owner': owner, 'files': files})
    (args.work / 'unresolved.json').write_text(json.dumps(unresolved, indent=2), encoding='utf-8')
    assert not unresolved, unresolved
    names = {a['trueName'] for a in additions}
    for old in catalog['assets']:
        if old['trueName'].lower() in names:
            assert old in additions, ('Conflicting existing entry', old)
    catalog['assets'] = sorted([a for a in catalog['assets'] if a['trueName'].lower() not in names] + additions, key=lambda a: a['logicalKey'])
    catalog['counts']['assets'] = len(catalog['assets'])
    catalog['counts']['voice'] = sum(a['category'] == 'voice' for a in catalog['assets'])
    catalog_bytes = (json.dumps(catalog, ensure_ascii=False, indent=1)+'\n').encode('utf-8')
    staged_catalog = args.work / 'staging/_runtime/audio.json'
    staged_catalog.parent.mkdir(parents=True, exist_ok=True)
    staged_catalog.write_bytes(catalog_bytes)
    report = {'source': {k: plan[k] for k in ['source','container','serializedAsset','sha256']}, 'objects': [d['object'] | {'value': None} for d in documents.values()], 'entries': additions, 'outputs': outputs, 'inputs': list(inputs.values()), 'toolSha256': sha(Path(__file__).read_bytes()), 'planSha256': sha(plan_path.read_bytes())}
    (args.work / 'report.json').write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
    if args.apply:
        assert catalog_path.read_bytes() == before, 'Catalogue changed during staging'
        for output in outputs:
            target = args.native_target / output['path']
            if target.exists():
                assert sha(target.read_bytes()) == output['sha256'], ('Refusing overwrite', target)
        for output in outputs:
            target = args.native_target / output['path']
            target.parent.mkdir(parents=True, exist_ok=True)
            temp = target.with_suffix('.ogg.next')
            temp.write_bytes((args.work/'staging'/output['path']).read_bytes())
            temp.replace(target)
        temp = catalog_path.with_suffix('.json.next')
        temp.write_bytes(catalog_bytes)
        temp.replace(catalog_path)
    print(f'{len(additions)} routes; {len(outputs)-len(additions)} Russian recordings connected; applied={args.apply}')


if __name__ == '__main__':
    main()
