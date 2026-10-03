"""Relocate native assets and all JSON/GLB/code references without changing image bytes.

Run from FusionForge with --target-root ../FFOneClient --controls-zip <archive>.
The report belongs to Editor work, never to the runtime assets.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import posixpath
import re
import struct
import zipfile


def sha(data):
    return hashlib.sha256(data).hexdigest()


def glb_json(path):
    with path.open('rb') as stream:
        header = stream.read(20)
        size = struct.unpack_from('<I', header, 12)[0]
        return json.loads(stream.read(size)), 20 + size


def strings(value):
    if isinstance(value, dict):
        for item in value.values():
            yield from strings(item)
    elif isinstance(value, list):
        for item in value:
            yield from strings(item)
    elif isinstance(value, str):
        yield value


def npc_owners(root):
    document = json.loads((root / 'data/tables/table-set.json').read_bytes())
    models = next(t['value']['m_pCharacterModelData'] for t in document['tables']
                  if t['name'] == 'native_asset_routes')
    aliases = {}
    for model in models:
        for alias in [model['logicalName'], model['id'].split('/')[-1], *model.get('legacyAliases', [])]:
            aliases[alias.casefold()] = posixpath.dirname(model['glb'])
    owners = {}
    def walk(value):
        if isinstance(value, dict):
            for gender in ('M', 'F'):
                owner = aliases.get(value.get(f'm_pstr{gender}MeshModelString', '').casefold())
                if owner:
                    for suffix in ('', '2'):
                        texture = value.get(f'm_pstr{gender}TextureString{suffix}')
                        if texture and texture != 'null':
                            owners.setdefault(texture, set()).add(owner)
            for child in value.values():
                walk(child)
        elif isinstance(value, list):
            for child in value:
                walk(child)
    walk(document)
    return owners


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--target-root', type=Path, required=True)
    parser.add_argument('--controls-zip', type=Path, required=True)
    parser.add_argument('--report', type=Path, default=Path('work/native-texture-relocation.json'))
    args = parser.parse_args()
    repo = args.target_root.resolve()
    root = repo / 'assets/game'
    mapping = {}
    owners = {}
    glbs = []
    for path in root.rglob('*.glb'):
        value, offset = glb_json(path)
        parent = path.parent.relative_to(root).as_posix()
        glbs.append((path, value, offset))
        for value in strings(value):
            resolved = posixpath.normpath(posixpath.join(parent, value))
            if resolved.startswith('textures/'):
                owners.setdefault(resolved, set()).add(parent)
    effect_names = {'attack_hit_18', 'attack_hit_19', 'back_fusionmatter', 'bubble2',
                    'fusionlight', 'k_testing_fx', 'marker_mask', 'spawn11_green', 'spwaneye'}
    packages = [p for p in (root / 'characters/npcs').iterdir() if p.is_dir()]
    appearance_owners = npc_owners(root)
    for path in (root / 'textures').rglob('*.png'):
        old = path.relative_to(root).as_posix()
        parts = old.split('/')
        base = parts[2].removesuffix('.mips').removesuffix('.png')
        refs = owners.get('textures/' + parts[1] + '/' + base + '.png', set())
        if parts[1] == 'hnpc':
            destination = 'characters/hnpc/textures'
        elif parts[1] == 'npc':
            candidates = [p for p in packages if base == p.name or base.startswith(p.name + '_')]
            appearance = appearance_owners.get(base, set())
            destination = (next(iter(appearance)) if len(appearance) == 1 else
                           max(candidates, key=lambda p: len(p.name)).relative_to(root).as_posix()
                           if candidates else 'characters/npcs/shared') + '/textures'
        elif any(base == name or base.startswith(name + '_variant_') for name in effect_names):
            destination = 'effects/shared/textures'
        elif refs and all('/effects/' in p for p in refs):
            destination = 'effects/shared/textures'
        elif len(refs) == 1:
            destination = next(iter(refs)) + '/textures/shared'
        elif refs and all(p.startswith('characters/npcs/') for p in refs):
            destination = 'characters/npcs/shared/textures'
        elif refs and all(p.startswith('characters/mobs/') for p in refs):
            destination = 'characters/mobs/shared/textures'
        elif base.startswith('dt_cpch_gm_machine'):
            destination = 'characters/npcs/nanomachine/textures/shared'
        else:
            destination = 'characters/shared/textures'
        mapping[old] = destination + '/' + '/'.join(parts[2:])
    for path in (root / 'ui').rglob('*'):
        if path.is_file() and path.relative_to(root / 'ui').parts[0] not in ('en', 'ru'):
            old = path.relative_to(root).as_posix()
            mapping[old] = 'ui/en/' + old[3:]
    assert len(set(mapping.values())) == len(mapping), 'colliding destinations'
    for old, new in mapping.items():
        assert not (root / new).exists(), f'destination exists: {new}'
        assert (root / new).resolve().is_relative_to(root), new

    # Rewrite the JSON chunk only; binary buffers and every image byte stay intact.
    changed_glbs = []
    for path, value, offset in glbs:
        parent = path.parent.relative_to(root).as_posix()
        replacements = {}
        for text in strings(value):
            resolved = posixpath.normpath(posixpath.join(parent, text))
            if resolved in mapping:
                replacements[text] = posixpath.relpath(mapping[resolved], parent)
            elif text in mapping:
                replacements[text] = mapping[text]
        if not replacements:
            continue
        data = path.read_bytes()
        old_chunk = data[20:offset].decode('utf-8')
        chunk = old_chunk
        for old, new in replacements.items():
            chunk = chunk.replace(json.dumps(old), json.dumps(new))
        chunk = chunk.rstrip(' \x00').encode('utf-8')
        chunk += b' ' * (-len(chunk) % 4)
        output = struct.pack('<III', 0x46546C67, 2, 20 + len(chunk) + len(data[offset:]))
        output += struct.pack('<II', len(chunk), 0x4E4F534A) + chunk + data[offset:]
        assert output[20 + len(chunk):] == data[offset:]
        path.write_bytes(output)
        changed_glbs.append({'path': path.relative_to(root).as_posix(), 'before': sha(data), 'after': sha(output)})

    pattern = re.compile('|'.join(re.escape(k) for k in sorted(mapping, key=len, reverse=True)))
    ui_domains = [p.name for p in (root / 'ui').iterdir() if p.is_dir() and p.name not in ('en', 'ru')]
    ui_prefix = re.compile(r'ui/(' + '|'.join(map(re.escape, ui_domains)) + r')(?=/|["\x27])')
    changed_text = []
    for directory in ('assets/game', 'crates', 'xtask', 'tools', 'docs'):
        for path in (repo / directory).rglob('*'):
            if not path.is_file() or path.suffix not in ('.json', '.rs', '.md', '.mjs', '.toml', '.ps1'):
                continue
            data = path.read_bytes()
            if b'ui/' not in data and b'textures/' not in data:
                continue
            text = data.decode('utf-8')
            updated = pattern.sub(lambda m: mapping[m.group()], text)
            updated = ui_prefix.sub(r'ui/en/\1', updated)
            if updated != text:
                path.write_bytes(updated.encode('utf-8'))
                changed_text.append(path.relative_to(repo).as_posix())
    moved = []
    for old, new in mapping.items():
        source, target = root / old, root / new
        digest = sha(source.read_bytes())
        target.parent.mkdir(parents=True, exist_ok=True)
        source.rename(target)
        assert sha(target.read_bytes()) == digest
        moved.append({'from': old, 'to': new, 'sha256': digest})
    # Remove only verified empty directories within the explicit asset roots.
    for folder in (root / 'textures', root / 'ui'):
        for path in sorted(folder.rglob('*'), key=lambda p: len(p.parts), reverse=True):
            if path.is_dir() and not any(path.iterdir()):
                path.rmdir()
    if (root / 'textures').exists() and not any((root / 'textures').iterdir()):
        (root / 'textures').rmdir()
    translations = []
    with zipfile.ZipFile(args.controls_zip) as archive:
        assert set(archive.namelist()) == {'musicon.png', 'musicoff.png'}
        for name in archive.namelist():
            # Archive labels are reversed: musicon says MUSIC OFF in Russian,
            # musicoff says MUSIC ON. Match the established button action.
            source_name = {'musicon.png': 'musicoff.png', 'musicoff.png': 'musicon.png'}[name]
            data = archive.read(source_name)
            relative = 'character/selection/controls/' + name
            original = (root / 'ui/en' / relative).read_bytes()
            assert data[:8] == original[:8] == b'\x89PNG\r\n\x1a\n'
            assert data[16:24] == original[16:24], 'UI dimensions must remain unchanged'
            target = root / 'ui/ru' / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            translations.append({'path': target.relative_to(root).as_posix(),
                                 'archiveEntry': source_name, 'sha256': sha(data)})
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps({'moves': moved, 'glbs': changed_glbs,
        'textFiles': changed_text, 'translations': translations,
        'controlsZipSha256': sha(args.controls_zip.read_bytes())}, indent=2) + '\n')
    print(f'Moved {len(moved)} files; rewrote {len(changed_glbs)} GLBs and {len(changed_text)} text files; published {len(translations)} RU images')


if __name__ == '__main__':
    main()
