#!/usr/bin/env node

// Re-publishes the exact table-driven equipment visibility and runtime texture
// contracts without opening a Unity archive. The checked primary Texture2D
// evidence and every native PNG's catalogued byte/hash identity remain the
// authority; unresolved extension content is reported, never guessed.

import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const nativeProjectRootArg = process.argv[2];
if (!nativeProjectRootArg) {
  throw new Error(
    "usage: publish-character-equipment-contracts.mjs <native-project-root>",
  );
}
const nativeProjectRoot = path.resolve(nativeProjectRootArg);
const editorRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)), "../../..",
);
const assetRoot = path.join(nativeProjectRoot, "assets/game");
const avatarPath = path.join(assetRoot, "data/character_creation/avatar_items.json");
const runtimePath = path.join(assetRoot, "data/character_creation/runtime_textures.json");
const tablePath = path.join(assetRoot, "data/tables/table-set.json");
const metadataPath = path.join(
  editorRoot,
  "crates/ffone-asset-pipeline/fixtures/character_creation/retrobution-20260613-chartexture-metadata.json",
);
const equipmentEvidencePath = path.join(
  editorRoot,
  "crates/ffone-asset-pipeline/fixtures/character_creation/retrobution-20260613-player-equipment-texture-metadata.json",
);
const reportPath = path.join(
  editorRoot,
  "work/ffone/reports/player-equipment-texture-audit.json",
);

const [avatar, runtime, tableSet, metadata, equipmentEvidence] = await Promise.all([
  readJson(avatarPath),
  readJson(runtimePath),
  readJson(tablePath),
  readJson(metadataPath),
  readJson(equipmentEvidencePath),
]);

if (avatar.schema !== "ffone.character-creation.avatar-items.v1" || avatar.protocol !== 104) {
  fail("avatar item document is not the native protocol-0104 contract");
}
if (runtime.schema !== "ffone.character-creation.runtime-textures.v2" || runtime.protocol !== 104) {
  fail("runtime texture document is not the native protocol-0104 contract");
}
if (metadata.schema !== "ffone.offline.chartexture-metadata.v1") {
  fail("primary CharTexture metadata has an unexpected schema");
}
if (
  equipmentEvidence.schema !== "ffone.offline.player-equipment-texture-evidence.v1"
  || equipmentEvidence.sourceBuild !== "retrobution-20260613"
  || equipmentEvidence.sourceAlias !== "primary"
) {
  fail("primary player-equipment texture evidence has an unexpected contract");
}

const consolidated = tableSet.tables?.find((entry) => entry.name === "npc_imports_consolidated")?.value;
if (!consolidated) fail("table-set has no npc_imports_consolidated table");

const tableByCategory = new Map([
  ["back", "m_pBackItemTable"],
  ["glasses", "m_pGlassItemTable"],
  ["hat", "m_pHatItemTable"],
  ["head", "m_pHeadItemTable"],
  ["face", "m_pFaceItemTable"],
  ["pants", "m_pPantsItemTable"],
  ["shirt", "m_pShirtsItemTable"],
  ["shoes", "m_pShoesItemTable"],
  ["vehicle", "m_pVehicleItemTable"],
  ["weapon", "m_pWeaponItemTable"],
]);

const itemRows = new Map();
for (const [category, tableName] of tableByCategory) {
  const rows = consolidated[tableName]?.m_pItemData;
  if (!Array.isArray(rows)) fail(`${tableName}.m_pItemData is absent`);
  const byNumber = new Map();
  rows.forEach((row, rowIndex) => {
    const number = category === "face" || category === "head" ? rowIndex : row.m_iItemNumber;
    if (number !== 0) {
      if (byNumber.has(number)) fail(`${tableName} duplicates item ${number}`);
      byNumber.set(number, row);
    }
  });
  itemRows.set(category, byNumber);
}

const equipTypeCounts = new Map();
avatar.items = avatar.items.map((item) => {
  const row = itemRows.get(item.category)?.get(item.itemNumber);
  if (!row) fail(`${item.category} item ${item.itemNumber} has no primary table row`);
  const equipType = row.m_iEquipType;
  if (!Number.isInteger(equipType) || equipType < 0 || equipType > 255) {
    fail(`${item.category} item ${item.itemNumber} has invalid equip type ${equipType}`);
  }
  const key = `${item.category}:${equipType}`;
  equipTypeCounts.set(key, (equipTypeCounts.get(key) ?? 0) + 1);
  return { ...item, equipType };
});

const sourcesByName = groupBy(metadata.textures, (source) => source.trueName.toLowerCase());
const equipmentSourcesByIdentity = groupBy(
  equipmentEvidence.entries,
  (entry) => identityKey(entry.trueName, entry.nativeAsset.blake3, entry.nativeAsset.bytes),
);
const contractsByName = new Map();
for (const contract of runtime.textures) {
  const key = contract.trueName.toLowerCase();
  const previous = contractsByName.get(key);
  if (previous && previous.nativeAsset.path !== contract.nativeAsset.path) {
    fail(`existing runtime texture ${contract.trueName} has conflicting native routes`);
  }
  contractsByName.set(key, contract);
}

const stats = new Map();
const missing = new Set();
const ambiguous = new Set();
const missingSource = new Set();
const verifiedPaths = new Set();
const routeRepairs = new Map();
let textureReferences = 0;
let verifiedNativeFiles = 0;

for (const item of avatar.items) {
  const categoryStats = stats.get(item.category) ?? {
    items: 0,
    textureReferences: 0,
    verifiedUniqueReferences: 0,
    exactPublishedReferences: 0,
    deferredReferences: 0,
  };
  categoryStats.items += 1;
  stats.set(item.category, categoryStats);

  for (const visual of [item.male, item.female]) {
    for (const reference of [visual.primaryTexture, visual.secondaryTexture]) {
      if (!reference) continue;
      textureReferences += 1;
      categoryStats.textureReferences += 1;
      if (reference.status === "missing") {
        missing.add(reference.trueName);
        categoryStats.deferredReferences += 1;
        continue;
      }
      if (reference.status !== "verified_unique" || reference.candidates?.length !== 1) {
        ambiguous.add(reference.trueName);
        categoryStats.deferredReferences += 1;
        continue;
      }

      categoryStats.verifiedUniqueReferences += 1;
      const candidate = reference.candidates[0];
      const sourceKey = reference.trueName.toLowerCase();
      const existing = contractsByName.get(sourceKey);
      if (existing && existing.nativeAsset.path !== candidate.path) {
        if (
          existing.nativeAsset.bytes !== candidate.bytes
          || existing.nativeAsset.blake3.toLowerCase() !== candidate.blake3.toLowerCase()
        ) {
          fail(`${reference.trueName} contradicts existing route ${existing.nativeAsset.path}`);
        }
        candidate.path = existing.nativeAsset.path;
      }
      const png = await readNativeCandidate(candidate);
      verifiedPaths.add(candidate.path);
      if (png.length !== candidate.bytes) {
        fail(`${candidate.path} is ${png.length} bytes, expected ${candidate.bytes}`);
      }
      verifiedNativeFiles += 1;

      const exactEquipmentSources = equipmentSourcesByIdentity.get(
        identityKey(reference.trueName, candidate.blake3, candidate.bytes),
      );
      const charTextureSources = sourcesByName.get(sourceKey);
      let source;
      let sourceAsset;
      let routeRepair;
      let trueNameRepair;
      if (exactEquipmentSources?.length === 1) {
        const evidence = exactEquipmentSources[0];
        source = evidence.texture;
        sourceAsset = `${path.basename(evidence.source.rawResourceFile)}/${evidence.source.sourceAsset}`;
        routeRepair = evidence.routeRepair;
        trueNameRepair = evidence.trueNameRepair;
      } else if (charTextureSources?.length === 1) {
        source = charTextureSources[0];
        sourceAsset = `CharTexture.resourceFile/${metadata.sourceAsset}`;
      } else {
        missingSource.add(reference.trueName);
        categoryStats.deferredReferences += 1;
        continue;
      }
      if (!isPublishableSource(reference.trueName, source, routeRepair, trueNameRepair)) {
        missingSource.add(reference.trueName);
        categoryStats.deferredReferences += 1;
        continue;
      }
      if (existing) {
        // The canonical runtime route may predate this audit, but its byte/hash
        // identity was checked above and remains authoritative for publication.
      } else {
        contractsByName.set(
          sourceKey,
          makeContract(
            reference.trueName,
            candidate,
            source,
            sourceAsset,
            png,
            routeRepair,
            trueNameRepair,
          ),
        );
      }
      categoryStats.exactPublishedReferences += 1;
    }
  }
}

runtime.textures = [...contractsByName.values()].sort((left, right) =>
  left.nativeAsset.path.localeCompare(right.nativeAsset.path) || left.trueName.localeCompare(right.trueName),
);
const publishedPaths = new Set(runtime.textures.map((contract) => contract.nativeAsset.path));
let publishedVerifiedPaths = 0;
for (const route of verifiedPaths) if (publishedPaths.has(route)) publishedVerifiedPaths += 1;

runtime.coverage = {
  ...runtime.coverage,
  avatarTextureReferences: textureReferences,
  avatarVerifiedUniqueRoutes: verifiedPaths.size,
  avatarPublishedRoutes: publishedVerifiedPaths,
  avatarDeferredVerifiedRoutes: verifiedPaths.size - publishedVerifiedPaths,
  avatarMissingTrueNames: [...missing].sort(),
  avatarAmbiguousTrueNames: [...ambiguous].sort(),
  avatarMissingSourceMetadata: [...missingSource].sort(),
  sourceMetadataTextures: metadata.textureCount,
  sourceMetadataUnreadable: metadata.unreadableTextureCount,
};

const sentinels = [
  sentinel("hat", 46, "m_halmet_football", "characters/player/items/hat/helmat_football/textures/m_halmet_football.png"),
  sentinel("back", 31, "back_octibackpack", "characters/player/items/back/back_octibackpack/textures/back_octibackpack.png"),
  sentinel("weapon", 200, "bazooka_toybazooka", "characters/player/items/weapon/bazooka_toybazooka/textures/bazooka_toybazooka.png"),
];
const dynamicVariantBaseTrueNames = [...missing]
  .filter((trueName) => /^[fm]_(face|head)_\d{3}$/i.test(trueName))
  .sort();
const extensionMissingTrueNames = [...missing]
  .filter((trueName) => !dynamicVariantBaseTrueNames.includes(trueName))
  .sort();
const extensionMissingItems = avatar.items.flatMap((item) =>
  [["male", item.male], ["female", item.female]].flatMap(([gender, visual]) =>
    [["primary", visual.primaryTexture], ["secondary", visual.secondaryTexture]]
      .filter(([, reference]) =>
        reference?.status === "missing"
          && extensionMissingTrueNames.includes(reference.trueName))
      .map(([slot, reference]) => ({
        category: item.category,
        itemNumber: item.itemNumber,
        itemName: item.name,
        gender,
        slot,
        textureTrueName: reference.trueName,
        sourceModelTrueName: visual.sourceModelTrueName,
        modelStatus: visual.modelStatus,
      }))),
);

const report = {
  schema: "ffone.player-equipment-texture-audit.v1",
  sourceBuild: "retrobution-20260613",
  sourceAlias: "primary",
  tableSet: path.relative(nativeProjectRoot, tablePath).replaceAll("\\", "/"),
  sourceTextureMetadata: path.relative(editorRoot, metadataPath).replaceAll("\\", "/"),
  equipmentTextureEvidence: path
    .relative(editorRoot, equipmentEvidencePath)
    .replaceAll("\\", "/"),
  avatarItems: avatar.items.length,
  textureReferences,
  verifiedNativeFiles,
  verifiedUniqueRoutes: verifiedPaths.size,
  exactRuntimeContracts: runtime.textures.length,
  exactPublishedRoutes: publishedVerifiedPaths,
  deferredVerifiedRoutes: verifiedPaths.size - publishedVerifiedPaths,
  missingTrueNames: [...missing].sort(),
  dynamicVariantBaseTrueNames,
  extensionMissingTrueNames,
  extensionMissingItems,
  ambiguousTrueNames: [...ambiguous].sort(),
  missingSourceMetadata: [...missingSource].sort(),
  nativeRouteRepairs: [...routeRepairs.values()].sort((left, right) =>
    left.from.localeCompare(right.from)),
  primaryTextureRouteRepairs: equipmentEvidence.entries
    .filter((entry) => entry.routeRepair)
    .map((entry) => ({ trueName: entry.trueName, ...entry.routeRepair }))
    .sort((left, right) => left.trueName.localeCompare(right.trueName)),
  primaryTextureTrueNameRepairs: equipmentEvidence.entries
    .filter((entry) => entry.trueNameRepair)
    .map((entry) => ({ trueName: entry.trueName, ...entry.trueNameRepair }))
    .sort((left, right) => left.trueName.localeCompare(right.trueName)),
  byCategory: Object.fromEntries([...stats].sort(([left], [right]) => left.localeCompare(right))),
  equipTypeCounts: Object.fromEntries([...equipTypeCounts].sort(([left], [right]) => left.localeCompare(right))),
  screenshotSentinels: sentinels,
};

await mkdir(path.dirname(reportPath), { recursive: true });
await Promise.all([
  writeJson(avatarPath, avatar),
  writeJson(runtimePath, runtime),
  writeJson(reportPath, report),
]);

process.stdout.write(
  `published ${runtime.textures.length} exact runtime texture contracts; ` +
    `${publishedVerifiedPaths}/${verifiedPaths.size} verified avatar routes covered; ` +
    `${avatar.items.length} equipType rows preserved\n`,
);

function sentinel(category, itemNumber, trueName, nativePath) {
  const item = avatar.items.find(
    (candidate) => candidate.category === category && candidate.itemNumber === itemNumber,
  );
  if (!item) fail(`sentinel ${category} item ${itemNumber} is absent`);
  const references = [
    item.male?.primaryTexture,
    item.female?.primaryTexture,
    item.male?.secondaryTexture,
    item.female?.secondaryTexture,
  ].filter(Boolean);
  const reference = references.find((candidate) => candidate.trueName === trueName);
  if (!reference || reference.candidates?.[0]?.path !== nativePath) {
    fail(`sentinel ${category} item ${itemNumber} no longer resolves ${trueName} -> ${nativePath}`);
  }
  const contract = contractsByName.get(trueName.toLowerCase());
  if (!contract || contract.nativeAsset.path !== nativePath) {
    fail(`sentinel ${trueName} has no exact published runtime contract`);
  }
  return {
    category,
    itemNumber,
    name: item.name,
    equipType: item.equipType,
    trueName,
    nativePath,
    sourceAsset: contract.source.asset,
    sourcePathId: contract.source.pathId,
    nativeBytes: contract.nativeAsset.bytes,
    nativeBlake3: contract.nativeAsset.blake3,
    nativePngSha256: contract.nativePngSha256,
  };
}

function makeContract(
  trueName,
  candidate,
  source,
  sourceAsset,
  png,
  routeRepair,
  trueNameRepair,
) {
  if (source.sourceMipCount < 1 || (!source.mipMap && source.sourceMipCount !== 1)) {
    fail(`${trueName} has contradictory primary mip evidence`);
  }
  if (![0, 1, 2].includes(source.filterMode) || ![0, 1].includes(source.wrapMode)) {
    fail(`${trueName} has an unsupported primary sampler`);
  }
  const containerRoute = effectiveContainerRoute(
    trueName,
    source,
    routeRepair,
    trueNameRepair,
  );
  if (!containerRoute) fail(`${trueName} has no exact or explicitly repaired primary container route`);
  const hasMips = source.sourceMipCount > 1;
  const minFilter = source.filterMode === 0
    ? (hasMips ? "nearestMipmapNearest" : "nearest")
    : source.filterMode === 1 && hasMips
      ? "linearMipmapNearest"
      : source.filterMode === 2 && hasMips
        ? "linearMipmapLinear"
        : "linear";
  return {
    trueName,
    nativeAsset: { ...candidate },
    nativePngSha256: createHash("sha256").update(png).digest("hex"),
    source: {
      asset: sourceAsset,
      containerRoute,
      pathId: source.pathId,
      width: source.width,
      height: source.height,
      textureFormat: source.textureFormat,
      textureFormatName: textureFormatName(source.textureFormat),
      completeImageSize: source.completeImageSize,
      sourceChainSha256: source.sourceChainSha256,
      mipMap: source.mipMap,
      sourceMipCount: source.sourceMipCount,
      imageCount: source.imageCount,
      textureDimension: source.textureDimension,
    },
    usageColorSpace: "srgb",
    usageColorSpaceSource:
      "ActorSkinCombiner runtime assignment to ShaderLab _MainTex; same sRGB slot interpretation as audited static bindings",
    sampler: {
      name: trueName,
      magFilter: source.filterMode === 0 ? "nearest" : "linear",
      minFilter,
      wrapS: source.wrapMode === 0 ? "repeat" : "clampToEdge",
      wrapT: source.wrapMode === 0 ? "repeat" : "clampToEdge",
      legacyFilterMode: source.filterMode,
      legacyWrapMode: source.wrapMode,
      anisotropyLevel: source.anisotropyLevel,
      mipMapBias: source.mipMapBias,
    },
    publishedMipPolicy: "baseLevelOnly",
  };
}

async function readNativeCandidate(candidate) {
  const dexterMigration = {
    from: "characters/npc/npc_dexter/npc_dexter.textures/npc_dexter.png",
    to: "characters/npcs/npc_dexter/npc_dexter.textures/npc_dexter.png",
  };
  if (candidate.path === dexterMigration.to) {
    routeRepairs.set(dexterMigration.from, {
      ...dexterMigration,
      bytes: candidate.bytes,
      blake3: candidate.blake3,
    });
  }
  try {
    return await readFile(path.join(assetRoot, ...candidate.path.split("/")));
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }
  const alternates = [];
  if (candidate.path.startsWith("characters/npc/")) {
    alternates.push(candidate.path.replace(/^characters\/npc\//, "characters/npcs/"));
  }
  for (const alternate of alternates) {
    try {
      const bytes = await readFile(path.join(assetRoot, ...alternate.split("/")));
      if (bytes.length !== candidate.bytes) continue;
      routeRepairs.set(candidate.path, {
        from: candidate.path,
        to: alternate,
        bytes: candidate.bytes,
        blake3: candidate.blake3,
      });
      candidate.path = alternate;
      return bytes;
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
    }
  }
  fail(`verified native texture route is absent: ${candidate.path}`);
}

function isPublishableSource(trueName, source, routeRepair, trueNameRepair) {
  if (
    source.sourceMipCount < 1 ||
    (!source.mipMap && source.sourceMipCount !== 1) ||
    ![0, 1, 2].includes(source.filterMode) ||
    ![0, 1].includes(source.wrapMode)
  ) {
    return false;
  }
  const formats = new Set([1, 2, 3, 4, 5, 7, 10, 11, 12, 13, 14]);
  return formats.has(source.textureFormat)
    && effectiveContainerRoute(trueName, source, routeRepair, trueNameRepair) !== undefined;
}

function effectiveContainerRoute(trueName, source, routeRepair, trueNameRepair) {
  const expectedRoute = `texture/${trueName}.dds`;
  const exact = source.containerRoutes.find(
    (route) => route.toLowerCase() === expectedRoute.toLowerCase(),
  );
  const exactTrueName = source.trueName.toLowerCase() === trueName.toLowerCase();
  if (exact) {
    validateTrueNameRepair(trueName, source, exact, trueNameRepair);
    return exact;
  }
  if (!exactTrueName) {
    fail(`${trueName} differs from ${source.trueName} without its exact primary container route`);
  }
  if (trueNameRepair) fail(`${trueName} has unnecessary true-name repair evidence`);
  if (!routeRepair) return undefined;
  if (
    routeRepair.requestedContainerRoute.toLowerCase() !== expectedRoute.toLowerCase()
    || JSON.stringify(routeRepair.serializedContainerRoutes) !== JSON.stringify(source.containerRoutes)
    || typeof routeRepair.kind !== "string"
    || routeRepair.kind.length === 0
    || typeof routeRepair.reason !== "string"
    || routeRepair.reason.length === 0
  ) {
    fail(`${trueName} has invalid primary route-repair evidence`);
  }
  return routeRepair.requestedContainerRoute;
}

function validateTrueNameRepair(trueName, source, exactRoute, trueNameRepair) {
  const exactTrueName = source.trueName.toLowerCase() === trueName.toLowerCase();
  if (exactTrueName) {
    if (trueNameRepair) fail(`${trueName} has unnecessary true-name repair evidence`);
    return;
  }
  if (
    !trueNameRepair
    || trueNameRepair.requestedTrueName.toLowerCase() !== trueName.toLowerCase()
    || trueNameRepair.serializedTrueName.toLowerCase() !== source.trueName.toLowerCase()
    || trueNameRepair.requestedContainerRoute.toLowerCase() !== exactRoute.toLowerCase()
    || typeof trueNameRepair.reason !== "string"
    || trueNameRepair.reason.length === 0
  ) {
    fail(`${trueName} has invalid primary true-name repair evidence`);
  }
}

function textureFormatName(value) {
  const names = new Map([
    [1, "Alpha8"], [2, "ARGB4444"], [3, "RGB24"], [4, "RGBA32"], [5, "ARGB32"],
    [7, "RGB565"], [10, "DXT1"], [11, "DXT3"], [12, "DXT5"], [13, "RGBA4444"],
    [14, "BGRA32"],
  ]);
  const name = names.get(value);
  if (!name) fail(`unsupported primary Texture2D format ${value}`);
  return name;
}

function groupBy(values, keyOf) {
  const groups = new Map();
  for (const value of values) {
    const key = keyOf(value);
    const group = groups.get(key) ?? [];
    group.push(value);
    groups.set(key, group);
  }
  return groups;
}

function identityKey(trueName, blake3, bytes) {
  return `${trueName.toLowerCase()}\0${blake3.toLowerCase()}\0${bytes}`;
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

async function writeJson(file, value) {
  await writeFile(file, `${JSON.stringify(sortJson(value), null, 2)}\n`);
}

function fail(message) {
  throw new Error(message);
}
