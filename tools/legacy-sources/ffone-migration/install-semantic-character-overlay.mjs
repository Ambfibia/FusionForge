#!/usr/bin/env node

import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import {
  copyFile,
  lstat,
  mkdir,
  readFile,
  readdir,
  rename,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const REGISTRY_SCHEMA = "ffone.semantic-character-registry.v2";
const RECEIPT_SCHEMA = "ffone.semantic-character-overlay-install-receipt.v1";
const REGISTRY_RELATIVE = "_runtime/characters.json";
const CATEGORY_DIRECTORIES = new Map([
  ["nano", "nanos"],
  ["npc", "npcs"],
  ["mob", "mobs"],
  ["fusion", "fusions"],
  ["shared", "shared"],
]);

function fail(message) {
  throw new Error(message);
}

function parseArgs(argv) {
  const values = new Map();
  let apply = false;
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--apply") {
      apply = true;
      continue;
    }
    if (!arg.startsWith("--")) fail(`unexpected positional argument ${JSON.stringify(arg)}`);
    const value = argv[index + 1];
    if (value === undefined || value.startsWith("--")) fail(`${arg} requires a value`);
    if (values.has(arg)) fail(`duplicate argument ${arg}`);
    values.set(arg, value);
    index += 1;
  }
  const required = [
    "--base",
    "--overlay",
    "--receipt",
    "--overlay-report",
    "--source-manifest",
    "--batch-report",
    "--gpu-audit",
    "--source-alias",
    "--source-build",
  ];
  for (const key of required) {
    if (!values.has(key)) fail(`missing required argument ${key}`);
  }
  return { apply, values };
}

function portableRelative(value, label) {
  if (typeof value !== "string" || value.length === 0 || value.includes("\\")) {
    fail(`${label} must be a non-empty slash-separated relative path`);
  }
  const parts = value.split("/");
  if (path.posix.isAbsolute(value) || parts.some((part) => !part || part === "." || part === "..")) {
    fail(`${label} is not a clean relative path: ${JSON.stringify(value)}`);
  }
  return parts;
}

function nativePath(root, relative, label) {
  const parts = portableRelative(relative, label);
  return path.join(root, ...parts);
}

async function regularFile(file, label) {
  const metadata = await lstat(file).catch((error) => {
    if (error.code === "ENOENT") fail(`${label} does not exist: ${file}`);
    throw error;
  });
  if (!metadata.isFile() || metadata.isSymbolicLink()) fail(`${label} is not a regular file: ${file}`);
  return metadata;
}

async function regularDirectory(directory, label) {
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
  await regularFile(file, label);
  let value;
  try {
    value = JSON.parse(await readFile(file, "utf8"));
  } catch (error) {
    fail(`${label} is not valid JSON: ${error.message}`);
  }
  return value;
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

async function inputProof(role, file) {
  const metadata = await regularFile(file, role);
  return {
    role,
    path: path.resolve(file).replaceAll("\\", "/"),
    bytes: metadata.size,
    sha256: await sha256(file),
  };
}

async function walkFiles(root) {
  await regularDirectory(root, "overlay package");
  const pending = [root];
  const files = [];
  while (pending.length > 0) {
    const directory = pending.pop();
    const entries = await readdir(directory, { withFileTypes: true });
    entries.sort((left, right) => left.name.localeCompare(right.name));
    for (const entry of entries) {
      const full = path.join(directory, entry.name);
      if (entry.isSymbolicLink()) fail(`overlay package contains a symlink: ${full}`);
      if (entry.isDirectory()) pending.push(full);
      else if (entry.isFile()) files.push(full);
      else fail(`overlay package contains a non-regular entry: ${full}`);
    }
  }
  files.sort((left, right) => left.localeCompare(right));
  return files;
}

function validateRegistry(registry, label) {
  if (registry?.schema !== REGISTRY_SCHEMA || !Array.isArray(registry.models)) {
    fail(`${label} must use ${REGISTRY_SCHEMA}`);
  }
  const ids = new Set();
  const glbs = new Set();
  for (const model of registry.models) {
    if (!model || typeof model !== "object") fail(`${label} contains a non-object model`);
    const categoryDirectory = CATEGORY_DIRECTORIES.get(model.category);
    if (!categoryDirectory) fail(`${label} model has unsupported category ${JSON.stringify(model.category)}`);
    portableRelative(model.id, `${label} model ID`);
    portableRelative(model.glb, `${label} model GLB`);
    if (!model.id.startsWith(`${model.category}/`)) {
      fail(`${label} model ID/category mismatch: ${model.id}`);
    }
    if (!model.glb.startsWith(`characters/${categoryDirectory}/`)) {
      fail(`${label} model GLB/category mismatch: ${model.glb}`);
    }
    if (typeof model.logicalName !== "string" || !/^[A-Za-z0-9_]+$/.test(model.logicalName)) {
      fail(`${label} model has invalid logicalName: ${JSON.stringify(model.logicalName)}`);
    }
    if (typeof model.glbBlake3 !== "string" || !/^[0-9a-f]{64}$/.test(model.glbBlake3)) {
      fail(`${label} model has invalid GLB BLAKE3: ${model.id}`);
    }
    if (!Array.isArray(model.animations) || model.animations.some((name) => typeof name !== "string")) {
      fail(`${label} model has invalid animation list: ${model.id}`);
    }
    const idKey = model.id.toLowerCase();
    const glbKey = model.glb.toLowerCase();
    if (ids.has(idKey)) fail(`${label} has a case-insensitive ID collision: ${model.id}`);
    if (glbs.has(glbKey)) fail(`${label} has a case-insensitive GLB collision: ${model.glb}`);
    ids.add(idKey);
    glbs.add(glbKey);
  }
  return { ids, glbs };
}

function packageRoot(model) {
  const directory = path.posix.dirname(model.glb);
  const parts = portableRelative(directory, `package root for ${model.id}`);
  if (parts.length !== 3) fail(`model is not in one independently replaceable package: ${model.glb}`);
  if (parts[2] !== model.id.split("/")[1]) fail(`package ID differs from registry ID: ${model.id}`);
  return directory;
}

async function writeFreshJson(file, value) {
  await mustNotExist(file, "receipt output");
  await mkdir(path.dirname(file), { recursive: true });
  const temporary = `${file}.next-${process.pid}-${Date.now()}`;
  await mustNotExist(temporary, "temporary receipt output");
  await writeFile(temporary, `${JSON.stringify(value, null, 2)}\n`, { flag: "wx" });
  await rename(temporary, file);
}

async function main() {
  const { apply, values } = parseArgs(process.argv.slice(2));
  const base = path.resolve(values.get("--base"));
  const overlay = path.resolve(values.get("--overlay"));
  const receipt = path.resolve(values.get("--receipt"));
  const overlayReportPath = path.resolve(values.get("--overlay-report"));
  const sourceManifestPath = path.resolve(values.get("--source-manifest"));
  const batchReportPath = path.resolve(values.get("--batch-report"));
  const gpuAuditPath = path.resolve(values.get("--gpu-audit"));
  await regularDirectory(base, "base asset root");
  await regularDirectory(overlay, "overlay asset root");
  if (base.toLowerCase() === overlay.toLowerCase()) fail("base and overlay roots must differ");
  await mustNotExist(receipt, "receipt output");

  const baseRegistryPath = nativePath(base, REGISTRY_RELATIVE, "base registry path");
  const overlayRegistryPath = nativePath(overlay, REGISTRY_RELATIVE, "overlay registry path");
  const [baseRegistry, overlayRegistry, overlayReport, sourceManifest, batchReport, gpuAudit] =
    await Promise.all([
      readJson(baseRegistryPath, "base character registry"),
      readJson(overlayRegistryPath, "overlay character registry"),
      readJson(overlayReportPath, "overlay install report"),
      readJson(sourceManifestPath, "logical source manifest"),
      readJson(batchReportPath, "logical model batch report"),
      readJson(gpuAuditPath, "GPU evidence audit"),
    ]);
  const baseKeys = validateRegistry(baseRegistry, "base registry");
  const overlayKeys = validateRegistry(overlayRegistry, "overlay registry");
  if (overlayRegistry.models.length === 0) fail("overlay registry contains no models");
  for (const key of overlayKeys.ids) if (baseKeys.ids.has(key)) fail(`overlay ID already exists: ${key}`);
  for (const key of overlayKeys.glbs) if (baseKeys.glbs.has(key)) fail(`overlay GLB already exists: ${key}`);
  if (overlayReport?.status !== "committed" || overlayReport?.productionAssetsMutated !== true) {
    fail("overlay report is not a committed semantic-character install report");
  }
  if (!Array.isArray(overlayReport.published) || overlayReport.published.length !== overlayRegistry.models.length) {
    fail("overlay report/registry model counts differ");
  }
  if (batchReport?.structuralAuditPassed !== true || !Array.isArray(batchReport.models)) {
    fail("batch report is not structurally accepted");
  }
  if (gpuAudit?.automatedGpuPassed !== true || !Array.isArray(gpuAudit.models)) {
    fail("GPU audit is not an automated acceptance report");
  }
  if (gpuAudit.models.length !== overlayRegistry.models.length || gpuAudit.models.some((model) => model.passed !== true)) {
    fail("GPU audit does not accept the complete overlay model set");
  }

  const sourceByRoute = new Map(
    (sourceManifest.exported ?? []).map((entry) => [String(entry.exactRoute).toLowerCase(), entry]),
  );
  const publishedById = new Map(overlayReport.published.map((entry) => [entry.id, entry]));
  const packages = [];
  const fileProofs = [];
  let installedBytes = 0;
  for (const model of overlayRegistry.models) {
    const published = publishedById.get(model.id);
    if (!published || published.glb !== model.glb || published.glbBlake3 !== model.glbBlake3) {
      fail(`overlay report identity differs for ${model.id}`);
    }
    const source = sourceByRoute.get(String(published.legacyRoute).toLowerCase());
    if (!source || source.sourceSha256 !== published.sourceSha256) {
      fail(`source manifest identity differs for ${model.id}`);
    }
    const rootRelative = packageRoot(model);
    const sourceRoot = nativePath(overlay, rootRelative, `overlay package ${model.id}`);
    const destinationRoot = nativePath(base, rootRelative, `destination package ${model.id}`);
    await mustNotExist(destinationRoot, `destination package ${model.id}`);
    const modelGlb = nativePath(overlay, model.glb, `overlay GLB ${model.id}`);
    await regularFile(modelGlb, `overlay GLB ${model.id}`);
    const files = await walkFiles(sourceRoot);
    const packageFiles = [];
    for (const file of files) {
      const tail = path.relative(sourceRoot, file);
      const relative = path.posix.join(rootRelative, ...tail.split(path.sep));
      const metadata = await stat(file);
      const hash = await sha256(file);
      installedBytes += metadata.size;
      const proof = { path: relative, bytes: metadata.size, sha256: hash };
      packageFiles.push(proof);
      fileProofs.push(proof);
    }
    packages.push({ model, published, source, rootRelative, sourceRoot, destinationRoot, files, packageFiles });
  }
  const glbPaths = new Set(fileProofs.filter((file) => file.path.endsWith(".glb")).map((file) => file.path));
  if (glbPaths.size !== overlayRegistry.models.length) fail("overlay package closure does not contain one GLB per model");

  const mergedRegistry = {
    schema: REGISTRY_SCHEMA,
    models: [...baseRegistry.models, ...overlayRegistry.models].sort((left, right) =>
      left.glb.localeCompare(right.glb),
    ),
  };
  validateRegistry(mergedRegistry, "merged registry");
  const mergedRegistryBytes = Buffer.from(`${JSON.stringify(mergedRegistry, null, 2)}\n`, "utf8");
  const inputPaths = [
    ["base-character-registry", baseRegistryPath],
    ["overlay-character-registry", overlayRegistryPath],
    ["overlay-install-report", overlayReportPath],
    ["logical-source-manifest", sourceManifestPath],
    ["logical-model-batch-report", batchReportPath],
    ["gpu-evidence-audit", gpuAuditPath],
  ];
  const inputs = [];
  for (const [role, file] of inputPaths) inputs.push(await inputProof(role, file));
  const categoryCounts = Object.fromEntries([...CATEGORY_DIRECTORIES.keys()].map((category) => [category, 0]));
  for (const model of overlayRegistry.models) categoryCounts[model.category] += 1;
  const receiptValue = {
    schema: RECEIPT_SCHEMA,
    status: apply ? "committed" : "planned",
    source: {
      alias: values.get("--source-alias"),
      buildRelativePath: values.get("--source-build"),
      role: "primary",
    },
    command: ["node", ...process.argv.slice(1)].join(" "),
    converter: {
      overlaySchema: overlayReport.schema,
      batchSchema: batchReport.schema,
      gpuAuditSchema: gpuAudit.schema,
    },
    inputs,
    registry: {
      path: REGISTRY_RELATIVE,
      modelsBefore: baseRegistry.models.length,
      modelsAdded: overlayRegistry.models.length,
      modelsAfter: mergedRegistry.models.length,
      beforeSha256: await sha256(baseRegistryPath),
      overlaySha256: await sha256(overlayRegistryPath),
      afterSha256: sha256Bytes(mergedRegistryBytes),
    },
    counts: {
      packages: packages.length,
      files: fileProofs.length,
      glbs: glbPaths.size,
      pngs: fileProofs.filter((file) => file.path.endsWith(".png")).length,
      bytes: installedBytes,
      categories: categoryCounts,
    },
    models: packages.map(({ model, published, source, packageFiles }) => ({
      id: model.id,
      logicalName: model.logicalName,
      category: model.category,
      legacyRoute: published.legacyRoute,
      glb: model.glb,
      glbBlake3: model.glbBlake3,
      animations: model.animations,
      sourceDocument: source.sourceRelativePath,
      sourceSha256: source.sourceSha256,
      sourceOwnership: source.sourceOwnership,
      packageFiles,
    })),
    intentionalDivergencesFromPrimary: [],
  };

  if (!apply) {
    console.log(
      `plan passed: add ${packages.length} packages (${categoryCounts.fusion} fusions, ${categoryCounts.mob} mobs), ` +
        `${fileProofs.length} files and ${installedBytes} bytes; registry ${baseRegistry.models.length} -> ${mergedRegistry.models.length}`,
    );
    return;
  }

  const token = `${process.pid}-${Date.now()}`;
  const stage = path.join(base, `.semantic-character-overlay-stage-${token}`);
  const registryNext = path.join(path.dirname(baseRegistryPath), `.characters.json.next-${token}`);
  const registryBackup = path.join(path.dirname(baseRegistryPath), `.characters.json.backup-${token}`);
  await mustNotExist(stage, "transaction stage");
  await mustNotExist(registryNext, "next registry");
  await mustNotExist(registryBackup, "registry backup");
  await mkdir(stage);
  const installed = [];
  let registryBackedUp = false;
  let registryCommitted = false;
  try {
    for (const item of packages) {
      const stageRoot = nativePath(stage, item.rootRelative, `stage package ${item.model.id}`);
      await mkdir(stageRoot, { recursive: true });
      for (const sourceFile of item.files) {
        const tail = path.relative(item.sourceRoot, sourceFile);
        const destination = path.join(stageRoot, tail);
        await mkdir(path.dirname(destination), { recursive: true });
        await copyFile(sourceFile, destination);
        if ((await sha256(destination)) !== (await sha256(sourceFile))) {
          fail(`staged copy SHA-256 mismatch: ${sourceFile}`);
        }
      }
    }
    await writeFile(registryNext, mergedRegistryBytes, { flag: "wx" });
    for (const item of packages) {
      const stageRoot = nativePath(stage, item.rootRelative, `stage package ${item.model.id}`);
      await mkdir(path.dirname(item.destinationRoot), { recursive: true });
      await rename(stageRoot, item.destinationRoot);
      installed.push(item.destinationRoot);
    }
    await rename(baseRegistryPath, registryBackup);
    registryBackedUp = true;
    await rename(registryNext, baseRegistryPath);
    registryCommitted = true;
    await writeFreshJson(receipt, receiptValue);
  } catch (error) {
    if (registryCommitted) {
      await rm(baseRegistryPath, { force: true }).catch(() => {});
      registryCommitted = false;
    }
    if (registryBackedUp) {
      await rename(registryBackup, baseRegistryPath).catch(() => {});
      registryBackedUp = false;
    }
    for (const destination of installed.reverse()) {
      await rm(destination, { recursive: true, force: true }).catch(() => {});
    }
    throw error;
  } finally {
    await rm(registryNext, { force: true }).catch(() => {});
    if (registryCommitted) await rm(registryBackup, { force: true }).catch(() => {});
    await rm(stage, { recursive: true, force: true }).catch(() => {});
  }
  console.log(
    `installed ${packages.length} primary semantic character packages (${fileProofs.length} files, ${installedBytes} bytes); ` +
      `registry ${baseRegistry.models.length} -> ${mergedRegistry.models.length}; receipt=${receipt}`,
  );
}

main().catch((error) => {
  console.error(`install-semantic-character-overlay: ${error.message}`);
  process.exitCode = 1;
});
