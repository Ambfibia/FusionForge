#!/usr/bin/env node

// Consolidates already-extracted primary Texture2D metadata into the checked
// evidence closure needed by table-driven player equipment. The patched cache
// is navigation input only; every selected entry retains the raw primary
// resource-file hash and exact native PNG acceptance identity.

import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const nativeProjectRootArg = process.argv[2];
const metadataSourceRootArg = process.argv[3];
if (!nativeProjectRootArg || !metadataSourceRootArg) {
  throw new Error(
    "usage: build-character-equipment-texture-evidence.mjs <native-project-root> <metadata-source-root>",
  );
}
const nativeProjectRoot = path.resolve(nativeProjectRootArg);
const editorRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)), "../../..",
);

const avatarPath = path.join(
  nativeProjectRoot,
  "assets/game/data/character_creation/avatar_items.json",
);
const sourceRoot = path.resolve(metadataSourceRootArg);
const destination = path.join(
  editorRoot,
  "crates/ffone-asset-pipeline/fixtures/character_creation/retrobution-20260613-player-equipment-texture-metadata.json",
);
const sourceFiles = [
  "texture-metadata.primary.CharTexture.mapped.json",
  "texture-metadata.primary.NpcTexture.mapped.json",
  "texture-metadata.primary.Retro_shared.mapped.json",
  "texture-metadata.primary.Retro_shared_part2.mapped.json",
  "texture-metadata.primary.CharacterCreation.mapped.json",
  "texture-metadata.primary.DongResources_12_10.mapped.json",
  "texture-metadata.primary.Tutorial.mapped.json",
];
const routeRepairRules = new Map(Object.entries({
  back_buzzshockbmo: {
    kind: "primary_container_alias",
    serializedContainerRoutes: ["texture/back_buzzshock.dds"],
    reason: "The primary Texture2D trueName is exact, but its AssetBundle route keeps the shorter back_buzzshock alias.",
  },
  back_leggarnets1: {
    kind: "primary_container_alias",
    serializedContainerRoutes: ["texture/back_leggarnet.dds"],
    reason: "The primary Texture2D trueName is exact, but its AssetBundle route keeps the singular back_leggarnet alias.",
  },
  hatglassmelee_pochitabullfragbananaguard: {
    kind: "primary_container_alias",
    serializedContainerRoutes: ["texture/glass_bullfrag.dds"],
    reason: "The primary Texture2D trueName is exact, but its AssetBundle route keeps the legacy glass_bullfrag alias.",
  },
  head_puckerberryhead: {
    kind: "primary_unrouted_object",
    serializedContainerRoutes: [],
    reason: "The exact primary Texture2D is serialized but has no AssetBundle container route; FFOne publishes its verified native PNG directly.",
  },
  mob_bat: {
    kind: "primary_external_container_route",
    sourceFile: "texture-metadata.primary.Tutorial.mapped.json",
    sourcePathId: 277,
    serializedContainerRoutes: [],
    reason: "NpcTexture owns texture/mob_bat.dds through fileId 1 pathId 277 in the primary Tutorial asset.",
    routeOwner: {
      sourceFile: "texture-metadata.primary.NpcTexture.mapped.json",
      assetBundlePathId: 1,
      externalFileId: 1,
      externalPathId: 277,
    },
  },
  shirt_gunter: {
    kind: "primary_cross_routed_object",
    serializedContainerRoutes: ["texture/shirt_rainicorn.dds"],
    reason: "The primary bundle cross-routes the Gunter and Rainicorn Texture2D objects; FFOne preserves the exact trueName PNG instead of the swapped route.",
  },
  shirt_rainicorn: {
    kind: "primary_cross_routed_object",
    serializedContainerRoutes: ["texture/shirt_gunter.dds"],
    reason: "The primary bundle cross-routes the Gunter and Rainicorn Texture2D objects; FFOne preserves the exact trueName PNG instead of the swapped route.",
  },
}));

const avatar = await readJson(avatarPath);
const documents = await Promise.all(
  sourceFiles.map(async (file, priority) => ({
    file,
    priority,
    document: await readJson(path.join(sourceRoot, file)),
  })),
);
const documentsByFile = new Map(documents.map((entry) => [entry.file, entry]));

const sourcesByNativeIdentity = new Map();
const sourcesByContainerRoute = new Map();
for (const { file, priority, document } of documents) {
  if (document.schema !== "ffone.offline.chartexture-metadata.v1") {
    fail(`${file} has an unexpected schema`);
  }
  const resourceFile = path.basename(document.rawBundle.sourcePath.replaceAll("\\", "/"));
  const normalizedCache = document.sourcePath.replaceAll("\\", "/");
  const cacheMarker = "retrobution-20260613.ffclient/";
  const cacheRelativePath = normalizedCache.includes(cacheMarker)
    ? normalizedCache.slice(normalizedCache.indexOf(cacheMarker) + cacheMarker.length)
    : path.basename(normalizedCache);
  for (const texture of document.textures) {
    if (!texture.nativePngBlake3 || !Number.isInteger(texture.nativePngBytes)) continue;
    const entry = {
      priority,
      sourceFile: file,
      source: {
        sourceAlias: "primary",
        rawResourceFile: `builds/retrobution-20260613/${resourceFile}`,
        rawResourceFileBytes: document.rawBundle.byteLength,
        rawResourceFileSha256: document.rawBundle.sha256,
        navigationCacheAlias: "patched",
        navigationCacheRelativePath: cacheRelativePath,
        sourceAsset: document.sourceAsset,
        sourceFileBytes: document.sourceFileBytes,
        sourceFileSha256: document.sourceFileSha256,
      },
      texture,
    };
    const key = identityKey(texture.trueName, texture.nativePngBlake3, texture.nativePngBytes);
    const entries = sourcesByNativeIdentity.get(key) ?? [];
    entries.push(entry);
    sourcesByNativeIdentity.set(key, entries);
    for (const route of texture.containerRoutes) {
      const routeEntries = sourcesByContainerRoute.get(route.toLowerCase()) ?? [];
      routeEntries.push(entry);
      sourcesByContainerRoute.set(route.toLowerCase(), routeEntries);
    }
  }
}

const required = new Map();
for (const item of avatar.items) {
  for (const visual of [item.male, item.female]) {
    for (const reference of [visual.primaryTexture, visual.secondaryTexture]) {
      if (reference?.status !== "verified_unique" || reference.candidates?.length !== 1) continue;
      const candidate = reference.candidates[0];
      required.set(identityKey(reference.trueName, candidate.blake3, candidate.bytes), {
        trueName: reference.trueName,
        nativeAsset: { ...candidate },
      });
    }
  }
}

const entries = [];
const unresolved = [];
for (const [key, reference] of required) {
  const repairRule = routeRepairRules.get(reference.trueName.toLowerCase());
  const requestedRoute = `texture/${reference.trueName}.dds`.toLowerCase();
  const routeCandidates = (sourcesByContainerRoute.get(requestedRoute) ?? [])
    .filter(({ texture }) =>
      texture.nativePngBlake3.toLowerCase() === reference.nativeAsset.blake3.toLowerCase()
        && texture.nativePngBytes === reference.nativeAsset.bytes);
  let candidates = uniqueSources([
    ...(sourcesByNativeIdentity.get(key) ?? []),
    ...routeCandidates,
  ])
    .filter(({ texture }) => hasPublishableEncoding(texture))
    .sort((left, right) => left.priority - right.priority
      || left.texture.pathId - right.texture.pathId);
  if (repairRule?.sourceFile) {
    candidates = candidates.filter(
      ({ texture, sourceFile }) => sourceFile === repairRule.sourceFile
        && texture.pathId === repairRule.sourcePathId,
    );
  }
  candidates = candidates.filter(({ texture }) =>
    hasRequestedContainerRoute(texture, reference.trueName) || repairRule !== undefined);
  if (candidates.length === 0) {
    unresolved.push(reference);
    continue;
  }
  const selected = candidates[0];
  const routeRepair = buildRouteRepair(reference, selected, repairRule);
  const trueNameRepair = buildTrueNameRepair(reference, selected);
  entries.push({
    trueName: reference.trueName,
    nativeAsset: reference.nativeAsset,
    source: selected.source,
    texture: selected.texture,
    ...(routeRepair ? { routeRepair } : {}),
    ...(trueNameRepair ? { trueNameRepair } : {}),
    equivalentSourceCandidates: candidates.map((candidate) => ({
      rawResourceFile: candidate.source.rawResourceFile,
      sourceAsset: candidate.source.sourceAsset,
      pathId: candidate.texture.pathId,
      sourceChainSha256: candidate.texture.sourceChainSha256,
    })),
  });
}

entries.sort((left, right) =>
  left.nativeAsset.path.localeCompare(right.nativeAsset.path)
    || left.trueName.localeCompare(right.trueName),
);
unresolved.sort((left, right) => left.nativeAsset.path.localeCompare(right.nativeAsset.path));

const document = {
  schema: "ffone.offline.player-equipment-texture-evidence.v1",
  sourceBuild: "retrobution-20260613",
  sourceAlias: "primary",
  navigationCacheAlias: "patched",
  conversionCommand:
    "node crates/ffone-asset-pipeline/scripts/build-character-equipment-texture-evidence.mjs <native-project-root> <metadata-source-root>",
  conversionVersion: 2,
  requiredNativeRoutes: required.size,
  exactEntries: entries.length,
  unresolvedNativeRoutes: unresolved.length,
  sourceDocuments: sourceFiles,
  entries,
  unresolved,
};
await writeFile(destination, `${JSON.stringify(sortJson(document), null, 2)}\n`);
process.stdout.write(
  `published ${entries.length}/${required.size} exact primary equipment texture entries; `
    + `${unresolved.length} routes remain explicit extensions/unresolved\n`,
);

function identityKey(trueName, blake3, bytes) {
  return `${trueName.toLowerCase()}\0${blake3.toLowerCase()}\0${bytes}`;
}

function hasPublishableEncoding(texture) {
  return texture.sourceMipCount >= 1
    && (texture.mipMap || texture.sourceMipCount === 1)
    && [0, 1, 2].includes(texture.filterMode)
    && [0, 1].includes(texture.wrapMode)
    && [1, 2, 3, 4, 5, 7, 10, 11, 12, 13, 14].includes(texture.textureFormat);
}

function hasRequestedContainerRoute(texture, trueName) {
  const expectedRoute = `texture/${trueName}.dds`.toLowerCase();
  return texture.containerRoutes.some((route) => route.toLowerCase() === expectedRoute);
}

function buildRouteRepair(reference, selected, rule) {
  if (hasRequestedContainerRoute(selected.texture, reference.trueName)) return undefined;
  if (!rule) fail(`${reference.trueName} needs an unregistered primary route repair`);
  const actualRoutes = [...selected.texture.containerRoutes];
  if (JSON.stringify(actualRoutes) !== JSON.stringify(rule.serializedContainerRoutes)) {
    fail(`${reference.trueName} primary container routes changed`);
  }
  let routeOwner;
  if (rule.routeOwner) {
    const ownerEntry = documentsByFile.get(rule.routeOwner.sourceFile);
    if (!ownerEntry) fail(`${reference.trueName} route-owner document is absent`);
    routeOwner = {
      ...sourceOwner(ownerEntry.document),
      assetBundlePathId: rule.routeOwner.assetBundlePathId,
      externalFileId: rule.routeOwner.externalFileId,
      externalPathId: rule.routeOwner.externalPathId,
      targetSourceAsset: selected.source.sourceAsset,
      targetPathId: selected.texture.pathId,
    };
  }
  return {
    kind: rule.kind,
    requestedContainerRoute: `texture/${reference.trueName}.dds`,
    serializedContainerRoutes: actualRoutes,
    reason: rule.reason,
    ...(routeOwner ? { routeOwner } : {}),
  };
}

function buildTrueNameRepair(reference, selected) {
  if (selected.texture.trueName.toLowerCase() === reference.trueName.toLowerCase()) {
    return undefined;
  }
  const requestedContainerRoute = `texture/${reference.trueName}.dds`;
  if (!hasRequestedContainerRoute(selected.texture, reference.trueName)) {
    fail(`${reference.trueName} true-name repair lacks its exact primary container route`);
  }
  return {
    requestedTrueName: reference.trueName,
    serializedTrueName: selected.texture.trueName,
    requestedContainerRoute,
    reason:
      "The logical XDT texture name resolves through this exact primary AssetBundle container route, while the serialized Texture2D m_Name differs.",
  };
}

function uniqueSources(candidates) {
  const unique = new Map();
  for (const candidate of candidates) {
    unique.set(`${candidate.sourceFile}\0${candidate.texture.pathId}`, candidate);
  }
  return [...unique.values()];
}

function sourceOwner(document) {
  const resourceFile = path.basename(document.rawBundle.sourcePath.replaceAll("\\", "/"));
  return {
    sourceAlias: "primary",
    rawResourceFile: `builds/retrobution-20260613/${resourceFile}`,
    rawResourceFileBytes: document.rawBundle.byteLength,
    rawResourceFileSha256: document.rawBundle.sha256,
    sourceAsset: document.sourceAsset,
    sourceFileBytes: document.sourceFileBytes,
    sourceFileSha256: document.sourceFileSha256,
  };
}

function sortJson(value) {
  if (Array.isArray(value)) return value.map(sortJson);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, child]) => [key, sortJson(child)]),
    );
  }
  return value;
}

async function readJson(file) {
  return JSON.parse(await readFile(file, "utf8"));
}

function fail(message) {
  throw new Error(message);
}
