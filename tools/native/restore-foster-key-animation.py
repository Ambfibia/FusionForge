"""Restore the proven key clip without rewriting native mesh/material/texture data.

Input animation GLB is produced by FusionForge + publish-logical-model from the
primary mob/npc_key.kfm route. All staging paths must be supplied explicitly.
"""
import argparse
import copy
import json
import struct
from pathlib import Path


def read_glb(path):
    data = path.read_bytes()
    assert data[:4] == b'glTF'
    length = struct.unpack_from('<I', data, 12)[0]
    document = json.loads(data[20:20 + length])
    offset = 20 + length
    size, kind = struct.unpack_from('<II', data, offset)
    assert kind == 0x004E4942
    return document, data[offset + 8:offset + 8 + size]


def restore(base_path, animation_path, output):
    base, base_bin = read_glb(base_path)
    source, source_bin = read_glb(animation_path)
    # Exact hierarchy ownership, including local TRS, is required for binding.
    fields = ('name', 'children', 'translation', 'rotation', 'scale', 'matrix')
    assert [[n.get(k) for k in fields] for n in base['nodes']] == [
        [n.get(k) for k in fields] for n in source['nodes']]
    assert not base.get('animations')
    clip = copy.deepcopy(next(a for a in source['animations'] if a['name'] == 'stand1'))
    binary = bytearray(base_bin[:base['buffers'][0]['byteLength']])
    accessor_map, view_map = {}, {}

    def copy_accessor(index):
        if index in accessor_map:
            return accessor_map[index]
        accessor = copy.deepcopy(source['accessors'][index])
        assert 'sparse' not in accessor
        old_view = accessor['bufferView']
        if old_view not in view_map:
            view = copy.deepcopy(source['bufferViews'][old_view])
            assert view['buffer'] == 0
            start = view.get('byteOffset', 0)
            binary.extend(b'\0' * (-len(binary) % 4))
            view['byteOffset'] = len(binary)
            binary.extend(source_bin[start:start + view['byteLength']])
            view_map[old_view] = len(base['bufferViews'])
            base['bufferViews'].append(view)
        accessor['bufferView'] = view_map[old_view]
        accessor_map[index] = len(base['accessors'])
        base['accessors'].append(accessor)
        return accessor_map[index]

    for sampler in clip['samplers']:
        sampler['input'] = copy_accessor(sampler['input'])
        sampler['output'] = copy_accessor(sampler['output'])
    for channel in clip['channels']:
        channel.pop('extras', None)
    base['animations'] = [clip]
    base['buffers'][0]['byteLength'] = len(binary)
    document = json.dumps(base, ensure_ascii=False, separators=(',', ':')).encode('utf-8')
    document += b' ' * (-len(document) % 4)
    binary.extend(b'\0' * (-len(binary) % 4))
    data = (struct.pack('<III', 0x46546C67, 2, 28 + len(document) + len(binary))
            + struct.pack('<II', len(document), 0x4E4F534A) + document
            + struct.pack('<II', len(binary), 0x004E4942) + binary)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(data)
    # Preserve every pre-existing immutable render asset and binary accessor.
    before, before_bin = read_glb(base_path)
    after, after_bin = read_glb(output)
    for key in ('nodes', 'meshes', 'materials', 'images', 'textures', 'samplers', 'skins', 'scenes'):
        assert before.get(key) == after.get(key), key
    assert after_bin[:len(before_bin)] == before_bin
    assert after['accessors'][:len(before['accessors'])] == before['accessors']
    assert after['bufferViews'][:len(before['bufferViews'])] == before['bufferViews']
    print(f'Restored stand1 ({len(clip["channels"])} channels); original render assets unchanged: {output}')


def update_registry(path, model):
    import blake3
    text = path.read_text(encoding='utf-8')
    entry = next(m for m in json.loads(text)['models'] if m['id'] == 'npc/npc_key')
    digest = blake3.blake3(model.read_bytes()).hexdigest()
    if entry['glbBlake3'] == digest and entry['animations'] == ['stand1']:
        return
    assert entry['animations'] == []
    old = f'"glbBlake3": "{entry["glbBlake3"]}",\n      "animations": []'
    new = f'"glbBlake3": "{digest}",\n      "animations": [\n        "stand1"\n      ]'
    assert text.count(old) == 1
    path.write_text(text.replace(old, new), encoding='utf-8', newline='\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base', required=True, type=Path)
    parser.add_argument('--animation-glb', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--registry', type=Path, help='Optional native character registry to update')
    args = parser.parse_args()
    restore(args.base, args.animation_glb, args.output)
    if args.registry:
        update_registry(args.registry, args.output)
