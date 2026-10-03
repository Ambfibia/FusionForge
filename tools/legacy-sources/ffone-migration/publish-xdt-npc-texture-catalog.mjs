#!/usr/bin/env node

import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { lstat, mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const SOURCE_SCHEMA = "ffone.xdt-npc-render-gallery.v1";
const OUTPUT_SCHEMA = "ffone.xdt-npc-texture-catalog.v1";
const RECEIPT_SCHEMA = "ffone.xdt-npc-texture-catalog-publication.v1";

function fail(message) {
  throw new Error(message);
}

function parseArgs(argv) {
  const values = new Map();
  let apply = false;
  let replaceExisting = false;
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index];
    if (key === "--apply") {
      apply = true;
      continue;
    }
    if (key === "--replace-existing") {
      replaceExisting = true;
      continue;
    }
    if (!key.startsWith("--")) fail(`unexpected argument ${JSON.stringify(key)}`);
    const value = argv[index + 1];
    if (value === undefined || value.startsWith("--")) fail(`${key} requires a value`);
    if (values.has(key)) fail(`duplicate argument ${key}`);
    values.set(key, value);
    index += 1;
  }
  for (const key of [
    "--asset-root",
    "--texture-resolution",
    "--xdt",
    "--output",
    "--receipt",
    "--xdt-source-alias",
    "--xdt-source-build",
  ]) {
    if (!values.has(key)) fail(`missing required argument ${key}`);
  }
  if (replaceExisting && !values.has("--hostile-overlay")) {
    fail("--replace-existing requires --hostile-overlay to preserve non-hostile catalog entries");
  }
  return { apply, replaceExisting, values };
}

async function mustBeRegularFile(file, label) {
  const metadata = await lstat(file).catch((error) => {
    if (error.code === "ENOENT") fail(`${label} does not exist: ${file}`);
    throw error;
  });
  if (!metadata.isFile() || metadata.isSymbolicLink()) fail(`${label} is not a regular file: ${file}`);
  return metadata;
}

async function mustBeDirectory(directory, label) {
  const metadata = await lstat(directory).catch((error) => {
    if (error.code === "ENOENT") fail(`${label} does not exist: ${directory}`);
    throw error;
  });
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
    fail(`${label} is not a regular directory: ${directory}`);
  }
}

async function mustNotExist(target, label) {
  try {
    await lstat(target);
  } catch (error) {
    if (error.code === "ENOENT") return;
    throw error;
  }
  fail(`${label} already exists: ${target}`);
}

async function readJson(file, label) {
  await mustBeRegularFile(file, label);
  try {
    return JSON.parse(await readFile(file, "utf8"));
  } catch (error) {
    fail(`${label} is not valid JSON: ${error.message}`);
  }
}

async function sha256(file) {
  return await new Promise((resolve, reject) => {
    const hash = createHash("sha256");
    const stream = createReadStream(file);
    stream.on("error", reject);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("end", () => resolve(hash.digest("hex")));
  });
}

function sha256Bytes(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function validateRuntimePath(relative) {
  if (
    typeof relative !== "string" ||
    !relative ||
    relative.includes("\\") ||
    path.posix.isAbsolute(relative) ||
    relative.split("/").some((part) => !part || part === "." || part === "..")
  ) {
    fail(`invalid runtime texture path ${JSON.stringify(relative)}`);
  }
}

function sortedUnique(values) {
  return [...new Set(values)].sort((left, right) =>
    typeof left === "number" ? left - right : String(left).localeCompare(String(right)),
  );
}

function sameJson(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function entriesByTrueName(entries, label) {
  if (!Array.isArray(entries)) fail(`${label} must be an array`);
  const result = new Map();
  for (const entry of entries) {
    if (typeof entry?.trueName !== "string" || !entry.trueName) {
      fail(`${label} contains an invalid trueName`);
    }
    const key = entry.trueName.toLowerCase();
    if (result.has(key)) fail(`${label} duplicates ${JSON.stringify(entry.trueName)}`);
    result.set(key, entry);
  }
  return result;
}

function hostileOverlayTextureNames(overlay) {
  if (
    overlay?.schema !== "ffone.retrobution-hostile-visual-overlay.v1" ||
    overlay?.counts?.changedFriendlyOrHnpcRows !== 0 ||
    !Array.isArray(overlay.changes)
  ) {
    fail("hostile overlay must prove zero friendly/HNPC row changes");
  }
  const names = new Set();
  for (const change of overlay.changes) {
    for (const field of ["mainTexture", "subTexture"]) {
      const value = change?.[field];
      if (typeof value !== "string" || !value || value.toLowerCase() === "null") continue;
      if (value.includes("/") || value.includes("\\")) {
        fail(`invalid hostile overlay texture name ${JSON.stringify(value)}`);
      }
      names.add(value.toLowerCase());
    }
  }
  if (names.size === 0) fail("hostile overlay selects no texture names");
  return names;
}

function mergeHostileCatalog(base, fresh, changedNames) {
  if (
    base?.schema !== OUTPUT_SCHEMA ||
    !Array.isArray(base.textures) ||
    !Array.isArray(base.blocked)
  ) {
    fail(`existing runtime texture catalog must use ${OUTPUT_SCHEMA}`);
  }
  const textures = entriesByTrueName(base.textures, "existing textures");
  const blocked = entriesByTrueName(base.blocked, "existing blocked textures");
  const freshTextures = entriesByTrueName(fresh.textures, "fresh textures");
  const freshBlocked = entriesByTrueName(fresh.blocked, "fresh blocked textures");
  for (const key of textures.keys()) {
    if (blocked.has(key)) fail(`existing catalog resolves and blocks ${JSON.stringify(key)}`);
  }
  for (const key of changedNames) {
    if (freshTextures.has(key)) {
      textures.set(key, freshTextures.get(key));
      blocked.delete(key);
    } else if (freshBlocked.has(key)) {
      blocked.set(key, freshBlocked.get(key));
      textures.delete(key);
    } else {
      fail(`fresh audit has no hostile overlay texture ${JSON.stringify(key)}`);
    }
  }
  const byName = (left, right) =>
    left.trueName.toLowerCase().localeCompare(right.trueName.toLowerCase());
  return {
    schema: OUTPUT_SCHEMA,
    textures: [...textures.values()].sort(byName),
    blocked: [...blocked.values()].sort(byName),
  };
}

async function inputProof(role, file) {
  const metadata = await mustBeRegularFile(file, role);
  return {
    role,
    path: path.resolve(file).replaceAll("\\", "/"),
    bytes: metadata.size,
    sha256: await sha256(file),
  };
}

async function main() {
  const { apply, replaceExisting, values } = parseArgs(process.argv.slice(2));
  const assetRoot = path.resolve(values.get("--asset-root"));
  const resolutionPath = path.resolve(values.get("--texture-resolution"));
  const xdtPath = path.resolve(values.get("--xdt"));
  const outputPath = path.resolve(values.get("--output"));
  const receiptPath = path.resolve(values.get("--receipt"));
  const overlayPath = replaceExisting ? path.resolve(values.get("--hostile-overlay")) : null;
  await mustBeDirectory(assetRoot, "asset root");
  if (replaceExisting) {
    await mustBeRegularFile(outputPath, "existing runtime texture catalog");
  } else {
    await mustNotExist(outputPath, "runtime texture catalog");
  }
  const existingCatalog = replaceExisting
    ? await readJson(outputPath, "existing runtime texture catalog")
    : null;
  const hostileOverlay = replaceExisting ? await readJson(overlayPath, "hostile overlay") : null;
  await mustNotExist(receiptPath, "publication receipt");
  const resolution = await readJson(resolutionPath, "texture-resolution evidence");
  await mustBeRegularFile(xdtPath, "expanded XDT");
  if (resolution?.schema !== SOURCE_SCHEMA || !Array.isArray(resolution.textures)) {
    fail(`texture-resolution evidence must use ${SOURCE_SCHEMA}`);
  }

  const groups = new Map();
  for (const entry of resolution.textures) {
    if (!Number.isInteger(entry.appliedBySetupNpc) || entry.appliedBySetupNpc <= 0) continue;
    if (typeof entry.trueName !== "string" || !entry.trueName || entry.trueName.includes("/")) {
      fail(`invalid texture trueName ${JSON.stringify(entry.trueName)}`);
    }
    if (entry.slot !== "main" && entry.slot !== "sub") fail(`invalid XDT texture slot for ${entry.trueName}`);
    const key = entry.trueName.toLowerCase();
    const group = groups.get(key) ?? [];
    group.push(entry);
    groups.set(key, group);
  }

  const textures = [];
  const blocked = [];
  for (const [key, entries] of [...groups].sort(([left], [right]) => left.localeCompare(right))) {
    const resolved = entries.filter((entry) => entry.selectedPath && entry.sampler);
    if (resolved.length > 0 && resolved.length !== entries.length) {
      fail(`texture ${key} is both resolved and blocked across XDT slots`);
    }
    if (resolved.length > 0) {
      const first = resolved[0];
      for (const entry of resolved.slice(1)) {
        if (entry.selectedPath !== first.selectedPath || !sameJson(entry.sampler, first.sampler)) {
          fail(`texture ${key} has contradictory path or sampler selections`);
        }
      }
      if (!first.sampler) fail(`texture ${key} has no verified sampler`);
      const normalizedSampler =
        first.sampler.name.toLowerCase() === key
          ? first.sampler
          : { ...first.sampler, name: first.trueName };
      validateRuntimePath(first.selectedPath);
      const disk = path.join(assetRoot, ...first.selectedPath.split("/"));
      await mustBeRegularFile(disk, `runtime texture ${key}`);
      textures.push({
        trueName: first.trueName,
        path: first.selectedPath,
        sha256: await sha256(disk),
        sampler: normalizedSampler,
      });
      continue;
    }
    blocked.push({
      trueName: entries[0].trueName,
      slots: sortedUnique(entries.map((entry) => entry.slot)),
      rowCount: entries.reduce((sum, entry) => sum + Number(entry.rowCount ?? 0), 0),
      modelStems: sortedUnique(entries.flatMap((entry) => entry.modelStems ?? [])),
      npcNumbers: sortedUnique(entries.flatMap((entry) => entry.npcNumbers ?? [])),
      status: sortedUnique(entries.map((entry) => String(entry.status))).join("+"),
      samplerStatus: sortedUnique(entries.map((entry) => String(entry.samplerStatus))).join("+"),
    });
  }
  if (textures.length === 0) fail("no runtime XDT texture contracts were selected");
  let catalog = { schema: OUTPUT_SCHEMA, textures, blocked };
  let changedNames = new Set();
  if (replaceExisting) {
    changedNames = hostileOverlayTextureNames(hostileOverlay);
    catalog = mergeHostileCatalog(existingCatalog, catalog, changedNames);
  }
  const catalogBytes = Buffer.from(`${JSON.stringify(catalog, null, 2)}\n`, "utf8");
  const inputs = [
    await inputProof("xdt-texture-resolution", resolutionPath),
    await inputProof("expanded-xdt", xdtPath),
  ];
  if (replaceExisting) {
    inputs.push(await inputProof("existing-runtime-texture-catalog", outputPath));
    inputs.push(await inputProof("hostile-overlay", overlayPath));
  }
  const receipt = {
    schema: RECEIPT_SCHEMA,
    status: apply ? "committed" : "planned",
    tableSource: {
      alias: values.get("--xdt-source-alias"),
      buildRelativePath: values.get("--xdt-source-build"),
    },
    textureAuthority: {
      alias: "primary",
      sources: resolution.primarySources ?? [],
      pinnedSelections: resolution.primarySelection ?? [],
    },
    command: ["node", ...process.argv.slice(1)].join(" "),
    inputs,
    output: {
      path: path.relative(assetRoot, outputPath).split(path.sep).join("/"),
      bytes: catalogBytes.length,
      sha256: sha256Bytes(catalogBytes),
    },
    counts: {
      resolvedTextures: catalog.textures.length,
      blockedTextures: catalog.blocked.length,
      hostileOverlayTextureNames: changedNames.size,
      xdtRowsUsingResolvedTextures: resolution.textures
        .filter((entry) => entry.appliedBySetupNpc > 0 && entry.selectedPath && entry.sampler)
        .reduce((sum, entry) => sum + entry.appliedBySetupNpc, 0),
      xdtRowsUsingBlockedTextures: resolution.textures
        .filter((entry) => entry.appliedBySetupNpc > 0 && (!entry.selectedPath || !entry.sampler))
        .reduce((sum, entry) => sum + entry.appliedBySetupNpc, 0),
    },
    intentionalDivergencesFromPrimary: replaceExisting
      ? [
          "Existing catalog entries outside the hostile overlay texture-name set are preserved.",
        ]
      : [],
  };
  if (!apply) {
    console.log(
      `plan passed: ${catalog.textures.length} resolved XDT textures, ${catalog.blocked.length} blocked; ` +
        `catalog bytes=${catalogBytes.length}`,
    );
    return;
  }

  await mkdir(path.dirname(outputPath), { recursive: true });
  await mkdir(path.dirname(receiptPath), { recursive: true });
  const token = `${process.pid}-${Date.now()}`;
  const outputNext = `${outputPath}.next-${token}`;
  const receiptNext = `${receiptPath}.next-${token}`;
  const outputBackup = `${outputPath}.backup-${token}`;
  await mustNotExist(outputNext, "temporary runtime texture catalog");
  await mustNotExist(receiptNext, "temporary publication receipt");
  if (replaceExisting) await mustNotExist(outputBackup, "runtime texture catalog backup");
  let oldOutputMoved = false;
  let newOutputCommitted = false;
  try {
    await writeFile(outputNext, catalogBytes, { flag: "wx" });
    await writeFile(receiptNext, `${JSON.stringify(receipt, null, 2)}\n`, { flag: "wx" });
    if (replaceExisting) {
      await rename(outputPath, outputBackup);
      oldOutputMoved = true;
    }
    await rename(outputNext, outputPath);
    newOutputCommitted = true;
    try {
      await rename(receiptNext, receiptPath);
    } catch (error) {
      await rm(outputPath, { force: true }).catch(() => {});
      newOutputCommitted = false;
      if (oldOutputMoved) {
        await rename(outputBackup, outputPath);
        oldOutputMoved = false;
      }
      throw error;
    }
    if (oldOutputMoved) {
      await rm(outputBackup);
      oldOutputMoved = false;
    }
  } finally {
    if (oldOutputMoved && !newOutputCommitted) {
      await rename(outputBackup, outputPath)
        .then(() => {
          oldOutputMoved = false;
        })
        .catch(() => {});
    }
    await rm(outputNext, { force: true }).catch(() => {});
    await rm(receiptNext, { force: true }).catch(() => {});
    if (!oldOutputMoved) await rm(outputBackup, { force: true }).catch(() => {});
  }
  console.log(
    `published ${catalog.textures.length} resolved XDT NPC textures (${catalog.blocked.length} blocked) to ${outputPath}; ` +
      `receipt=${receiptPath}`,
  );
}

main().catch((error) => {
  console.error(`publish-xdt-npc-texture-catalog: ${error.message}`);
  process.exitCode = 1;
});
