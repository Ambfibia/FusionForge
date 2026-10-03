"""Compare published Nano main/sub bindings to the primary game table.

Resolve table routes by semantic registry ID, never by imported root or filename.
This audits applicable texture replacements, not pixel parity or model availability
for extensions that have no table route.
"""
import argparse
import json
from pathlib import Path
import struct


def audit(root):
    registry = json.loads((root / '_runtime/characters.json').read_text(encoding='utf-8'))
    models = {m['id'].removeprefix('nano/'): m for m in registry['models'] if m['category'] == 'nano'}
    tables = json.loads((root / 'data/tables/table-set.json').read_text(encoding='utf-8'))
    rows = tables['tables'][0]['value']['m_pNanoTable']['m_pNanoMeshData']
    checked, missing, mismatches = [], [], []
    for row in rows:
        route = row['m_pstrMMeshModelString']
        if route in ('', 'null'):
            continue
        model = models.get(route)
        if model is None:
            missing.append(route)
            continue
        data = (root / model['glb']).read_bytes()
        length = struct.unpack_from('<I', data, 12)[0]
        glb = json.loads(data[20:20 + length])
        for role, field in [('main', 'm_pstrMTextureString'), ('sub', 'm_pstrMTextureString2')]:
            expected = row[field]
            if expected in ('', 'null'):
                continue
            for material in glb['materials']:
                if role not in material['name']:
                    continue
                binding = next((b for b in material['extras']['ffone']['textureBindings'] if b['slot'] == '_MainTex'), None)
                actual = binding.get('sourceName') if binding else None
                entry = dict(id=model['id'], glb=model['glb'], role=role, expected=expected, actual=actual)
                checked.append(entry)
                if actual != expected:
                    mismatches.append(entry)
    return dict(checkedBindings=len(checked), unresolvedRoutes=missing, mismatches=mismatches, bindings=checked)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target-root', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    result = audit(args.target_root)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({k: v for k, v in result.items() if k != 'bindings'}))
    raise SystemExit(bool(result['mismatches'] or result['unresolvedRoutes']))
