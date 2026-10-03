"""Read-only donor extraction and pixel comparison; never installs runtime assets.

Run with the Editor-local UnityPy/Pillow dependencies on PYTHONPATH.
All discovery/cache data stays in --work; --output is an explicit review collection.
"""
import argparse
import collections
import hashlib
import json
import pathlib
import re
import struct
import gc
import io
import tarfile
from types import SimpleNamespace
from PIL import Image, ImageOps
import UnityPy
from UnityPy.export.Texture2DConverter import parse_image_data


def sha(data):
    return hashlib.sha256(data).hexdigest()


def pixel_key(image):
    image = image.convert('RGBA')
    prefix = struct.pack('<II', *image.size)
    return sha(prefix + image.tobytes())


def slug(name):
    return re.sub(r'[^\w. -]+', '_', name).strip(' .')[:100] or 'unnamed'


def name_key(name):
    name = name.lower()
    while pathlib.Path(name).suffix in ('.png', '.dds', '.tga', '.psd', '.jpg', '.jpeg'):
        name = pathlib.Path(name).stem
    return re.sub(r'[^a-z0-9]', '', name)


def category(text):
    text = text.lower().replace('\\', '/')
    if '/multiterraineditor/' in text:
        return None
    for prefix, result in [('nanoicon', 'Nanos'), ('mobicon', 'Mobs'), ('npcicon', 'Characters'), ('hnpcicon', 'Characters')]:
        if re.search(r'(^|[/ _-])' + prefix, text):
            return result
    if re.search(r'(^|[/ _-])(thinbrow|thickbrow|wideeye|sleepyeye|gameface|femaleskin|basefemaletexture|m_skin|m_naked)', text):
        return 'Cosmetics'
    if re.search(r'(^|[/ _-])(m|f)_(skin|naked|body|leg|cat|halter|sailormoon|paddedjumpers|helmet|berserker|battletee|lopsided)([/ ._-]|$)', text):
        return 'Cosmetics'
    if re.search(r'(clockwork(coat|furnace|hat|pants|boots)|urbanranger(backpack|pants|shirt|beret|sandal)|kndefense(shirt|pants|shoes)|waybig(feet|shirt|pants|helmet)|academy(graduate|training)|boysvulcanizedloafers|vulcanfrownyface)', text):
        return 'Cosmetics'
    if re.search(r'(^|[/ _-])(avatarbody|back_bmo|helmet_atomix|helmet_fusiontom|albedo_body|albedo_members|normal_body|metallic_body|metallic_members)([/ ._-]|$)', text):
        return 'Cosmetics'
    if re.search(r'(^|[/ _-])(bmo)([/ ._-]|$)', text):
        return 'Characters'
    if re.search(r'(^|[/ _-])(nano|nanos)([/ _-]|$)', text):
        return 'Nanos'
    if re.search(r'(^|[/ _-])(mob|mobs|monster|monsters|fusion)([/ _-]|$)', text):
        return 'Mobs'
    if re.search(r'(^|[/ _-])(npc|npcs|hnpc)([/ _-]|$)', text):
        return 'Characters'
    if re.search(r'(^|[/ _-])(player|avatar|wear|clothes|clothing|costume|cosmetic|equipment|hair|face|head|shirt|shirts|pants|pant|shoes|shoe|hat|hats|backpack|accessory|weapon|weapons|melee|rifle|pistol|rocket|shattergun)([/ _-]|$)', text):
        return 'Cosmetics'
    if '/characters/' in '/' + text:
        return 'Characters'
    return None


def outside_requested_content(row):
    name = row['name'].lower()
    if re.match(r'^(dt_|sb_|wd_|wt_|etc_|ep_|ex_|terraindata|ldr_)', name):
        return 'environment texture'
    if re.search(r'(^title_|interactionui|foregroundpiece|nano_holster|nano_power|nanoback|nanotab|nano_info|nano_wheel|nano_whell|nano_stamina|nano_com|nanocom|mob_hp|playerinfo|toonramp|tcp2.*ramp|nanoglow|bloodspray|^attack_hit|^monster_megaattack|^ring[._])', name):
        return 'interface, common shader ramp or standalone effect'
    return None


def save_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding='utf-8')


def read_unity25_texture(obj):
    """Unity 2.5 stores this exact type tree tightly packed, including strings.

    Gate on the serialized schema; do not scan for plausible dimensions or payloads.
    """
    node = obj.serialized_type.node
    expected = ['m_Name', 'm_Width', 'm_Height', 'm_CompleteImageSize', 'm_TextureFormat',
                'm_MipMap', 'm_ImageCount', 'm_TextureDimension', 'm_TextureSettings', 'image data']
    if not obj.assets_file.unity_version.startswith('2.5.') or [n.m_Name for n in node.m_Children] != expected:
        raise ValueError('unsupported packed Texture2D schema')
    raw = obj.get_raw_data()
    length = struct.unpack_from('<I', raw)[0]
    if length > len(raw) - 49:
        raise ValueError('invalid packed Texture2D name length')
    name = raw[4:4+length].rstrip(b'\0').decode('utf-8')
    offset = 4 + length
    width, height, complete, fmt = struct.unpack_from('<4i', raw, offset)
    offset += 16
    mipmap = raw[offset]; offset += 1
    image_count, dimension = struct.unpack_from('<2i', raw, offset); offset += 8
    settings = struct.unpack_from('<iifi', raw, offset); offset += 16
    payload_size = struct.unpack_from('<I', raw, offset)[0]; offset += 4
    if width <= 0 or height <= 0 or mipmap not in (0, 1) or image_count != 1 or dimension != 2:
        raise ValueError('invalid packed Texture2D dimensions or ownership')
    if payload_size != complete or offset + payload_size != len(raw):
        raise ValueError('packed Texture2D payload does not exactly consume the object')
    image = parse_image_data(raw[offset:], width, height, fmt, obj.version, obj.platform)
    return SimpleNamespace(m_Name=name, m_Width=width, m_Height=height,
                           m_TextureFormat=fmt, image=image, packedSettings=settings)


def main():
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument('--inventory', required=True, type=pathlib.Path)
    parser.add_argument('--native', required=True, type=pathlib.Path)
    parser.add_argument('--work', required=True, type=pathlib.Path)
    parser.add_argument('--output', required=True, type=pathlib.Path)
    parser.add_argument('--retry-errors', action='store_true')
    args = parser.parse_args()
    args.work.mkdir(parents=True, exist_ok=True)
    cache = args.work / 'decoded'
    cache.mkdir(exist_ok=True)
    errors = []
    sources = json.loads(args.inventory.read_text())
    native_cache = args.work / 'native-pixels.json'
    if native_cache.exists():
        native = json.loads(native_cache.read_text(encoding='utf-8'))
    else:
        native = []
        for i, path in enumerate(args.native.rglob('*.png')):
            try:
                with Image.open(path) as image:
                    row = {'path': str(path.resolve()), 'name': path.stem,
                           'pixels': pixel_key(image), 'flipPixels': pixel_key(ImageOps.flip(image)),
                           'size': list(image.size), 'mtimeNs': path.stat().st_mtime_ns}
                    native.append(row)
            except Exception as e:
                errors.append({'native': str(path), 'error': str(e)})
            if i % 3000 == 0:
                print('NATIVE', i, flush=True)
        save_json(native_cache, native)
    native_pixels = collections.defaultdict(list)
    native_names = collections.defaultdict(list)
    for row in native:
        native_pixels[row['pixels']].append(row['path'])
        native_pixels[row['flipPixels']].append(row['path'])
        native_names[name_key(row['name'])].append(row['path'])
    records = []
    stats = []
    raw_hashes = {}
    def add(image, row):
        image = image.convert('RGBA')
        if not image.width or not image.height:
            return
        key = pixel_key(image)
        row.update(pixels=key, size=list(image.size))
        row['category'] = category(row['name'] + '/' + row.get('relative', '') + '/' + ' '.join(row.get('owners', [])))
        matches = native_pixels.get(key, [])
        row['nativeExact'] = matches[:3]
        row['nativeNameCandidates'] = native_names.get(name_key(row['name']), [])[:12]
        if not matches:
            png = cache / (key + '.png')
            if not png.exists():
                image.save(png)
        records.append(row)

    for src in sources:
        root = pathlib.Path(src['root'])
        source = src.get('alias', root.name)
        checkpoint = args.work / 'source-cache' / (slug(source) + '.json')
        start = len(records); error_start = len(errors); containers = 0
        retry_files = None
        if checkpoint.exists():
            saved = json.loads(checkpoint.read_text(encoding='utf-8'))
            if not (args.retry_errors and saved['errors']):
                records.extend(saved['records']); errors.extend(saved['errors']); stats.append(saved['stats'])
                print('CACHED', source, len(saved['records']), flush=True)
                continue
            retry_files = {e['file'] for e in saved['errors']}
            records.extend(r for r in saved['records'] if r['relative'] not in retry_files)
            containers = saved['stats']['containers']
        for entry in src['files']:
            path = pathlib.Path(entry['path']); suffix = path.suffix.lower()
            relative = str(path.relative_to(root))
            if retry_files is not None and relative not in retry_files:
                continue
            if suffix == '.unitypackage':
                try:
                    package_hash = sha(path.read_bytes())
                    with tarfile.open(path, 'r:gz') as package:
                        members = {m.name: m for m in package.getmembers() if m.isfile()}
                        for member in members:
                            if not member.endswith('/pathname'):
                                continue
                            asset_name = package.extractfile(members[member]).read().decode('utf-8').strip('\0\r\n')
                            if pathlib.Path(asset_name).suffix.lower() not in ('.png', '.dds', '.tga', '.psd', '.jpg', '.jpeg'):
                                continue
                            asset_member = member.rsplit('/', 1)[0] + '/asset'
                            payload = package.extractfile(members[asset_member]).read()
                            with Image.open(io.BytesIO(payload)) as image:
                                add(image, {'source':source, 'name':pathlib.Path(asset_name).name,
                                    'relative':relative, 'unityPackageAsset':asset_name,
                                    'raw':str(path), 'rawSha256':package_hash, 'kind':'unitypackage', 'owners':[asset_name]})
                except Exception as e:
                    errors.append({'source':source, 'file':relative, 'error':str(e)})
                continue
            if suffix in ('.png', '.dds', '.tga', '.psd', '.jpg', '.jpeg', '.bmp', '.tif', '.tiff'):
                try:
                    with Image.open(path) as image:
                        add(image, {'source':source, 'name':path.name, 'relative':relative,
                                    'raw':str(path), 'rawSha256':sha(path.read_bytes()), 'kind':'loose', 'owners':[]})
                except Exception as e:
                    errors.append({'source':source, 'file':relative, 'error':str(e)})
                continue
            if not (suffix in ('.assets', '.unity3d', '.bundle', '.resourcefile') or
                    re.fullmatch(r'(mainData|level\d+|globalgamemanagers|data)', path.name)):
                continue
            if retry_files is None:
                containers += 1
            try:
                env = UnityPy.load(str(path))
                objects = list(env.objects)
                owners = collections.defaultdict(list)
                # Only local, scoped references are used for classification. External pointers
                # are not guessed from global PathID or similarly named loaded objects.
                for obj in objects:
                    if obj.type.name != 'Material':
                        continue
                    try:
                        data = obj.read()
                        for slot, texenv in data.m_SavedProperties.m_TexEnvs:
                            ptr = texenv.m_Texture
                            if ptr.m_FileID == 0 and ptr.m_PathID:
                                owners[(obj.assets_file.name, ptr.m_PathID)].append(data.m_Name + ':' + slot)
                    except Exception:
                        pass
                count = 0
                for obj in objects:
                    if obj.type.name != 'Texture2D':
                        continue
                    try:
                        try:
                            data = obj.read()
                        except Exception:
                            data = read_unity25_texture(obj)
                        if data.m_Width <= 0 or data.m_Height <= 0:
                            continue
                        stream = getattr(data, 'm_StreamData', None)
                        stream_info = {'path':getattr(stream,'path',None),'offset':getattr(stream,'offset',None),'size':getattr(stream,'size',None)} if stream else None
                        if str(path) not in raw_hashes:
                            raw_hashes[str(path)] = sha(path.read_bytes())
                        try:
                            image = data.image
                        except AttributeError:
                            # Some old custom type trees deserialize as UnknownObject but
                            # retain all exact Texture2D fields and the embedded payload.
                            image = parse_image_data(data.image_data, data.m_Width, data.m_Height,
                                data.m_TextureFormat, obj.version, obj.platform)
                        add(image, {'source':source, 'relative':relative, 'raw':str(path),
                            'rawSha256':raw_hashes[str(path)], 'kind':'Texture2D',
                            'serializedAsset':obj.assets_file.name, 'pathId':obj.path_id,
                            'name':data.m_Name, 'format':int(data.m_TextureFormat),
                            'mipCount':getattr(data,'m_MipCount',None), 'stream':stream_info,
                            'owners':owners.get((obj.assets_file.name,obj.path_id),[])})
                        count += 1
                    except Exception as e:
                        errors.append({'source':source,'file':relative,'pathId':obj.path_id,'error':str(e)})
                print('CONTAINER', source, relative, count, flush=True)
                del env, objects
                gc.collect()
            except Exception as e:
                errors.append({'source':source,'file':relative,'error':str(e)})
                print('ERROR', source, relative, str(e)[:160], flush=True)
        stat = {'source':source,'containers':containers,'textures':len(records)-start,'errors':len(errors)-error_start}
        stats.append(stat)
        save_json(checkpoint, {'records':records[start:],'errors':errors[error_start:],'stats':stat})
        print('SOURCE', stat, flush=True)

    save_json(args.work/'all-textures.json',records)
    save_json(args.work/'errors.json', errors)
    # Same image in several donors has one review file and multiple provenance rows.
    groups = collections.defaultdict(list)
    for row in records:
        row['nativeExact'] = native_pixels.get(row['pixels'], [])[:3]
        row['nativeNameCandidates'] = native_names.get(name_key(row['name']), [])[:12]
        row['category'] = category(row['name'] + '/' + row.get('relative', '') + '/' + ' '.join(row.get('owners', [])))
        row['scopeExclusion'] = outside_requested_content(row)
        if row['scopeExclusion']:
            row['category'] = None
            continue
        if not row['category']:
            inferred = {category(p) for p in row['nativeNameCandidates'] if '/characters/' in p.replace('\\', '/').lower()}
            inferred.discard(None)
            if len(inferred) == 1:
                row['category'] = inferred.pop()
        if row['category'] and not row['nativeExact']:
            groups[row['pixels']].append(row)
    args.output.mkdir(parents=True, exist_ok=True)
    exported = []
    for key, rows in groups.items():
        row = rows[0]
        status = 'Different' if any(r['nativeNameCandidates'] for r in rows) else 'NoExactMatch'
        relative = pathlib.Path(slug(row['source'])) / row['category'] / status / (slug(row['name']) + '__' + key[:12] + '.png')
        target = args.output / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        payload = (cache/(key+'.png')).read_bytes()
        if target.exists() and target.read_bytes() != payload:
            raise RuntimeError('refusing to overwrite different output '+str(target))
        target.write_bytes(payload)
        exported.append({'file':relative.as_posix(), 'pixels':key, 'pngSha256':sha(payload),
                         'size':row['size'],'status':status,'category':row['category'],'sources':rows})
    report = {'nativeRoot':str(args.native.resolve()),'nativePngCount':len(native),
              'comparison':'Exact decoded base-level RGBA pixels and dimensions, allowing a vertical orientation flip. Names only classify candidates; no asset ownership equivalence is asserted.',
              'sources':stats,'scannedTextureCount':len(records),
              'exactNativeOccurrences':sum(bool(r['nativeExact']) for r in records),
              'exportedUniqueTextures':len(exported),'errors':errors,'textures':exported}
    save_json(args.output/'manifest.json',report)
    save_json(args.work/'result.json',report)
    print('RESULT', len(exported), 'unique textures;',len(errors),'errors',flush=True)


if __name__ == '__main__':
    main()
