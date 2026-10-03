#!/usr/bin/env node

/**
 * Builds a local GPU gallery for the complete patched-XDT Nano table plus
 * explicitly published runtime extensions. Legacy bundles remain offline
 * evidence; the renderer opens only assets/game GLB/PNG files.
 */

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const SCHEMA = "ffone.xdt-nano-render-gallery.v1";
const REGISTRY_SCHEMA = "ffone.semantic-character-registry.v2";

function fail(message) {
  throw new Error(message);
}

function parseArgs(argv) {
  const options = {
    frames: 900,
    timeout: 45,
    resume: false,
    maxModels: Number.POSITIVE_INFINITY,
    textureMetadata: [],
    command: [process.execPath, ...process.argv.slice(1)],
  };
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index];
    if (flag === "--resume") {
      options.resume = true;
      continue;
    }
    if (flag === "-h" || flag === "--help") {
      console.log(`FFOne XDT Nano render gallery

Usage:
  node tools/legacy-sources/ffone-migration/render_xdt_nano_gallery.mjs --xdt <JSON> --asset-root <DIR> \\
    --output <DIR> --preview <EXE> --texture-metadata <JSON> [OPTIONS]

Options:
  --source-manifest <JSON>  primary logical-model export manifest for blockers
  --texture-metadata <JSON> exact primary Texture2D sampler metadata; repeatable
  --frames <COUNT>          GPU frame budget per appearance (default: 900)
  --timeout <SECONDS>       GPU timeout per appearance (default: 45)
  --max-models <COUNT>      bounded smoke render
  --resume                  reuse complete render outputs
`);
      process.exit(0);
    }
    const value = argv[index + 1];
    if (value === undefined) fail(`${flag} requires a value`);
    index += 1;
    switch (flag) {
      case "--xdt": options.xdt = value; break;
      case "--asset-root": options.assetRoot = value; break;
      case "--output": options.output = value; break;
      case "--preview": options.preview = value; break;
      case "--source-manifest": options.sourceManifest = value; break;
      case "--texture-metadata": options.textureMetadata.push(value); break;
      case "--frames": options.frames = positiveNumber(value, flag); break;
      case "--timeout": options.timeout = positiveNumber(value, flag); break;
      case "--max-models": options.maxModels = positiveInteger(value, flag); break;
      default: fail(`unknown option ${JSON.stringify(flag)}`);
    }
  }
  for (const key of ["xdt", "assetRoot", "output", "preview"]) {
    if (!options[key]) fail(`--${key.replace(/[A-Z]/g, c => `-${c.toLowerCase()}`)} is required`);
  }
  options.xdt = requiredFile(options.xdt, "--xdt");
  options.assetRoot = requiredDirectory(options.assetRoot, "--asset-root");
  options.preview = requiredFile(options.preview, "--preview");
  options.sourceManifest = options.sourceManifest
    ? requiredFile(options.sourceManifest, "--source-manifest")
    : null;
  options.textureMetadata = options.textureMetadata.map(file => requiredFile(file, "--texture-metadata"));
  options.output = path.resolve(options.output);
  if (fs.existsSync(options.output) && !options.resume) {
    fail(`output already exists; choose a fresh directory or pass --resume: ${options.output}`);
  }
  fs.mkdirSync(options.output, { recursive: true });
  return options;
}

function positiveNumber(value, flag) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) fail(`${flag} must be a positive number`);
  return parsed;
}

function positiveInteger(value, flag) {
  const parsed = positiveNumber(value, flag);
  if (!Number.isInteger(parsed)) fail(`${flag} must be an integer`);
  return parsed;
}

function requiredFile(input, flag) {
  const resolved = path.resolve(input);
  if (!fs.statSync(resolved, { throwIfNoEntry: false })?.isFile()) fail(`${flag} is not a file: ${resolved}`);
  return resolved;
}

function requiredDirectory(input, flag) {
  const resolved = path.resolve(input);
  if (!fs.statSync(resolved, { throwIfNoEntry: false })?.isDirectory()) fail(`${flag} is not a directory: ${resolved}`);
  return resolved;
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function slash(file) {
  return file.replaceAll("\\", "/");
}

function relative(root, file) {
  const result = slash(path.relative(root, file));
  if (!result || result.startsWith("../") || path.isAbsolute(result)) fail(`path escaped output root: ${file}`);
  return result;
}

function normalizedTexture(value) {
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  return !trimmed || trimmed.toLowerCase() === "null" ? null : trimmed;
}

function normalizedModel(value) {
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  return !trimmed || trimmed.toLowerCase() === "null" ? null : trimmed;
}

function indexRegistry(registry) {
  if (registry.schema !== REGISTRY_SCHEMA) fail(`unexpected runtime registry schema ${JSON.stringify(registry.schema)}`);
  const nanos = registry.models.filter(model => model.category === "nano");
  const byStem = new Map();
  for (const model of nanos) {
    for (const candidate of [model.logicalName, model.id.split("/").at(-1), ...(model.legacyAliases ?? [])]) {
      if (!candidate) continue;
      const key = candidate.toLowerCase();
      const previous = byStem.get(key);
      if (previous && previous.id !== model.id) fail(`contradictory Nano registry alias ${candidate}`);
      byStem.set(key, model);
    }
  }
  return { nanos, byStem };
}

function sourceEquivalent(left, right) {
  return left.sourceChainSha256 === right.sourceChainSha256
    && left.nativePngBlake3 === right.nativePngBlake3
    && left.filterMode === right.filterMode
    && left.wrapMode === right.wrapMode
    && left.anisotropyLevel === right.anisotropyLevel
    && left.mipMapBias === right.mipMapBias
    && left.sourceMipCount === right.sourceMipCount;
}

function nativeSampler(source) {
  const hasMips = source.sourceMipCount > 1;
  if (!Number.isInteger(source.sourceMipCount) || source.sourceMipCount <= 0) return null;
  if (!source.mipMap && source.sourceMipCount !== 1) return null;
  if (source.mipMapBias !== 0 || source.anisotropyLevel < 0 || source.anisotropyLevel > 65535) return null;
  let minFilter;
  if (source.filterMode === 0) minFilter = hasMips ? "nearestMipmapNearest" : "nearest";
  else if (source.filterMode === 1) minFilter = hasMips ? "linearMipmapNearest" : "linear";
  else if (source.filterMode === 2) minFilter = hasMips ? "linearMipmapLinear" : "linear";
  else return null;
  const wrap = source.wrapMode === 0 ? "repeat" : source.wrapMode === 1 ? "clampToEdge" : null;
  if (!wrap) return null;
  return {
    name: source.trueName,
    magFilter: source.filterMode === 0 ? "nearest" : "linear",
    minFilter,
    wrapS: wrap,
    wrapT: wrap,
    legacyFilterMode: source.filterMode,
    legacyWrapMode: source.wrapMode,
    anisotropyLevel: source.anisotropyLevel,
    mipMapBias: source.mipMapBias,
  };
}

function buildTextureCatalog(options) {
  const runtimeByName = new Map();
  const runtimeByDigest = new Map();
  const textureRoot = path.join(options.assetRoot, "textures");
  for (const entry of fs.readdirSync(textureRoot, { withFileTypes: true })) {
    if (!entry.isFile() || path.extname(entry.name).toLowerCase() !== ".png") continue;
    const match = /^(.*)--([0-9a-f]{16})\.png$/i.exec(entry.name);
    if (!match) continue;
    const runtimePath = `textures/${entry.name}`;
    pushMap(runtimeByName, match[1].toLowerCase(), runtimePath);
    pushMap(runtimeByDigest, match[2].toLowerCase(), runtimePath);
  }
  const sourcesByName = new Map();
  const sourcesByRoute = new Map();
  const proofs = [];
  for (const file of options.textureMetadata) {
    const bytes = fs.readFileSync(file);
    const document = JSON.parse(bytes);
    if (document.schema !== "ffone.offline.chartexture-metadata.v1") fail(`unexpected texture metadata schema in ${file}`);
    proofs.push({
      metadataPath: slash(file),
      metadataBytes: bytes.length,
      metadataSha256: sha256(bytes),
      sourcePath: document.sourcePath,
      sourceFileBytes: document.sourceFileBytes,
      sourceFileSha256: document.sourceFileSha256,
      sourceAsset: document.sourceAsset,
      rawBundle: document.rawBundle,
      textureCount: document.textures.length,
    });
    for (const source of document.textures) {
      pushMap(sourcesByName, source.trueName.toLowerCase(), source);
      for (const route of source.containerRoutes ?? []) pushMap(sourcesByRoute, slash(route).toLowerCase(), source);
    }
  }
  function resolve(trueName) {
    if (!trueName) return null;
    const key = trueName.toLowerCase();
    const exactRoutes = [`texture/${key}.dds`, `textures/${key}.dds`, `texture/${key}.dds.asset`, `textures/${key}.dds.asset`];
    const exact = exactRoutes.flatMap(route => sourcesByRoute.get(route) ?? []);
    const named = sourcesByName.get(key) ?? [];
    let candidates = exact.length ? exact : named;
    candidates = [...new Map(candidates.map(source => [`${source.sourceChainSha256}:${source.pathId}`, source])).values()];
    let source = null;
    if (candidates.length === 1) source = candidates[0];
    else if (candidates.length > 1 && candidates.slice(1).every(candidate => sourceEquivalent(candidates[0], candidate))) source = candidates[0];
    if (!source) return { trueName, status: candidates.length ? "source_ambiguous" : "source_missing", path: null, sampler: null, candidates: [] };
    const digest = source.nativePngBlake3.slice(0, 16).toLowerCase();
    const runtimeCandidates = [...new Set([...(runtimeByName.get(key) ?? []), ...(runtimeByDigest.get(digest) ?? [])])].sort();
    const expectedSuffix = `--${digest}.png`;
    const runtimePath = runtimeCandidates.find(candidate => candidate.toLowerCase() === `textures/${key}${expectedSuffix}`)
      ?? runtimeCandidates.find(candidate => candidate.toLowerCase().endsWith(expectedSuffix));
    const sampler = nativeSampler(source);
    return {
      trueName,
      status: !runtimePath ? "runtime_png_missing" : !sampler ? "sampler_unsupported" : "resolved_primary_exact",
      path: runtimePath ?? null,
      sampler,
      candidates: runtimeCandidates,
      pathId: source.pathId,
      sourceChainSha256: source.sourceChainSha256,
      nativePngBlake3: source.nativePngBlake3,
      containerRoutes: source.containerRoutes,
    };
  }
  return { resolve, proofs };
}

function pushMap(map, key, value) {
  const bucket = map.get(key) ?? [];
  bucket.push(value);
  map.set(key, bucket);
}

function parseGlbAudit(assetRoot, model) {
  const file = path.join(assetRoot, ...model.glb.split("/"));
  if (!fs.statSync(file, { throwIfNoEntry: false })?.isFile()) {
    return { status: "glb_missing", file: slash(file), missingImages: [], materials: [], images: [] };
  }
  const bytes = fs.readFileSync(file);
  if (bytes.length < 20 || bytes.toString("ascii", 0, 4) !== "glTF") {
    return { status: "glb_invalid", file: slash(file), missingImages: [], materials: [], images: [] };
  }
  const jsonLength = bytes.readUInt32LE(12);
  if (bytes.toString("ascii", 16, 20) !== "JSON" || 20 + jsonLength > bytes.length) {
    return { status: "glb_invalid", file: slash(file), missingImages: [], materials: [], images: [] };
  }
  const gltf = JSON.parse(bytes.subarray(20, 20 + jsonLength).toString("utf8").replace(/\0+$/u, ""));
  const images = (gltf.images ?? []).map(image => image.uri ?? `[embedded:${image.name ?? "unnamed"}]`);
  const missingImages = images.filter(uri => !uri.startsWith("[embedded:") && !fs.statSync(path.join(path.dirname(file), ...uri.split("/")), { throwIfNoEntry: false })?.isFile());
  const materials = (gltf.materials ?? []).map(material => material.name ?? "");
  return {
    status: missingImages.length ? "glb_dependency_missing" : "complete",
    file: slash(file),
    bytes: bytes.length,
    sha256: sha256(bytes),
    materialCount: materials.length,
    imageCount: images.length,
    hasMainMaterial: materials.some(name => /(^|[-_])main([-_.]|$)/i.test(name)),
    hasSubMaterial: materials.some(name => /(^|[-_])sub([-_.]|$)/i.test(name)),
    materials,
    images,
    missingImages,
  };
}

function loadBlockers(file) {
  const result = new Map();
  if (!file) return result;
  const manifest = readJson(file);
  for (const entry of manifest.blocked ?? []) {
    const route = (entry.normalizedRoute ?? entry.exactRoute ?? "").toLowerCase();
    if (route) result.set(route, { code: entry.code, detail: entry.detail });
  }
  return result;
}

function selectAnimation(animations) {
  return animations.find(name => name.toLowerCase() === "stand1")
    ?? animations.find(name => name.toLowerCase() === "stand")
    ?? animations.find(name => name.toLowerCase() === "idle")
    ?? animations[0]
    ?? null;
}

function safeName(value) {
  return value.toLowerCase().replace(/[^a-z0-9_-]+/g, "-").replace(/^-+|-+$/g, "") || "unnamed";
}

function appearanceKey(record) {
  return JSON.stringify({
    id: record.registry.id,
    glbBlake3: record.registry.glbBlake3,
    main: record.mainTexture?.path ?? null,
    sub: record.subTexture?.path ?? null,
    mainSampler: record.mainTexture?.sampler ?? null,
    subSampler: record.subTexture?.sampler ?? null,
  });
}

function makeRecords(options, xdt, registryIndex, textures, blockers) {
  const table = xdt.m_pNanoTable;
  if (!table) fail("XDT has no m_pNanoTable");
  const rows = table.m_pNanoData;
  const meshes = table.m_pNanoMeshData;
  const strings = table.m_pNanoStringData;
  if (!Array.isArray(rows) || !Array.isArray(meshes) || !Array.isArray(strings)) fail("XDT Nano arrays are missing");
  const records = [];
  const xdtStems = new Set();
  for (let rowIndex = 0; rowIndex < rows.length; rowIndex += 1) {
    const row = rows[rowIndex];
    const meshIndex = Number(row.m_iMesh);
    const mesh = Number.isInteger(meshIndex) && meshIndex >= 0 ? meshes[meshIndex] : null;
    const modelStem = normalizedModel(mesh?.m_pstrMMeshModelString);
    if (!modelStem) continue;
    xdtStems.add(modelStem.toLowerCase());
    const registry = registryIndex.byStem.get(modelStem.toLowerCase()) ?? null;
    const string = strings[Number(row.m_iNanoName)] ?? {};
    const nanoNumber = Number(row.m_iNanoNumber) || 0;
    const legacyRoute = `nano/${modelStem.toLowerCase()}.kfm`;
    const mainTexture = textures.resolve(normalizedTexture(mesh.m_pstrMTextureString));
    const subTexture = textures.resolve(normalizedTexture(mesh.m_pstrMTextureString2));
    const glbAudit = registry ? parseGlbAudit(options.assetRoot, registry) : null;
    const blocker = blockers.get(legacyRoute) ?? null;
    const materialWarnings = [];
    if (mainTexture && glbAudit?.status === "complete" && !glbAudit.hasMainMaterial) {
      materialWarnings.push("main_no_matching_material_unity_noop");
    }
    if (subTexture && glbAudit?.status === "complete" && !glbAudit.hasSubMaterial) {
      materialWarnings.push("sub_no_matching_material_unity_noop");
    }
    let status = "runtime_available";
    if (!registry) status = blocker ? "primary_blocked" : "missing_runtime_model";
    else if (glbAudit.status !== "complete") status = glbAudit.status;
    else if (mainTexture && mainTexture.status !== "resolved_primary_exact") status = "texture_unresolved";
    else if (subTexture && subTexture.status !== "resolved_primary_exact") status = "texture_unresolved";
    records.push({
      source: "xdt",
      rowIndex,
      nanoNumber,
      nanoName: string.m_strName || modelStem.replace(/^nano_/i, ""),
      meshIndex,
      modelStem,
      legacyRoute,
      xdtTexture: normalizedTexture(mesh.m_pstrMTextureString),
      xdtTexture2: normalizedTexture(mesh.m_pstrMTextureString2),
      mainTexture,
      subTexture,
      registry,
      glbAudit,
      blocker,
      materialWarnings,
      selectedAnimation: registry ? selectAnimation(registry.animations) : null,
      status,
      image: null,
      report: null,
      log: null,
    });
  }
  for (const registry of registryIndex.nanos) {
    const identities = [registry.logicalName, registry.id.split("/").at(-1), ...(registry.legacyAliases ?? [])]
      .filter(Boolean).map(value => value.toLowerCase());
    if (identities.some(identity => xdtStems.has(identity))) continue;
    const glbAudit = parseGlbAudit(options.assetRoot, registry);
    const modelStem = registry.id.split("/").at(-1);
    // Some clean-primary Nano packages predate a table row but still carry the
    // same link-material contract as table-owned Nanos. Accept a default
    // extension appearance only when the exact primary catalog contains both
    // canonical companion textures and the GLB proves both matching roles.
    // Today this closes Nano Ben (`nano_ben` -> main, `nano_ben_face` -> sub)
    // without baking a guessed texture into the reusable source model.
    const faceCandidate = textures.resolve(`${modelStem}_face`);
    const bodyCandidate = textures.resolve(modelStem);
    const hasPrimaryCompanionPair = glbAudit.status === "complete"
      && glbAudit.hasMainMaterial
      && glbAudit.hasSubMaterial
      && faceCandidate?.status === "resolved_primary_exact"
      && bodyCandidate?.status === "resolved_primary_exact";
    const mainTexture = hasPrimaryCompanionPair ? bodyCandidate : null;
    const subTexture = hasPrimaryCompanionPair ? faceCandidate : null;
    records.push({
      source: "runtime_extension",
      rowIndex: null,
      nanoNumber: null,
      nanoName: modelStem.replace(/^nano_/, ""),
      meshIndex: null,
      modelStem,
      legacyRoute: null,
      xdtTexture: null,
      xdtTexture2: null,
      mainTexture,
      subTexture,
      extensionTextureBinding: hasPrimaryCompanionPair
        ? "exact_primary_canonical_face_body_pair"
        : null,
      registry,
      glbAudit,
      blocker: null,
      materialWarnings: [],
      selectedAnimation: selectAnimation(registry.animations),
      status: glbAudit.status === "complete" ? "runtime_extension" : glbAudit.status,
      image: null,
      report: null,
      log: null,
    });
  }
  return { records, xdtRows: rows.length };
}

function renderRecords(options, records) {
  const tasks = new Map();
  for (const record of records) {
    if (!record.registry || !["runtime_available", "runtime_extension"].includes(record.status)) continue;
    const key = appearanceKey(record);
    record.appearanceKey = key;
    if (!tasks.has(key)) tasks.set(key, record);
  }
  let renderIndex = 0;
  for (const [key, representative] of tasks) {
    const slug = safeName(representative.registry.id);
    const digest = sha256(Buffer.from(key)).slice(0, 12);
    const base = path.join(options.output, "appearances", `${slug}--${digest}`);
    const screenshot = `${base}.png`;
    const report = `${base}.report.json`;
    const log = `${base}.log.txt`;
    fs.mkdirSync(path.dirname(base), { recursive: true });
    let status;
    if (renderIndex >= options.maxModels) status = "not_rendered_limit";
    else if (options.resume && fs.statSync(screenshot, { throwIfNoEntry: false })?.size > 0 && fs.statSync(report, { throwIfNoEntry: false })?.isFile()) status = "rendered";
    else {
      if (fs.existsSync(screenshot) || fs.existsSync(report) || fs.existsSync(log)) fail(`partial appearance output exists: ${base}`);
      const args = [
        "--asset-root", options.assetRoot,
        "--model", representative.registry.glb,
        "--screenshot", screenshot,
        "--character-kind", "nano",
        "--true-root", representative.registry.logicalName,
        "--camera-view", "reverse",
        "--blank-camera-retry", "same",
        "--report", report,
        "--frames", String(options.frames),
        "--timeout", String(options.timeout),
        "--outline", "source",
      ];
      if (representative.selectedAnimation) args.push("--animation-name", representative.selectedAnimation);
      if (representative.mainTexture?.path) args.push("--main-texture", representative.mainTexture.path, "--main-sampler", JSON.stringify(representative.mainTexture.sampler));
      if (representative.subTexture?.path) args.push("--sub-texture", representative.subTexture.path, "--sub-sampler", JSON.stringify(representative.subTexture.sampler));
      console.log(`[${renderIndex + 1}/${tasks.size}] ${representative.registry.id}`);
      const result = spawnSync(options.preview, args, { encoding: "utf8", windowsHide: true, timeout: Math.ceil((options.timeout + 15) * 1000) });
      fs.writeFileSync(log, `command=${JSON.stringify([options.preview, ...args])}\nexit=${result.status}\nsignal=${result.signal}\nerror=${result.error?.stack ?? ""}\n\n--- stdout ---\n${result.stdout ?? ""}\n\n--- stderr ---\n${result.stderr ?? ""}\n`);
      status = result.status === 0 && fs.statSync(screenshot, { throwIfNoEntry: false })?.size > 0 && fs.statSync(report, { throwIfNoEntry: false })?.isFile()
        ? "rendered"
        : "render_failed";
    }
    for (const record of records.filter(candidate => candidate.appearanceKey === key)) {
      record.status = status;
      record.image = status === "rendered" ? relative(options.output, screenshot) : null;
      record.report = relative(options.output, report);
      record.log = relative(options.output, log);
    }
    renderIndex += 1;
  }
  return { uniqueAppearances: tasks.size };
}

function linkEntityImages(options, records) {
  const entityRoot = path.join(options.output, "entities");
  fs.mkdirSync(entityRoot, { recursive: true });
  for (const record of records) {
    if (!record.image) continue;
    const name = record.source === "xdt"
      ? `row-${String(record.rowIndex).padStart(3, "0")}_nano-${String(record.nanoNumber).padStart(3, "0")}.png`
      : `extension-${safeName(record.modelStem)}.png`;
    const destination = path.join(entityRoot, name);
    const source = path.join(options.output, ...record.image.split("/"));
    if (fs.existsSync(destination)) {
      const sourceStat = fs.statSync(source);
      const destinationStat = fs.statSync(destination);
      if (sourceStat.dev !== destinationStat.dev || sourceStat.ino !== destinationStat.ino) {
        // Entity images are generated links, not evidence originals. Refresh a
        // stale link when --resume produces a new appearance identity.
        fs.unlinkSync(destination);
      }
    }
    if (!fs.existsSync(destination)) {
      try { fs.linkSync(source, destination); }
      catch { fs.copyFileSync(source, destination); }
    }
    record.image = relative(options.output, destination);
  }
}

function countBy(values) {
  const result = {};
  for (const value of values) result[value] = (result[value] ?? 0) + 1;
  return result;
}

function inputProof(alias, file) {
  const bytes = fs.readFileSync(file);
  return { alias, path: slash(file), bytes: bytes.length, sha256: sha256(bytes) };
}

function writeJson(file, value) {
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function htmlEscape(value) {
  return String(value).replace(/[&<>"']/g, char => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[char]);
}

function writeHtml(file, summary, records) {
  const data = JSON.stringify(records).replaceAll("<", "\\u003c");
  const stats = `${summary.counts.records} cards · ${summary.counts.rendered} rendered · ${summary.counts.xdtUniqueModels} XDT models · ${summary.counts.runtimeExtensions} extensions · ${summary.counts.blockedOrMissing} blocked/missing`;
  fs.writeFileSync(file, `<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>FFOne XDT Nano render audit</title><style>
:root{color-scheme:dark;font-family:Segoe UI,Arial,sans-serif;background:#0d131d;color:#e7edf6}*{box-sizing:border-box}body{margin:0}header{position:sticky;top:0;z-index:3;padding:15px 18px;background:#111a27f2;border-bottom:1px solid #33445b}h1{margin:0 0 6px;font-size:22px}.stats{color:#a9bad0;margin-bottom:10px}.filters{display:flex;gap:8px;flex-wrap:wrap}input,select{padding:8px 10px;background:#1b2737;color:#fff;border:1px solid #40536d;border-radius:6px}main{display:grid;grid-template-columns:repeat(auto-fill,minmax(230px,1fr));gap:12px;padding:14px}.card{overflow:hidden;background:#15202e;border:1px solid #2b3b50;border-radius:9px}.card img,.missing{display:block;width:100%;aspect-ratio:1;object-fit:contain;background:#101821}.missing{display:grid;place-items:center;color:#ff9aa8;background:repeating-linear-gradient(135deg,#321821,#321821 18px,#26141a 18px,#26141a 36px);font-weight:800}.meta{padding:10px}.name{font-weight:750}.route,.detail{font-size:12px;color:#a9bad0;overflow-wrap:anywhere}.status{display:inline-block;margin:7px 0 5px;padding:3px 7px;border-radius:4px;background:#32445c;font-size:11px}.rendered .status{background:#176b4a}.primary_blocked .status,.missing_runtime_model .status,.render_failed .status,.texture_unresolved .status,.main_material_missing .status,.sub_material_missing .status{background:#8b303f}.runtime_extension .status{background:#285c78}.chip{display:inline-block;margin:2px 3px 0 0;padding:2px 5px;border-radius:4px;background:#26374b;font-size:10px;color:#c6d4e5}</style></head><body><header><h1>FFOne XDT Nano render audit</h1><div class="stats">${htmlEscape(stats)}</div><div class="filters"><input id="q" placeholder="Nano, model, texture, status"><select id="source"><option value="all">All sources</option><option value="xdt">XDT</option><option value="runtime_extension">Runtime extensions</option></select><select id="status"><option value="all">All statuses</option>${Object.keys(summary.statusCounts).sort().map(status => `<option>${htmlEscape(status)}</option>`).join("")}</select></div></header><main id="grid"></main><script>const rows=${data};const q=document.querySelector('#q'),source=document.querySelector('#source'),status=document.querySelector('#status'),grid=document.querySelector('#grid');function esc(v){return String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]))}function draw(){const term=q.value.toLowerCase();const filtered=rows.filter(x=>(source.value==='all'||x.source===source.value)&&(status.value==='all'||x.status===status.value)&&JSON.stringify(x).toLowerCase().includes(term));grid.innerHTML=filtered.map(x=>\`<article class="card \${esc(x.status)}">\${x.image?\`<img loading="lazy" src="\${encodeURI(x.image)}" alt="\${esc(x.nanoName)}">\`:\`<div class="missing">NO GPU RENDER</div>\`}<div class="meta"><div class="name">\${x.nanoNumber?\`#\${x.nanoNumber} · \`:''}\${esc(x.nanoName)}</div><div class="status">\${esc(x.status)}</div><div><span class="chip">\${esc(x.source)}</span><span class="chip">\${esc(x.selectedAnimation||'static')}</span></div><div class="route">\${esc(x.modelStem)}<br>\${esc(x.registry?.glb||x.legacyRoute||'')}</div><div class="detail">main: \${esc(x.mainTexture?.trueName||x.xdtTexture||'embedded')}<br>sub: \${esc(x.subTexture?.trueName||x.xdtTexture2||'none/embedded')}\${x.extensionTextureBinding?\`<br>binding: \${esc(x.extensionTextureBinding)}\`:''}<br>materials: \${esc(x.glbAudit?.materialCount??'—')} · images: \${esc(x.glbAudit?.imageCount??'—')}\${x.blocker?\`<br>\${esc(x.blocker.code)}: \${esc(x.blocker.detail)}\`:''}</div></div></article>\`).join('')}q.addEventListener('input',draw);source.addEventListener('change',draw);status.addEventListener('change',draw);draw();</script></body></html>`);
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const xdt = readJson(options.xdt);
  const registryPath = path.join(options.assetRoot, "_runtime", "characters.json");
  const registry = indexRegistry(readJson(registryPath));
  const textures = buildTextureCatalog(options);
  const blockers = loadBlockers(options.sourceManifest);
  const { records, xdtRows } = makeRecords(options, xdt, registry, textures, blockers);
  const render = renderRecords(options, records);
  linkEntityImages(options, records);
  const statusCounts = countBy(records.map(record => record.status));
  const xdtUniqueModels = new Set(records.filter(record => record.source === "xdt").map(record => record.modelStem.toLowerCase())).size;
  const summary = {
    schema: SCHEMA,
    status: statusCounts.render_failed ? "generated_with_render_failures" : "generated",
    inputs: {
      xdt: inputProof("patched", options.xdt),
      runtimeRegistry: inputProof("native-runtime", registryPath),
      sourceManifest: options.sourceManifest ? inputProof("primary-derived", options.sourceManifest) : null,
      textureMetadata: options.textureMetadata.map(file => inputProof("primary", file)),
    },
    command: options.command,
    counts: {
      xdtRows,
      records: records.length,
      xdtRecords: records.filter(record => record.source === "xdt").length,
      xdtUniqueModels,
      runtimeExtensions: records.filter(record => record.source === "runtime_extension").length,
      uniqueAppearances: render.uniqueAppearances,
      rendered: records.filter(record => record.status === "rendered").length,
      blockedOrMissing: records.filter(record => ["primary_blocked", "missing_runtime_model"].includes(record.status)).length,
      textureIssues: records.filter(record => record.status === "texture_unresolved").length,
      materialWarnings: records.filter(record => record.materialWarnings.length > 0).length,
      glbDependencyIssues: records.filter(record => /glb_/.test(record.status)).length,
      renderFailures: records.filter(record => record.status === "render_failed").length,
    },
    statusCounts,
    presentation: {
      cameraView: "reverse",
      blankCameraRetry: "same",
      characterKind: "nano",
      characterRuntimeHalfTurn: true,
      intent: "front-facing Nano audit after the exact gameplay character half-turn",
    },
    notes: [
      "Patched-XDT Nano rows use their exact main/sub Texture2D replacements and primary sampler metadata.",
      "Runtime extensions absent from patched XDT remain explicitly labeled extensions and otherwise render their embedded published GLB material bindings.",
      "An extension with exact primary <model>_face and <model> textures plus proven main/sub GLB roles receives that canonical default appearance without mutating the reusable model.",
      "Every external GLB image URI is checked before GPU rendering.",
      "Missing and blocked models stay diagnostic; no alternate model is guessed.",
    ],
  };
  writeJson(path.join(options.output, "summary.json"), summary);
  writeJson(path.join(options.output, "manifest.json"), { schema: SCHEMA, entities: records });
  writeJson(path.join(options.output, "texture-resolution.json"), {
    schema: SCHEMA,
    primarySources: textures.proofs,
    entities: records.filter(record => record.source === "xdt").map(record => ({
      rowIndex: record.rowIndex,
      nanoNumber: record.nanoNumber,
      modelStem: record.modelStem,
      main: record.mainTexture,
      sub: record.subTexture,
    })),
  });
  writeHtml(path.join(options.output, "index.html"), summary, records);
  console.log(`Nano gallery generated: ${JSON.stringify(summary.counts)} output=${options.output}`);
}

try { main(); }
catch (error) { console.error(`render_xdt_nano_gallery: ${error.stack ?? error}`); process.exitCode = 1; }
