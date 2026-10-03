"""Stage NanoController's table-selected textures in native GLB materials.

Inputs are a native GLB and focused FusionForge export-exact-texture reports.
The mesh/skin/animation binary chunk is copied byte-for-byte. No image repair,
material-name guesses, shader substitutions, or legacy runtime dependencies.
"""
import argparse
import base64
import copy
import hashlib
import json
from pathlib import Path
import shutil
import struct


def sha(data):
    return hashlib.sha256(data).hexdigest()


def add_texture(document, report, output):
    payload = report['payload']
    png = base64.b64decode(payload['dataUrl'].split(',', 1)[1])
    assert sha(png) == payload['sha256'] and len(png) == payload['byteLength']
    assert payload['pixelTransform'] == 'vertical-flip-only-for-png-top-left-origin'
    assert not any(payload[k] for k in ['resized', 'rgbRepair', 'alphaMaskApplied', 'tintApplied'])
    name = report['name']
    assert name.replace('_', '').isalnum(), name
    uri = f'{output.stem}.textures/{name}.png'
    texture_file = output.parent / uri
    texture_file.parent.mkdir(parents=True, exist_ok=True)
    texture_file.write_bytes(png)
    settings = report['sampler']
    mode = settings['filterMode']['value']
    wrap = settings['wrapMode']['value']
    assert mode in (0, 1, 2) and wrap in (0, 1)
    sampler_index = len(document['samplers'])
    descriptor = dict(name=name, magFilter='nearest' if mode == 0 else 'linear',
                      minFilter='nearest' if mode == 0 else 'linear',
                      wrapS='repeat' if wrap == 0 else 'clampToEdge',
                      wrapT='repeat' if wrap == 0 else 'clampToEdge',
                      mipMapBias=settings['mipBias']['value'],
                      anisotropyLevel=settings['aniso']['value'],
                      legacyFilterMode=mode, legacyWrapMode=wrap)
    document['samplers'].append(dict(name=name, magFilter=9728 if mode == 0 else 9729,
        minFilter=9728 if mode == 0 else 9729, wrapS=10497 if wrap == 0 else 33071,
        wrapT=10497 if wrap == 0 else 33071, extras={'ffone': descriptor}))
    source = report['sourcePayload']
    level = report['mipLevels'][0]
    assert report['mipCount'] == 1, 'explicit mip-chain support is required for other inputs'
    provenance = dict(sourceTextureFormat=report['textureFormat'],
        sourceTextureFormatName=report['textureFormatName'], sourceMipCount=report['mipCount'],
        sourceChainByteLength=source['byteLength'], sourceChainSha256=source['sha256'],
        sourceChainComplete=True, sourceLayout='largestToSmallestContiguous',
        publishedPixelTransform=payload['pixelTransform'], publishedPolicy='baseLevelOnly')
    mip = {k: level[k] for k in ['level','width','height','sourceByteOffset','sourceByteLength','sourceByteSha256']}
    mip.update(uri=uri, decodedRgba8ByteLength=level['decodedRgbaByteLength'],
               decodedRgba8Sha256=level['decodedRgbaSha256'], pngByteLength=len(png), pngSha256=sha(png))
    native = dict(sourceName=name, uri=uri, width=report['width'], height=report['height'],
                  sampler=sampler_index, mipProvenance=provenance, mipLevels=[mip])
    image_index = len(document['images'])
    document['images'].append(dict(name=name, uri=uri, mimeType='image/png', extras={'ffone': native}))
    texture_index = len(document['textures'])
    document['textures'].append(dict(name=name, sampler=sampler_index, source=image_index, extras={'ffone': native}))
    return dict(texture=texture_index, sourceName=name, uri=uri,
                sampler=dict(index=sampler_index, descriptor=descriptor),
                mipProvenance=provenance, mipLevels=[mip])


def repair(source, output, main=None, sub=None):
    raw = source.read_bytes()
    magic, version, length, json_length, kind = struct.unpack_from('<5I', raw)
    assert magic == 0x46546c67 and version == 2 and length == len(raw) and kind == 0x4e4f534a
    document = json.loads(raw[20:20+json_length])
    binary = raw[20+json_length:]
    output.parent.mkdir(parents=True, exist_ok=True)
    # Preserve ordinary existing PNGs referenced by the unchanged materials.
    for image in document['images']:
        uri = image['uri']
        assert not Path(uri).is_absolute() and '..' not in Path(uri).parts
        destination = output.parent / uri
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source.parent / uri, destination)
    changes = []
    for role, path in [('main', main), ('sub', sub)]:
        if path is None:
            continue
        report = json.loads(path.read_text(encoding='utf-8'))
        assert report['name'] == path.stem, 'wrong scoped Texture2D: check the AssetBundle PPtr external file'
        matches = [m for m in document['materials'] if role in m['name']]
        assert matches, f'{source}: no {role} material'
        replacement = add_texture(document, report, output)
        for material in matches:
            bindings = material['extras']['ffone']['textureBindings']
            binding = next(b for b in bindings if b['slot'] == '_MainTex')
            changes.append(dict(material=material['name'], previous=binding.get('sourceName'),
                                replacement=report['name'], textureSource=report['source']))
            binding.update(copy.deepcopy(replacement))
            material['pbrMetallicRoughness']['baseColorTexture'] = dict(
                index=replacement['texture'], texCoord=0,
                extras={'ffone': {'slot': '_MainTex', 'colorSpace': binding['colorSpace']}})
    encoded = json.dumps(document, separators=(',', ':'), ensure_ascii=False).encode()
    encoded += b' ' * (-len(encoded) % 4)
    output.write_bytes(struct.pack('<5I', magic, version, 20+len(encoded)+len(binary), len(encoded), kind)+encoded+binary)
    assert output.read_bytes()[20+len(encoded):] == binary
    return dict(inputSha256=sha(raw), outputSha256=sha(output.read_bytes()), binaryChunkSha256=sha(binary), changes=changes)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--main', type=Path)
    parser.add_argument('--sub', type=Path)
    parser.add_argument('--report', required=True, type=Path)
    args = parser.parse_args()
    result = repair(args.model, args.output, args.main, args.sub)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(result, indent=2)+'\n', encoding='utf-8')
    print(f'{args.output}: {len(result["changes"])} material bindings repaired')
