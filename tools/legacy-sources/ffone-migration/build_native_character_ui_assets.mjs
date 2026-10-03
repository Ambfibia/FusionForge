#!/usr/bin/env node

import {
  copyFileSync,
  createReadStream,
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { basename, dirname, isAbsolute, join, relative, resolve } from "node:path";
import process from "node:process";

const CATALOG_SCHEMA = "ffone.native-character-ui-catalog.v1";
const PLAN_SCHEMA = "ffone.native-character-ui-publication-plan.v1";
const SEMANTIC_ROUTE_PLAN_SCHEMA = "ffone.semantic-tree-route-plan.v1";
const EXPECTED_SHARED_TEXTURES = 65;
const EXPECTED_CHARACTER_TEXTURES = 52;
const EXPECTED_TOTAL_TEXTURES = 117;
const HASHED_SOURCE_NAME = /^(.+)--([0-9a-f]{16})\.png$/i;
const CHARACTER_BUNDLE_TEXTURE =
  /^(?:CS(?:Future|Suburbs|Downtown|Wilds|Darklands)BG_(?:[1-9]|10)|CSBG|CharCreationBG)$/;

function usage() {
  return [
    "Usage:",
    "  node tools/legacy-sources/ffone-migration/build_native_character_ui_assets.mjs \\",
    "    --asset-root assets/game \\",
    "    --shared-objects work/main-sharedassets0-all.json \\",
    "    --character-objects work/character-creation-all.json \\",
    "    --output work/native-character-ui-v1",
  ].join("\n");
}

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || !value) {
      throw new Error(`invalid arguments\n\n${usage()}`);
    }
    values.set(key.slice(2), value);
  }
  const required = [
    "asset-root",
    "shared-objects",
    "character-objects",
    "output",
  ];
  for (const key of required) {
    if (!values.has(key)) {
      throw new Error(`missing --${key}\n\n${usage()}`);
    }
  }
  return Object.fromEntries(values);
}

function normalizeProjectPath(path) {
  return path.replaceAll("\\", "/");
}

function displayPath(path) {
  const fromCwd = relative(process.cwd(), path);
  return normalizeProjectPath(fromCwd && !fromCwd.startsWith("..") ? fromCwd : path);
}

function readJson(path) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    throw new Error(`failed to read JSON ${displayPath(path)}: ${error.message}`);
  }
}

function sha256File(path) {
  return new Promise((resolveHash, reject) => {
    const hash = createHash("sha256");
    const input = createReadStream(path);
    input.on("error", reject);
    input.on("data", (chunk) => hash.update(chunk));
    input.on("end", () => resolveHash(hash.digest("hex")));
  });
}

function pngDimensions(path) {
  const bytes = readFileSync(path);
  const signature = bytes.subarray(0, 8).toString("hex");
  if (signature !== "89504e470d0a1a0a" || bytes.subarray(12, 16).toString() !== "IHDR") {
    throw new Error(`${displayPath(path)} is not a canonical PNG with an IHDR header`);
  }
  return {
    width: bytes.readUInt32BE(16),
    height: bytes.readUInt32BE(20),
  };
}

function normalizedSourceStem(trueName) {
  return trueName
    .normalize("NFKD")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
}

function sourceTextureObjects(objects, predicate, sourceDump) {
  const result = objects
    .filter((object) => object?.type === "Texture2D" && predicate(object.name))
    .map((object) => {
      const width = object.value?.m_Width;
      const height = object.value?.m_Height;
      if (
        !Number.isSafeInteger(object.pathId) ||
        typeof object.asset !== "string" ||
        typeof object.name !== "string" ||
        !Number.isSafeInteger(width) ||
        !Number.isSafeInteger(height)
      ) {
        throw new Error(
          `Texture2D identity is incomplete in ${displayPath(sourceDump)}: ${JSON.stringify({
            asset: object.asset,
            pathId: object.pathId,
            trueName: object.name,
            width,
            height,
          })}`,
        );
      }
      return {
        sourceAsset: object.asset,
        sourcePathId: object.pathId,
        trueName: object.name,
        width,
        height,
        sourceDump: displayPath(sourceDump),
      };
    });
  result.sort((left, right) => left.trueName.localeCompare(right.trueName, "en"));
  return result;
}

function destinationFor(trueName) {
  const background = trueName.match(
    /^CS(Future|Suburbs|Downtown|Wilds|Darklands)BG_([1-9]|10)$/,
  );
  if (background) {
    return `ui/character/selection/backgrounds/${background[1].toLowerCase()}/${trueName}.png`;
  }
  if (trueName === "CSBG") {
    return `ui/character/selection/background/${trueName}.png`;
  }
  if (trueName.startsWith("CS")) {
    return `ui/character/selection/controls/${trueName}.png`;
  }
  if (trueName === "CharCreationBG") {
    return `ui/character/creation/background/${trueName}.png`;
  }
  if (/^CC(?:Name|Scroll)/.test(trueName)) {
    return `ui/character/creation/name/${trueName}.png`;
  }
  if (/^CCBodyshape/.test(trueName)) {
    return `ui/character/creation/body/${trueName}.png`;
  }
  if (/^CCCheckbox/.test(trueName)) {
    return `ui/character/creation/body/checkbox/${trueName}.png`;
  }
  if (/^CCClothes/.test(trueName)) {
    return `ui/character/creation/clothes/${trueName}.png`;
  }
  if (/^CC(?:Color|SelectedColor)/.test(trueName)) {
    return `ui/character/creation/colors/${trueName}.png`;
  }
  if (/^CC(?:Rotate|Roatate|Zoom)/.test(trueName)) {
    return `ui/character/creation/camera/${trueName}.png`;
  }
  if (/^CC(?:BG|Box|CharacterDisplayArea|ClassBG|LeftIn|RightBG)/.test(trueName)) {
    return `ui/character/creation/layout/${trueName}.png`;
  }
  throw new Error(`no evidence-backed character UI route for Texture2D.m_Name=${trueName}`);
}

function assertReadableDestination(path) {
  const file = basename(path);
  if (HASHED_SOURCE_NAME.test(file) || /(?:^|[_-])[0-9a-f]{16,}(?:\.|$)/i.test(file)) {
    throw new Error(`semantic destination retained a hash-like filename: ${path}`);
  }
  if (!/^[A-Za-z0-9_.-]+\.png$/.test(file)) {
    throw new Error(`semantic destination is not a readable PNG name: ${path}`);
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const assetRoot = resolve(args["asset-root"]);
  const sharedObjectsPath = resolve(args["shared-objects"]);
  const characterObjectsPath = resolve(args["character-objects"]);
  const outputRoot = resolve(args.output);
  const outputParent = dirname(outputRoot);
  const stageRoot = join(outputParent, `.native-character-ui-stage-${process.pid}`);

  for (const path of [assetRoot, sharedObjectsPath, characterObjectsPath]) {
    if (!existsSync(path)) {
      throw new Error(`required input does not exist: ${displayPath(path)}`);
    }
  }
  if (existsSync(outputRoot)) {
    throw new Error(`output already exists and will not be overwritten: ${displayPath(outputRoot)}`);
  }
  if (existsSync(stageRoot)) {
    throw new Error(`stale stage exists: ${displayPath(stageRoot)}`);
  }

  const manifestPath = join(assetRoot, "asset-manifest.json");
  const manifest = readJson(manifestPath);
  if (manifest.schema !== "ffone.project-assets.v1" || !Array.isArray(manifest.files)) {
    throw new Error(`${displayPath(manifestPath)} is not ffone.project-assets.v1`);
  }

  const sharedObjects = readJson(sharedObjectsPath);
  const characterObjects = readJson(characterObjectsPath);
  if (!Array.isArray(sharedObjects) || !Array.isArray(characterObjects)) {
    throw new Error("object dumps must be top-level JSON arrays");
  }

  const sharedTextures = sourceTextureObjects(
    sharedObjects,
    (name) => /^(?:CC|CS)/.test(name),
    sharedObjectsPath,
  );
  const characterTextures = sourceTextureObjects(
    characterObjects,
    (name) => CHARACTER_BUNDLE_TEXTURE.test(name),
    characterObjectsPath,
  );
  if (sharedTextures.length !== EXPECTED_SHARED_TEXTURES) {
    throw new Error(
      `sharedassets0 closure drifted: expected ${EXPECTED_SHARED_TEXTURES} CC*/CS* Texture2D objects, found ${sharedTextures.length}`,
    );
  }
  if (characterTextures.length !== EXPECTED_CHARACTER_TEXTURES) {
    throw new Error(
      `CharacterCreation closure drifted: expected ${EXPECTED_CHARACTER_TEXTURES} UI Texture2D objects, found ${characterTextures.length}`,
    );
  }

  const textures = [...sharedTextures, ...characterTextures];
  const foldedNames = new Set();
  for (const texture of textures) {
    const folded = texture.trueName.toLowerCase();
    if (foldedNames.has(folded)) {
      throw new Error(`duplicate case-insensitive Texture2D.m_Name: ${texture.trueName}`);
    }
    foldedNames.add(folded);
  }
  if (textures.length !== EXPECTED_TOTAL_TEXTURES) {
    throw new Error(
      `character UI closure drifted: expected ${EXPECTED_TOTAL_TEXTURES}, found ${textures.length}`,
    );
  }

  const manifestTextureByStem = new Map();
  for (const file of manifest.files) {
    if (file.kind !== "texture" || typeof file.path !== "string") {
      continue;
    }
    const match = basename(file.path).match(HASHED_SOURCE_NAME);
    if (!match || !file.path.startsWith("textures/")) {
      continue;
    }
    const list = manifestTextureByStem.get(match[1]) ?? [];
    list.push(file);
    manifestTextureByStem.set(match[1], list);
  }

  const destinations = new Set();
  const catalogAssets = [];
  const publicationEntries = [];
  const semanticRoutes = [];
  const counts = {
    total: 0,
    selection: 0,
    creation: 0,
    sharedassets0: sharedTextures.length,
    characterCreationBundle: characterTextures.length,
  };
  let installedBytes = 0;

  mkdirSync(stageRoot, { recursive: false });
  try {
    for (const texture of textures) {
      const stem = normalizedSourceStem(texture.trueName);
      const matches = manifestTextureByStem.get(stem) ?? [];
      if (matches.length !== 1) {
        throw new Error(
          `Texture2D.m_Name=${texture.trueName} resolved to ${matches.length} hashed source PNGs`,
        );
      }
      const sourceEntry = matches[0];
      const sourcePath = join(
        assetRoot,
        ...sourceEntry.path.split("/").filter((component) => component.length > 0),
      );
      const sourceStats = statSync(sourcePath);
      if (!sourceStats.isFile() || sourceStats.size !== sourceEntry.bytes) {
        throw new Error(
          `source byte identity mismatch for ${sourceEntry.path}: manifest=${sourceEntry.bytes}, disk=${sourceStats.size}`,
        );
      }
      const dimensions = pngDimensions(sourcePath);
      if (dimensions.width !== texture.width || dimensions.height !== texture.height) {
        throw new Error(
          `Texture2D dimension mismatch for ${texture.trueName}: object=${texture.width}x${texture.height}, PNG=${dimensions.width}x${dimensions.height}`,
        );
      }

      const destination = destinationFor(texture.trueName);
      assertReadableDestination(destination);
      const foldedDestination = destination.toLowerCase();
      if (destinations.has(foldedDestination)) {
        throw new Error(`duplicate case-insensitive destination: ${destination}`);
      }
      destinations.add(foldedDestination);

      const targetPath = join(stageRoot, "assets", "game", ...destination.split("/"));
      mkdirSync(dirname(targetPath), { recursive: true });
      copyFileSync(sourcePath, targetPath);
      const sha256 = await sha256File(targetPath);
      installedBytes += sourceStats.size;
      counts.total += 1;
      if (destination.startsWith("ui/character/selection/")) {
        counts.selection += 1;
      } else {
        counts.creation += 1;
      }

      const asset = {
        trueName: texture.trueName,
        path: destination,
        bytes: sourceStats.size,
        blake3: sourceEntry.blake3,
        sha256,
        dimensions,
        source: {
          asset: texture.sourceAsset,
          pathId: texture.sourcePathId,
          type: "Texture2D",
          objectDump: texture.sourceDump,
          nativePath: sourceEntry.path,
          nativeSourcePath: sourceEntry.source_path,
        },
      };
      catalogAssets.push(asset);
      publicationEntries.push({
        source: normalizeProjectPath(relative(outputRoot, targetPath)),
        destination: `assets/game/${destination}`,
        bytes: sourceStats.size,
        blake3: sourceEntry.blake3,
        sha256,
      });
      semanticRoutes.push({
        sourcePath: sourceEntry.path,
        destinationPath: destination,
        kind: "texture",
        category: "ui",
        ownership: {
          trueLegacyName: texture.trueName,
          authority:
            "exact Texture2D.m_Name and PathID from FusionForge offline object dump",
          sourceBuild: "retrobution-20260613",
          sourceArchive: texture.sourceAsset,
          sourceAsset: `${texture.sourceAsset}#${texture.sourcePathId}`,
          sourceObjectIds: {
            texture2DPathId: texture.sourcePathId,
          },
        },
        content: {
          mode: "copy_exact",
        },
      });
    }

    catalogAssets.sort((left, right) => left.path.localeCompare(right.path, "en"));
    publicationEntries.sort((left, right) =>
      left.destination.localeCompare(right.destination, "en"),
    );

    const evidence = {
      sharedObjects: {
        path: displayPath(sharedObjectsPath),
        sha256: await sha256File(sharedObjectsPath),
        textureCount: sharedTextures.length,
        sourceAssets: [...new Set(sharedTextures.map((texture) => texture.sourceAsset))].sort(),
      },
      characterObjects: {
        path: displayPath(characterObjectsPath),
        sha256: await sha256File(characterObjectsPath),
        textureCount: characterTextures.length,
        sourceAssets: [...new Set(characterTextures.map((texture) => texture.sourceAsset))].sort(),
      },
      projectAssetManifest: {
        path: displayPath(manifestPath),
        sha256: await sha256File(manifestPath),
        sourcePack: manifest.source_pack,
      },
    };

    const catalogPath = join(
      stageRoot,
      "assets",
      "game",
      "ui",
      "character",
      "catalog.json",
    );
    mkdirSync(dirname(catalogPath), { recursive: true });
    const catalog = {
      schema: CATALOG_SCHEMA,
      sourceBuild: "retrobution-20260613",
      status: "complete-source-proven-texture-closure",
      sourcePolicy:
        "exact extracted PNG bytes routed by Texture2D.m_Name; FusionForge object dumps are offline provenance only; no Unity runtime dependency",
      counts,
      installedBytes,
      evidence,
      assets: catalogAssets,
    };
    writeFileSync(catalogPath, `${JSON.stringify(catalog, null, 2)}\n`, {
      encoding: "utf8",
      flag: "wx",
    });

    const catalogBytes = statSync(catalogPath).size;
    const catalogSha256 = await sha256File(catalogPath);
    const plan = {
      schema: PLAN_SCHEMA,
      sourceBuild: "retrobution-20260613",
      status: "ready-for-isolated-audit",
      policy:
        "publish only readable Texture2D.m_Name paths below assets/game/ui/character; hashed source files remain provenance until the production manifest swap",
      textureCount: textures.length,
      textureBytes: installedBytes,
      catalog: {
        source: normalizeProjectPath(relative(outputRoot, catalogPath)),
        destination: "assets/game/ui/character/catalog.json",
        bytes: catalogBytes,
        sha256: catalogSha256,
      },
      entries: publicationEntries,
    };
    writeFileSync(join(stageRoot, "publication-plan.json"), `${JSON.stringify(plan, null, 2)}\n`, {
      encoding: "utf8",
      flag: "wx",
    });

    semanticRoutes.push({
      sourcePath: "native-character-ui/catalog",
      destinationPath: "ui/character/catalog.json",
      kind: "data",
      category: "ui",
      ownership: {
        trueLegacyName: "FusionFall character selection and creation UI catalog",
        authority:
          "generated only from the exact 117-entry Texture2D.m_Name closure recorded in this plan",
        sourceBuild: "retrobution-20260613",
        sourceArchive: "CharacterCreation + main/sharedassets0",
        sourceAsset: "native-character-ui-catalog",
        sourceObjectIds: {},
      },
      content: {
        mode: "generated_json",
        value: catalog,
      },
    });
    semanticRoutes.sort((left, right) =>
      left.destinationPath.localeCompare(right.destinationPath, "en"),
    );
    const semanticRoutePlan = {
      schema: SEMANTIC_ROUTE_PLAN_SCHEMA,
      sourceBuild: "retrobution-20260613",
      policy:
        "publish exact PNG bytes under true Texture2D.m_Name paths; generated catalog contains only independently hashed source provenance; retain flat hashed sources until all runtime consumers migrate",
      routes: semanticRoutes,
    };
    writeFileSync(
      join(stageRoot, "semantic-route-plan.json"),
      `${JSON.stringify(semanticRoutePlan, null, 2)}\n`,
      {
        encoding: "utf8",
        flag: "wx",
      },
    );

    renameSync(stageRoot, outputRoot);
    console.log(
      JSON.stringify(
        {
          status: "complete",
          output: displayPath(outputRoot),
          textures: textures.length,
          selectionTextures: counts.selection,
          creationTextures: counts.creation,
          textureBytes: installedBytes,
          files: textures.length + 3,
          hashedDestinationNames: 0,
          gltfFiles: 0,
        },
        null,
        2,
      ),
    );
  } catch (error) {
    rmSync(stageRoot, { force: true, recursive: true });
    throw error;
  }
}

main().catch((error) => {
  console.error(error.stack ?? error.message);
  process.exitCode = 1;
});
