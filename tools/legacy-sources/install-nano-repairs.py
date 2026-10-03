"""Install declared GPU-validated Nano GLB/PNG closures and semantic registry entries.

Requires blake3 (pip install --target work/nano-editor-python blake3).
The plan and evidence stay in FusionForge. Runtime output is native data only.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'work/nano-editor-python'))
from blake3 import blake3


def sha(data):
    return hashlib.sha256(data).hexdigest()


def within(root, relative):
    path = (root / relative).resolve()
    assert path.is_relative_to(root.resolve()), relative
    return path


def install(plan_path, stage, target, report_path, apply):
    registry_path = target / '_runtime/characters.json'
    registry_before = registry_path.read_bytes()
    registry = json.loads(registry_before)
    assert registry['schema'] == 'ffone.semantic-character-registry.v2'
    outputs = {}
    evidence_records = []
    for item in json.loads(plan_path.read_text(encoding='utf-8'))['models']:
        source = within(stage, item['candidate'])
        glb = source.read_bytes()
        evidence_path = within(stage, item['evidence'])
        evidence = json.loads(evidence_path.read_text(encoding='utf-8'))
        assert evidence['schema'] == 'ffone.logical-model-gpu-evidence.v1'
        assert evidence['status'] == 'passed' and evidence['runtime']['sceneReady']
        assert evidence['runtime']['materialErrors'] == 0 and evidence['runtime']['shaderErrors'] == 0
        assert evidence['model']['glbSha256'] == sha(glb)
        assert evidence['model']['glbByteLength'] == len(glb)
        evidence_root = evidence_path
        for _ in Path(evidence['model']['relativeGlb']).parts:
            evidence_root = evidence_root.parent
        screenshot = within(evidence_root, evidence['screenshot']['relativePng']).read_bytes()
        assert sha(screenshot) == evidence['screenshot']['sha256']
        length = struct.unpack_from('<I', glb, 12)[0]
        document = json.loads(glb[20:20+length])
        assert evidence['model']['trueName'] == source.stem
        model = next((m for m in registry['models'] if m['id'] == item['id']), None)
        if model is None:
            model = dict(id=item['id'], logicalName=source.stem, category='nano', glb=item['target'])
            registry['models'].append(model)
        assert model['glb'] == item['target'] and model['category'] == 'nano'
        model['glbBlake3'] = blake3(glb).hexdigest()
        model['animations'] = [clip['name'] for clip in document['animations']]
        outputs[item['target']] = glb
        texture_uris = {image['uri'] for image in document['images']}
        for texture in document['textures']:
            for level in texture.get('extras', {}).get('ffone', {}).get('mipLevels', []):
                texture_uris.add(level['uri'])
        for uri in sorted(texture_uris):
            source_texture = within(source.parent, uri)
            assert source_texture.suffix == '.png'
            data = source_texture.read_bytes()
            relative = (Path(item['target']).parent / uri).as_posix()
            assert relative not in outputs or outputs[relative] == data
            outputs[relative] = data
        evidence_records.append(dict(path=item['evidence'], sha256=sha(evidence_path.read_bytes())))
    registry['models'].sort(key=lambda model: model['id'])
    outputs['_runtime/characters.json'] = (json.dumps(registry, indent=2, ensure_ascii=False)+'\n').encode()
    report = dict(schema='ffone.nano-repair-install.v1', applied=apply,
        evidence=evidence_records,
        outputs=[dict(path=path, bytes=len(data), sha256=sha(data)) for path,data in sorted(outputs.items())])
    if apply:
        before = {relative: within(target, relative).read_bytes() if within(target, relative).exists() else None for relative in outputs}
        backup = report_path.parent / 'preinstall'
        for relative,data in before.items():
            if data is not None:
                path = within(backup, relative); path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(data)
        assert registry_path.read_bytes() == registry_before, 'registry changed during validation'
        committed = []
        try:
            # Publish the registry last, once the complete native closure exists.
            for relative,data in outputs.items():
                path = within(target, relative); path.parent.mkdir(parents=True, exist_ok=True)
                pending = path.with_name(path.name+'.nano-repair-next')
                pending.write_bytes(data); os.replace(pending, path); committed.append(relative)
            for relative,data in outputs.items():
                assert within(target, relative).read_bytes() == data
        except BaseException:
            for relative in reversed(committed):
                path=within(target,relative)
                if before[relative] is None: path.unlink()
                else: path.write_bytes(before[relative])
            raise
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
    print(f'{len(outputs)} validated native files; applied={apply}')


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan',type=Path,required=True)
    parser.add_argument('--stage',type=Path,required=True)
    parser.add_argument('--target-root',type=Path,required=True)
    parser.add_argument('--report',type=Path,required=True)
    parser.add_argument('--apply',action='store_true')
    args=parser.parse_args()
    install(args.plan,args.stage,args.target_root,args.report,args.apply)
