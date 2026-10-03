"""Turn an authored glTF character into a `ffone.skinned-model.v1` document.

A few NPCs of the modded build have no Unity source to export. The modding tool
kept only a decomposed `fftools.npc-build-assets.v1` dump -- loose `.mesh`,
`.gameobject` and `.anim` objects with no AssetBundle container table -- so
`export-logical-model-source` resolves zero routes for them and the normal
publisher can never see them. What does exist is the authored model the NPC was
built from, plus its texture.

This tool reads that authored GLB and writes the same `NativeModel` document the
publisher produces, so `fusionforge native encode-native-model` can run the
ordinary validator over it and emit a GLB the ordinary GPU gate and installer
accept. Nothing here invents shading: the material contract is copied from an
already published model that uses the *same* ShaderLab program, and the
per-material colours and floats are parsed out of the authored NPC's own
serialized `.mat` when one is supplied.

Two details are easy to get wrong and are handled explicitly:

* The published PNG keeps its top-left origin and the provenance records the
  `vertical-flip-only-for-png-top-left-origin` transform, so `decoded_rgba8` must
  hash the *flipped* buffer -- the runtime reverses the transform before
  comparing. Getting this backwards fails acceptance with a hash mismatch.
* An authored texture has no Unity byte stream, so its declared source chain is
  the decoded RGBA8 itself, recorded as `RGBA32` with a single `baseLevelOnly`
  level. That is a true statement about this asset rather than a borrowed hash.

    python tools/legacy-sources/build-authored-character-model.py \\
      --authored ../Models/npc_otto.glb --texture ../Models/npc_otto.png \\
      --name npc_otto --material-donor ../FFOneClient/assets/game/characters/npcs/npc_3430_rex/npc_rex.glb \\
      --out work/legacy-sources/otto-authored-20260903
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import struct
import sys
import zlib
from pathlib import Path

COMPONENT = {5120: ("b", 1), 5121: ("B", 1), 5122: ("h", 2), 5123: ("H", 2), 5125: ("I", 4), 5126: ("f", 4)}
COUNTS = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}


def read_glb(path: Path) -> tuple[dict, bytes]:
    data = path.read_bytes()
    if data[:4] != b"glTF":
        raise SystemExit(f"{path} is not a GLB")
    offset = 12
    document, binary = None, b""
    while offset < len(data):
        length, kind = struct.unpack_from("<II", data, offset)
        chunk = data[offset + 8 : offset + 8 + length]
        if kind == 0x4E4F534A:
            document = json.loads(chunk)
        elif kind == 0x004E4942:
            binary = chunk
        offset += 8 + length + ((4 - length % 4) % 4 if length % 4 else 0)
    if document is None:
        raise SystemExit(f"{path} has no JSON chunk")
    return document, binary


def accessor(document: dict, binary: bytes, index: int) -> list:
    acc = document["accessors"][index]
    fmt, size = COMPONENT[acc["componentType"]]
    count = COUNTS[acc["type"]]
    view = document["bufferViews"][acc["bufferView"]]
    base = view.get("byteOffset", 0) + acc.get("byteOffset", 0)
    stride = view.get("byteStride") or size * count
    out = []
    for element in range(acc["count"]):
        start = base + element * stride
        values = struct.unpack_from("<" + fmt * count, binary, start)
        out.append(list(values))
    if acc.get("normalized") and acc["componentType"] in (5121, 5123):
        maximum = 255.0 if acc["componentType"] == 5121 else 65535.0
        out = [[value / maximum for value in row] for row in out]
    return out


def decode_png_rgba8(data: bytes) -> tuple[int, int, bytes]:
    """Minimal decoder for the non-interlaced 8-bit PNGs these assets use."""
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit("texture is not a PNG")
    width = height = color_type = bit_depth = interlace = None
    idat = bytearray()
    palette = b""
    trns = b""
    offset = 8
    while offset < len(data):
        length, kind = struct.unpack_from(">I4s", data, offset)
        body = data[offset + 8 : offset + 8 + length]
        if kind == b"IHDR":
            width, height, bit_depth, color_type, _, _, interlace = struct.unpack(">IIBBBBB", body)
        elif kind == b"PLTE":
            palette = body
        elif kind == b"tRNS":
            trns = body
        elif kind == b"IDAT":
            idat += body
        elif kind == b"IEND":
            break
        offset += 12 + length
    if bit_depth != 8 or interlace != 0:
        raise SystemExit(f"unsupported PNG: bitDepth={bit_depth} interlace={interlace}")
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[color_type]
    raw = zlib.decompress(bytes(idat))
    stride = width * channels
    out = bytearray(width * height * 4)
    previous = bytearray(stride)
    position = 0
    for row in range(height):
        filter_type = raw[position]
        position += 1
        line = bytearray(raw[position : position + stride])
        position += stride
        for index in range(stride):
            left = line[index - channels] if index >= channels else 0
            up = previous[index]
            up_left = previous[index - channels] if index >= channels else 0
            if filter_type == 1:
                line[index] = (line[index] + left) & 0xFF
            elif filter_type == 2:
                line[index] = (line[index] + up) & 0xFF
            elif filter_type == 3:
                line[index] = (line[index] + ((left + up) >> 1)) & 0xFF
            elif filter_type == 4:
                p = left + up - up_left
                pa, pb, pc = abs(p - left), abs(p - up), abs(p - up_left)
                predictor = left if pa <= pb and pa <= pc else (up if pb <= pc else up_left)
                line[index] = (line[index] + predictor) & 0xFF
            elif filter_type != 0:
                raise SystemExit(f"unsupported PNG filter {filter_type}")
        target = row * width * 4
        for column in range(width):
            source = column * channels
            if color_type == 6:
                out[target : target + 4] = line[source : source + 4]
            elif color_type == 2:
                out[target : target + 3] = line[source : source + 3]
                out[target + 3] = 255
            elif color_type == 0:
                grey = line[source]
                out[target] = out[target + 1] = out[target + 2] = grey
                out[target + 3] = 255
            elif color_type == 4:
                grey = line[source]
                out[target] = out[target + 1] = out[target + 2] = grey
                out[target + 3] = line[source + 1]
            else:
                entry = line[source] * 3
                out[target : target + 3] = palette[entry : entry + 3]
                out[target + 3] = trns[line[source]] if line[source] < len(trns) else 255
            target += 4
        previous = line
    return width, height, bytes(out)


def repair_opposing_windings(
    positions: list, normals: list, indices: list, repair: bool
) -> list[dict]:
    """Report -- and optionally flip -- triangles wound against their normals.

    Native validation rejects a mesh where any triangle's geometric normal
    opposes its authored vertex normals, because a published model's winding is
    an audited contract rather than a renderer setting. A DCC export can leave a
    near-degenerate sliver whose geometric normal is numerically meaningless at
    that scale. Flipping such a triangle is a real edit to the authored mesh, so
    it happens only when asked for and every flipped triangle is reported with
    the area and dot product that justified it.
    """

    def sub(a, b):
        return [a[0] - b[0], a[1] - b[1], a[2] - b[2]]

    def cross(a, b):
        return [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]

    repaired = []
    for triangle in range(0, len(indices), 3):
        a, b, c = indices[triangle], indices[triangle + 1], indices[triangle + 2]
        face = cross(sub(positions[b], positions[a]), sub(positions[c], positions[a]))
        average = [(normals[a][axis] + normals[b][axis] + normals[c][axis]) / 3 for axis in range(3)]
        alignment = sum(x * y for x, y in zip(face, average))
        if alignment >= 0:
            continue
        record = {
            "triangle": triangle // 3,
            "vertices": [a, b, c],
            "crossProductLength": sum(x * x for x in face) ** 0.5,
            "normalAlignment": alignment,
            "flipped": repair,
        }
        if repair:
            indices[triangle + 1], indices[triangle + 2] = c, b
        repaired.append(record)
    return repaired


def flip_rows(pixels: bytes, width: int, height: int) -> bytes:
    stride = width * 4
    return b"".join(pixels[(height - 1 - row) * stride : (height - row) * stride] for row in range(height))


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def native_texture(png_path: Path, source_name: str, uri: str, sampler_index: int) -> dict:
    data = png_path.read_bytes()
    width, height, rgba = decode_png_rgba8(data)
    source_pixels = flip_rows(rgba, width, height)
    return {
        "sourceName": source_name,
        "uri": uri,
        "width": width,
        "height": height,
        "sampler": sampler_index,
        "mipProvenance": {
            "sourceTextureFormat": 4,
            "sourceTextureFormatName": "RGBA32",
            "sourceMipCount": 1,
            "sourceChainByteLength": len(source_pixels),
            "sourceChainSha256": sha256(source_pixels),
            "sourceChainComplete": True,
            "sourceLayout": "largestToSmallestContiguous",
            "publishedPixelTransform": "vertical-flip-only-for-png-top-left-origin",
            "publishedPolicy": "baseLevelOnly",
        },
        "mipLevels": [
            {
                "level": 0,
                "width": width,
                "height": height,
                "uri": uri,
                "sourceByteOffset": 0,
                "sourceByteLength": len(source_pixels),
                "sourceByteSha256": sha256(source_pixels),
                "decodedRgba8ByteLength": len(source_pixels),
                "decodedRgba8Sha256": sha256(source_pixels),
                "pngByteLength": len(data),
                "pngSha256": sha256(data),
            }
        ],
    }


def parse_material_values(mat_path: Path | None) -> tuple[list, list]:
    """Read `_Color`-family and `_Outline` values from a serialized Unity material.

    Saved properties are stored as a length-prefixed name followed by the value,
    four-byte aligned. Only the properties this shader declares are read, and a
    property the file does not contain is simply absent from the result rather
    than defaulted.
    """
    if mat_path is None:
        return [], []
    body = mat_path.read_bytes()
    colors, floats = [], []
    for name in ("_Color", "_AmbColor", "_Emission", "_OutlineColor"):
        index = body.find(name.encode())
        if index < 0:
            continue
        start = index + len(name)
        start = (start + 3) & ~3
        colors.append({"name": name, "value": [round(v, 7) for v in struct.unpack_from("<4f", body, start)]})
    index = body.find(b"_Outline\x00") if b"_Outline\x00" in body else body.find(b"_Outline")
    if index >= 0:
        start = (index + len("_Outline") + 3) & ~3
        floats.append({"name": "_Outline", "value": round(struct.unpack_from("<f", body, start)[0], 9)})
    return colors, floats


def donor_material(donor_glb: Path) -> dict:
    document, _ = read_glb(donor_glb)
    return document["materials"][0]["extras"]["ffone"]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--authored", type=Path, required=True)
    parser.add_argument("--texture", type=Path, required=True)
    parser.add_argument("--name", required=True)
    parser.add_argument("--material-donor", type=Path, required=True)
    parser.add_argument("--material-name", default=None)
    parser.add_argument("--material-values", type=Path, default=None, help="serialized Unity .mat")
    parser.add_argument("--ramp", type=Path, default=None, help="_ShaderMap ramp PNG")
    parser.add_argument("--animation-name", default="stand1")
    parser.add_argument(
        "--repair-winding",
        action="store_true",
        help="flip triangles whose winding opposes their authored normals",
    )
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    document, binary = read_glb(args.authored)
    donor = donor_material(args.material_donor)

    # --- nodes -------------------------------------------------------------
    gltf_nodes = document["nodes"]
    parents: dict[int, int] = {}
    for index, node in enumerate(gltf_nodes):
        for child in node.get("children", []):
            parents[child] = index
    roots = [index for index in range(len(gltf_nodes)) if index not in parents]

    mesh_node = next((i for i, n in enumerate(gltf_nodes) if "mesh" in n), None)
    if mesh_node is None:
        raise SystemExit("authored GLB has no node carrying a mesh")

    # A native model has exactly one root, the way the Unity character
    # GameObject contains both its rig and its SkinnedMeshRenderer. A DCC export
    # commonly leaves the rig and the mesh as siblings, so the node the model is
    # named after adopts the other roots instead of inventing a new one.
    named_root = next(
        (index for index in roots if gltf_nodes[index].get("name") == args.name), None
    )
    if named_root is None:
        named_root = mesh_node if mesh_node in roots else roots[0]
    for index in roots:
        if index != named_root:
            parents[index] = named_root
    roots = [named_root]

    nodes = []
    for index, node in enumerate(gltf_nodes):
        nodes.append(
            {
                "name": node.get("name", f"node_{index}"),
                "parent": parents.get(index),
                "translation": [float(v) for v in node.get("translation", [0.0, 0.0, 0.0])],
                "rotation": [float(v) for v in node.get("rotation", [0.0, 0.0, 0.0, 1.0])],
                "scale": [float(v) for v in node.get("scale", [1.0, 1.0, 1.0])],
                "mesh": 0 if index == mesh_node else None,
                "skin": 0 if index == mesh_node and "skin" in node else None,
            }
        )

    # --- mesh --------------------------------------------------------------
    gltf_mesh = document["meshes"][gltf_nodes[mesh_node]["mesh"]]
    if len(gltf_mesh["primitives"]) != 1:
        raise SystemExit("only a single-primitive authored mesh is supported")
    primitive = gltf_mesh["primitives"][0]
    attributes = primitive["attributes"]
    material_name = args.material_name or f"{args.name}_material"
    positions = accessor(document, binary, attributes["POSITION"])
    normals = accessor(document, binary, attributes["NORMAL"])
    uvs = accessor(document, binary, attributes["TEXCOORD_0"])
    joints = [[int(v) for v in row] for row in accessor(document, binary, attributes["JOINTS_0"])]
    weights = accessor(document, binary, attributes["WEIGHTS_0"])
    indices = [row[0] for row in accessor(document, binary, primitive["indices"])]

    repaired_windings = repair_opposing_windings(positions, normals, indices, args.repair_winding)

    meshes = [
        {
            "name": gltf_mesh.get("name", args.name),
            "renderer_order": 0,
            "primitives": [
                {
                    "material": 0,
                    "materialSlot": material_name,
                    "positions": positions,
                    "normals": normals,
                    "uvs": uvs,
                    "joints": joints,
                    "weights": weights,
                    "indices": indices,
                }
            ],
        }
    ]

    # --- skin --------------------------------------------------------------
    gltf_skin = document["skins"][gltf_nodes[mesh_node]["skin"]]
    matrices = accessor(document, binary, gltf_skin["inverseBindMatrices"])
    skins = [
        {
            "name": gltf_skin.get("name", "skin"),
            "skeletonRoot": gltf_skin.get("skeleton", gltf_skin["joints"][0]),
            "joints": gltf_skin["joints"],
            # glTF stores MAT4 column-major, while a NativeModel matrix is
            # indexed [row][column] and encode_glb transposes it back on the way
            # out. Grouping the flat array into fours would publish the
            # transpose and break skinning.
            "inverseBindMatrices": [
                [[matrix[column * 4 + row] for column in range(4)] for row in range(4)]
                for matrix in matrices
            ],
        }
    ]

    # --- textures and material --------------------------------------------
    # Each texture owns its sampler entry: validation cross-checks a binding's
    # sampler descriptor against the one the texture itself references, so a
    # single shared sampler would contradict every texture but the first.
    def legacy_sampler(name: str) -> dict:
        return {
            "name": name,
            "magFilter": "linear",
            # A baseLevelOnly texture has no mip chain, and validation derives
            # the standard min filter from legacyFilterMode together with that
            # fact: with mips it would be linearMipmapNearest, without them it
            # must stay plain linear.
            "minFilter": "linear",
            "wrapS": "repeat",
            "wrapT": "repeat",
            "legacyFilterMode": 1,
            "legacyWrapMode": 0,
            "anisotropyLevel": 1,
            "mipMapBias": 0.0,
        }

    texture_dir = f"{args.name}.textures"
    samplers = [legacy_sampler(args.name)]
    textures = [native_texture(args.texture, args.name, f"{texture_dir}/{args.name}.png", 0)]
    ramp_index = None
    if args.ramp is not None:
        ramp_index = len(textures)
        samplers.append(legacy_sampler(args.ramp.stem))
        textures.append(
            native_texture(args.ramp, args.ramp.stem, f"{texture_dir}/{args.ramp.name}", ramp_index)
        )

    def binding(slot: str, texture_index: int | None, color_space: str) -> dict:
        if texture_index is None:
            return {
                "slot": slot,
                "ignoredStaleShaderBinding": False,
                "texture": None,
                "sourceName": None,
                "uri": None,
                "sampler": None,
                "mipProvenance": None,
                "mipLevels": None,
                "scale": [1.0, 1.0],
                "offset": [0.0, 0.0],
                "pivot": None,
                "rotation": None,
                "colorSpace": color_space,
            }
        texture = textures[texture_index]
        sampler_index = texture["sampler"]
        return {
            "slot": slot,
            "ignoredStaleShaderBinding": False,
            "texture": texture_index,
            "sourceName": texture["sourceName"],
            "uri": texture["uri"],
            "sampler": {"index": sampler_index, "descriptor": samplers[sampler_index]},
            "mipProvenance": texture["mipProvenance"],
            "mipLevels": texture["mipLevels"],
            "scale": [1.0, 1.0],
            "offset": [0.0, 0.0],
            "pivot": None,
            "rotation": None,
            "colorSpace": color_space,
        }

    colors, floats = parse_material_values(args.material_values)
    if not colors:
        colors = donor["colors"]
    if not floats:
        floats = donor["floats"]

    materials = [
        {
            "name": material_name,
            "serializedShaderName": donor["serializedShaderName"],
            "declaredShaderName": donor["declaredShaderName"],
            "legacyShaderName": donor["legacyShaderName"],
            "renderQueue": donor["renderQueue"],
            "colors": colors,
            "floats": floats,
            "shaderTextureDefaults": donor["shaderTextureDefaults"],
            "textureBindings": [
                binding("_MainTex", 0, "srgb"),
                binding("_SpecMap", None, "linear"),
                binding("_ShaderMap", ramp_index, "linear"),
            ],
            "passes": donor["passes"],
            "standardTextureRefsAreLoaderHints": True,
        }
    ]

    # --- animation ---------------------------------------------------------
    animations = []
    for clip in document.get("animations", []):
        channels = []
        duration = 0.0
        # Native validation requires the canonical Unity curve order -- every
        # translation curve, then every rotation, then every scale -- with a
        # strictly increasing source index inside each group. A glTF exporter
        # interleaves them per node, so they are regrouped here rather than
        # renumbered arbitrarily.
        ranked = sorted(
            (
                channel
                for channel in clip["channels"]
                if channel["target"].get("node") is not None
                and channel["target"]["path"] in ("translation", "rotation", "scale")
            ),
            key=lambda channel: (
                {"translation": 0, "rotation": 1, "scale": 2}[channel["target"]["path"]],
                channel["target"]["node"],
            ),
        )
        per_path_index: dict[str, int] = {}
        for channel in ranked:
            target = channel["target"]
            order = per_path_index.get(target["path"], 0)
            per_path_index[target["path"]] = order + 1
            sampler_entry = clip["samplers"][channel["sampler"]]
            times = [row[0] for row in accessor(document, binary, sampler_entry["input"])]
            values = accessor(document, binary, sampler_entry["output"])
            duration = max(duration, max(times) if times else 0.0)
            channels.append(
                {
                    "targetNode": target["node"],
                    "sourceIndex": order,
                    "sourceEncoding": "plain",
                    "sourceKeyCount": len(times),
                    "sourceKeyIndices": list(range(len(times))),
                    "duplicateKeys": [],
                    "interpolation": {"LINEAR": "LINEAR", "STEP": "STEP", "CUBICSPLINE": "CUBICSPLINE"}[
                        sampler_entry.get("interpolation", "LINEAR")
                    ],
                    "times": times,
                    "values": {"path": target["path"], "values": values},
                    "inTangents": None,
                    "outTangents": None,
                    "tangentModes": [],
                }
            )
        animations.append(
            {
                "name": args.animation_name,
                "duration": duration,
                "declaredDuration": duration,
                "keyedDuration": duration,
                "eventDuration": None,
                "sampleRate": 30.0,
                "wrapMode": 2,
                "looped": True,
                "channels": channels,
                "metadata": {
                    "floatCurves": [],
                    "objectCurves": [],
                    "events": [],
                    "emptyTrsBindings": [],
                    "duplicateTrsBindings": [],
                    "timeRecoveries": [],
                    "curveRecoveries": [],
                    "unsupported": [],
                },
            }
        )

    model = {
        "schema": "ffone.skinned-model.v1",
        "name": args.name,
        "nativeCoordinateContract": json.loads(
            (Path(__file__).with_name("native-coordinate-contract.json")).read_text("utf-8")
        ),
        "roots": roots,
        "nodes": nodes,
        "meshes": meshes,
        "skins": skins,
        "materials": materials,
        "textures": textures,
        "samplers": samplers,
        "animations": animations,
    }

    out = args.out
    (out / "models" / "npc").mkdir(parents=True, exist_ok=True)
    closure = out / "models" / "npc" / texture_dir
    closure.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(args.texture, closure / f"{args.name}.png")
    if args.ramp is not None:
        shutil.copyfile(args.ramp, closure / args.ramp.name)
    model_path = out / f"{args.name}.native-model.json"
    model_path.write_text(json.dumps(model, indent=1) + "\n", "utf-8")

    print(f"wrote {model_path}")
    print(f"  nodes={len(nodes)} roots={roots} joints={len(skins[0]['joints'])}")
    print(f"  vertices={len(positions)} triangles={len(indices) // 3}")
    print(f"  textures={[t['uri'] for t in textures]}")
    print(f"  animations={[(c['name'], len(c['channels'])) for c in animations]}")
    for record in repaired_windings:
        print(
            f"  winding triangle {record['triangle']} verts={record['vertices']} "
            f"cross={record['crossProductLength']:.6f} dot={record['normalAlignment']:.8f} "
            f"flipped={record['flipped']}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
