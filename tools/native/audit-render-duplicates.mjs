#!/usr/bin/env node
// Read-only audit of editable native assets. Reports are derived output, never runtime inputs.
import { createHash } from 'node:crypto';
import { mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { basename, dirname, extname, join, relative, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const canonical = value => JSON.stringify(sort(value));
function sort(value) {
  if (Array.isArray(value)) return value.map(sort);
  if (value && typeof value === 'object') return Object.fromEntries(
    Object.keys(value).sort().map(key => [key, sort(value[key])]),
  );
  return value;
}
function files(root) {
  return readdirSync(root, { withFileTypes: true }).flatMap(entry => {
    const path = join(root, entry.name);
    return entry.isDirectory() ? files(path) : [path];
  }).sort();
}
function record(groups, key, path, bytes = 0) {
  const group = groups.get(key) ?? { copies: 0, bytes: 0, examples: [] };
  group.copies++;
  group.bytes += bytes;
  if (group.examples.length < 5) group.examples.push(path);
  groups.set(key, group);
}
function summarize(groups) {
  const duplicates = [...groups.values()].filter(group => group.copies > 1);
  return {
    entries: [...groups.values()].reduce((sum, group) => sum + group.copies, 0),
    unique: groups.size,
    duplicateEntries: duplicates.reduce((sum, group) => sum + group.copies - 1, 0),
    redundantBytes: duplicates.reduce((sum, group) => sum + group.bytes * (group.copies - 1) / group.copies, 0),
    largestGroups: duplicates.sort((a, b) => b.copies - a.copies).slice(0, 20),
  };
}
export function audit(root) {
  root = resolve(root);
  const pngs = new Map(), glbs = new Map(), geometry = new Map(), visualGeometry = new Map(), materials = new Map();
  const imageHashes = new Map();
  const placements = new Map();
  const paths = files(root);
  const name = path => relative(root, path).replaceAll('\\', '/');
  for (const path of paths.filter(path => extname(path) === '.png')) {
    const bytes = readFileSync(path), digest = hash(bytes);
    imageHashes.set(path, digest);
    record(pngs, digest, name(path), bytes.length);
  }
  let staticGlbs = 0, unsupportedGeometry = 0;
  for (const path of paths.filter(path => extname(path) === '.glb')) {
    const bytes = readFileSync(path);
    if (bytes.toString('ascii', 0, 4) !== 'glTF' || bytes.readUInt32LE(4) !== 2
      || bytes.readUInt32LE(8) !== bytes.length) throw new Error(`Invalid GLB: ${path}`);
    record(glbs, hash(bytes), name(path), bytes.length);
    const jsonLength = bytes.readUInt32LE(12);
    const doc = JSON.parse(bytes.subarray(20, 20 + jsonLength));
    // Inspect the existing native static geometry contract only. Skins, animation,
    // entity ownership and transforms are deliberately outside this equivalence.
    if (doc.asset?.extras?.schema !== 'ffone.native-static-world-export.v1') continue;
    staticGlbs++;
    const binary = bytes.subarray(28 + jsonLength);
    function accessor(index) {
      const a = doc.accessors[index], view = doc.bufferViews?.[a?.bufferView];
      const components = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4 }[a?.type];
      const width = { 5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4 }[a?.componentType];
      if (!view || a.sparse || view.buffer !== 0 || !components || !width
        || doc.buffers[0].uri) throw new Error('Unsupported accessor');
      const size = components * width, start = (view.byteOffset ?? 0) + (a.byteOffset ?? 0);
      const packed = Buffer.alloc(a.count * size);
      for (let i = 0; i < a.count; i++) {
        const offset = start + i * (view.byteStride ?? size);
        if (offset + size > binary.length) throw new Error('Invalid accessor bounds');
        binary.copy(packed, i * size, offset, offset + size);
      }
      return { componentType: a.componentType, type: a.type, count: a.count,
        normalized: a.normalized ?? false, data: hash(packed) };
    }
    for (const [mi, mesh] of (doc.meshes ?? []).entries()) {
      for (const [pi, primitive] of mesh.primitives.entries()) {
        try {
          if (primitive.targets || primitive.extensions) throw new Error('Unsupported geometry');
          const payload = { mode: primitive.mode ?? 4,
            attributes: Object.fromEntries(Object.entries(primitive.attributes).map(([k, v]) => [k, accessor(v)])),
            indices: primitive.indices === undefined ? null : accessor(primitive.indices) };
          record(geometry, hash(canonical(payload)), `${name(path)}#mesh${mi}/${pi}`);
          if (basename(path) === 'visual.glb') record(visualGeometry, hash(canonical(payload)), `${name(path)}#mesh${mi}/${pi}`);
        } catch { unsupportedGeometry++; }
      }
    }
    const texture = index => {
      const tex = doc.textures[index], image = doc.images[tex.source];
      const imagePath = image.uri ? resolve(dirname(path), decodeURIComponent(image.uri)) : null;
      return { image: imageHashes.get(imagePath) ?? { model: name(path), image: tex.source },
        sampler: doc.samplers?.[tex.sampler] ?? {}, extensions: tex.extensions ?? null };
    };
    for (const [index, material] of (doc.materials ?? []).entries()) {
      const contract = structuredClone(material);
      // Names and source identities are audit metadata, never proof of equivalence.
      delete contract.name;
      if (contract.extras) {
        delete contract.extras.ffoneSourceMaterialId;
        delete contract.extras.legacyRenderState; // shaderName retains the exact native shader contract
        for (const slot of contract.extras.exactTextureSlots ?? []) delete slot.textureId;
      }
      function resolveTextures(value) {
        if (!value || typeof value !== 'object') return;
        for (const [key, child] of Object.entries(value)) {
          if (key.endsWith('Texture') && child && Number.isInteger(child.index)) {
            child.resolved = texture(child.index);
            delete child.index;
          } else resolveTextures(child);
        }
      }
      resolveTextures(contract);
      record(materials, hash(canonical(contract)), `${name(path)}#material${index}`);
    }
  }
  for (const path of paths.filter(path => basename(path) === 'scene.json')) {
    const doc = JSON.parse(readFileSync(path));
    if (!['ffone.native-world-scene.v1', 'ffone.native-world-scene.v2'].includes(doc.schema)) continue;
    const models = new Map(doc.models.map(model => [model.id, model.path]));
    for (const [index, visual] of doc.visuals.entries()) {
      const model = models.get(visual.model);
      if (!model) throw new Error(`Unresolved model in ${path}: ${visual.model}`);
      const identity = { scene: name(path), model, transform: visual.transform,
        gltfScene: visual.scene, layer: visual.legacyLayer };
      record(placements, hash(canonical(identity)), `${name(path)}#visual${index}`);
    }
  }
  return { schema: 'ffone.native-render-duplicates.v1',
    note: 'Read-only opportunities; PNG equality is encoded-byte equality. Material candidates require runtime value/texture/sampler checks. Geometry equality never authorizes deleting scene entities.',
    pngs: summarize(pngs), glbs: summarize(glbs), staticGlbs,
    staticGeometry: summarize(geometry), unsupportedGeometry,
    staticVisualGeometry: summarize(visualGeometry),
    coincidentPlacementCandidates: summarize(placements),
    staticMaterialCandidates: summarize(materials) };
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const editorRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
  const report = audit(process.argv[2] ?? join(editorRoot, '../FFOneClient/assets/game'));
  if (!process.argv[3]) { console.log(JSON.stringify(report, null, 2)); process.exit(0); }
  const output = resolve(process.argv[3]);
  mkdirSync(dirname(output), { recursive: true });
  writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify(Object.fromEntries(Object.entries(report).map(([k, v]) =>
    [k, v?.largestGroups ? { ...v, largestGroups: undefined } : v])), null, 2));
  console.log(`Report: ${output}`);
}
