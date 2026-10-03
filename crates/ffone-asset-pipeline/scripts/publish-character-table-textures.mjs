#!/usr/bin/env node

// Repairs table-driven texture lookups whose logical XDT name was not owned by
// a player resource set. Exact primary AssetBundle container routes select the
// source Texture2D; raw PNG recovery is an explicit Editor-owned input and only
// the hash-verified native publication is copied into assets/game.

import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const [workspaceArg, sourceRootArg, recoveryRootArg] = process.argv.slice(2);
if (!workspaceArg || !sourceRootArg || !recoveryRootArg) {
  throw new Error(
    "usage: publish-character-table-textures.mjs <workspace> <metadata-source-root> <png-recovery-root>",
  );
}
const workspace = path.resolve(workspaceArg);
const avatarPath = path.join(
  workspace,
  "assets/game/data/character_creation/avatar_items.json",
);
const runtimePath = path.join(
  workspace,
  "assets/game/data/character_creation/runtime_textures.json",
);
const sourceRoot = path.resolve(sourceRootArg);
const recoveryRoot = path.resolve(recoveryRootArg);
const assetRoot = path.join(workspace, "assets/game");
const publicationRoot = "characters/player/table-textures";
const sourceFiles = [
  ["CharTexture", "texture-metadata.primary.CharTexture.mapped.json"],
  ["NpcTexture", "texture-metadata.primary.NpcTexture.mapped.json"],
  ["Retro_shared", "texture-metadata.primary.Retro_shared.mapped.json"],
  ["Retro_shared_part2", "texture-metadata.primary.Retro_shared_part2.mapped.json"],
  ["CharacterCreation", "texture-metadata.primary.CharacterCreation.mapped.json"],
  ["DongResources_12_10", "texture-metadata.primary.DongResources_12_10.mapped.json"],
  ["Tutorial", "texture-metadata.primary.Tutorial.mapped.json"],
];

const avatar = await readJson(avatarPath);
const runtime = await readJson(runtimePath);
const runtimeByName = new Map(
  runtime.textures.map((texture) => [texture.trueName.toLowerCase(), texture]),
);
const sourcesByRoute = new Map();
for (const [sourceName, file] of sourceFiles) {
  const document = await readJson(path.join(sourceRoot, file));
  for (const texture of document.textures) {
    for (const route of texture.containerRoutes ?? []) {
      const entries = sourcesByRoute.get(route.toLowerCase()) ?? [];
      entries.push({ sourceName, file, document, texture });
      sourcesByRoute.set(route.toLowerCase(), entries);
    }
  }
}

let missingRepairs = 0;
let ambiguousRepairs = 0;
const publishedByIdentity = new Map();
const repairedNames = new Set();
for (const item of avatar.items) {
  for (const visual of [item.male, item.female]) {
    for (const reference of [visual.primaryTexture, visual.secondaryTexture]) {
      if (!reference || !["missing", "ambiguous"].includes(reference.status)) continue;
      const sources = (sourcesByRoute.get(
        `texture/${reference.trueName}.dds`.toLowerCase(),
      ) ?? []).sort(
        (left, right) => sourcePriority(left.sourceName) - sourcePriority(right.sourceName)
          || left.texture.pathId - right.texture.pathId,
      );
      if (sources.length === 0) continue;
      const selected = sources[0];
      const identity = nativeIdentity(selected.texture);
      let nativeAsset = publishedByIdentity.get(identity);
      const existing = runtimeByName.get(reference.trueName.toLowerCase());
      if (
        !nativeAsset
        && existing?.nativeAsset.bytes === selected.texture.nativePngBytes
        && existing.nativeAsset.blake3.toLowerCase()
          === selected.texture.nativePngBlake3.toLowerCase()
      ) {
        nativeAsset = { ...existing.nativeAsset };
      }
      if (!nativeAsset && reference.status === "ambiguous") {
        nativeAsset = reference.candidates.find(
          (candidate) => candidate.bytes === selected.texture.nativePngBytes
            && candidate.blake3.toLowerCase()
              === selected.texture.nativePngBlake3.toLowerCase(),
        );
      }
      if (!nativeAsset) {
        const recovered = path.join(
          recoveryRoot,
          selected.sourceName,
          `${selected.texture.pathId}--${selected.texture.nativePngBlake3}.png`,
        );
        const bytes = await readFile(recovered);
        if (bytes.length !== selected.texture.nativePngBytes) {
          fail(`${recovered} is ${bytes.length} bytes, expected ${selected.texture.nativePngBytes}`);
        }
        const relative = `${publicationRoot}/${portableName(reference.trueName)}`
          + `--${selected.texture.nativePngBlake3.slice(0, 16)}.png`;
        const destination = path.join(assetRoot, ...relative.split("/"));
        await mkdir(path.dirname(destination), { recursive: true });
        await copyFile(recovered, destination);
        nativeAsset = {
          path: relative,
          bytes: selected.texture.nativePngBytes,
          blake3: selected.texture.nativePngBlake3,
        };
      }
      publishedByIdentity.set(identity, nativeAsset);
      if (reference.status === "missing") missingRepairs += 1;
      else ambiguousRepairs += 1;
      reference.status = "verified_unique";
      reference.candidates = [{ ...nativeAsset }];
      repairedNames.add(reference.trueName);
    }
  }
}

await writeFile(avatarPath, `${JSON.stringify(sortJson(avatar), null, 2)}\n`);
process.stdout.write(
  `repaired ${repairedNames.size} logical table texture names `
    + `(${missingRepairs} missing references, ${ambiguousRepairs} ambiguous references); `
    + `${publishedByIdentity.size} exact primary identities published or selected\n`,
);

function sourcePriority(sourceName) {
  const index = sourceFiles.findIndex(([candidate]) => candidate === sourceName);
  return index < 0 ? Number.MAX_SAFE_INTEGER : index;
}

function nativeIdentity(texture) {
  return `${texture.nativePngBlake3.toLowerCase()}:${texture.nativePngBytes}`;
}

function portableName(value) {
  const name = value
    .normalize("NFKD")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
  if (!name) fail(`cannot publish an empty portable name for ${JSON.stringify(value)}`);
  return name;
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
