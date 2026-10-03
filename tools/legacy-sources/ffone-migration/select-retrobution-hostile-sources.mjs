#!/usr/bin/env node

import { createHash } from "node:crypto";
import { copyFile, lstat, mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const OVERLAY_SCHEMA = "ffone.retrobution-hostile-visual-overlay.v1";
const SOURCE_SCHEMA = "ffone.logical-model-source.v1";
const REPORT_SCHEMA = "ffone.retrobution-hostile-source-selection.v1";

function fail(message) {
  throw new Error(message);
}

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; ) {
    const key = argv[index];
    if (key === "--allow-blocked") {
      if (values.has(key)) fail(`duplicate argument ${key}`);
      values.set(key, true);
      index += 1;
      continue;
    }
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined || value.startsWith("--")) {
      fail("usage: select-retrobution-hostile-sources --selection <OVERLAY_REPORT> --source-manifest <JSON> --source-root <DIR> --output-root <DIR> --report <JSON> [--allow-blocked]");
    }
    if (values.has(key)) fail(`duplicate argument ${key}`);
    values.set(key, value);
    index += 2;
  }
  for (const key of ["--selection", "--source-manifest", "--source-root", "--output-root", "--report"]) {
    if (!values.has(key)) fail(`missing required argument ${key}`);
  }
  return values;
}

async function readJson(file, label) {
  const bytes = await readFile(file).catch((error) => fail(`${label} is unreadable: ${error.message}`));
  try {
    return { bytes, value: JSON.parse(bytes.toString("utf8")) };
  } catch (error) {
    fail(`${label} is not valid JSON: ${error.message}`);
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

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function parseRoute(route) {
  if (typeof route !== "string" || !/^mob\/[a-z0-9_]+\.kfm$/.test(route)) {
    fail(`unsafe or unsupported exact route ${JSON.stringify(route)}`);
  }
  const stem = path.posix.basename(route, ".kfm");
  return { family: "mob", stem };
}

async function writeFreshJson(file, value) {
  await mustNotExist(file, "report output");
  await mkdir(path.dirname(file), { recursive: true });
  const next = `${file}.next-${process.pid}`;
  await writeFile(next, `${JSON.stringify(value, null, 2)}\n`, { flag: "wx" });
  await rename(next, file);
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const selectionPath = path.resolve(args.get("--selection"));
  const sourceManifestPath = path.resolve(args.get("--source-manifest"));
  const sourceRoot = path.resolve(args.get("--source-root"));
  const outputRoot = path.resolve(args.get("--output-root"));
  const reportPath = path.resolve(args.get("--report"));
  const allowBlocked = args.has("--allow-blocked");
  await mustNotExist(outputRoot, "selected source root");
  await mustNotExist(reportPath, "report output");

  const [selection, sourceManifest] = await Promise.all([
    readJson(selectionPath, "hostile overlay report"),
    readJson(sourceManifestPath, "logical source manifest"),
  ]);
  if (selection.value?.schema !== OVERLAY_SCHEMA || selection.value?.status !== "ready") {
    fail(`selection must be a ready ${OVERLAY_SCHEMA}`);
  }
  if (!Array.isArray(selection.value.selectedModels) || selection.value.selectedModels.length === 0) {
    fail("selection has no hostile models");
  }
  if (
    sourceManifest.value?.schema !== "ffclient.logical-model-source-batch.v1" ||
    !Array.isArray(sourceManifest.value.exported)
  ) {
    fail("source manifest has the wrong schema or no exported array");
  }
  const sourceByRoute = new Map();
  for (const entry of sourceManifest.value.exported) {
    const route = entry.exactRoute?.toLowerCase();
    if (typeof route !== "string" || typeof entry.sourceRelativePath !== "string") continue;
    if (sourceByRoute.has(route)) fail(`source manifest duplicates exact route ${route}`);
    sourceByRoute.set(route, entry);
  }

  const planned = [];
  const blocked = [];
  const seen = new Set();
  for (const model of selection.value.selectedModels) {
    const route = model.exactRoute;
    if (seen.has(route)) fail(`duplicate selected route ${route}`);
    seen.add(route);
    parseRoute(route);
    const manifestEntry = sourceByRoute.get(route);
    if (!manifestEntry) {
      if (!allowBlocked) fail(`source manifest has no exported report for ${route}`);
      blocked.push({
        exactRoute: route,
        model: model.model,
        code: "sourceManifestRouteNotExported",
        detail: "the full primary ownership scan did not export an unambiguous exact source",
      });
      continue;
    }
    const relative = manifestEntry.sourceRelativePath.replaceAll("\\", "/");
    if (path.posix.isAbsolute(relative) || relative.split("/").some((part) => !part || part === "." || part === "..")) {
      fail(`source manifest has unsafe relative path for ${route}`);
    }
    const source = path.join(sourceRoot, ...relative.split("/"));
    const sourceReport = await readJson(source, `source report for ${route}`);
    if (
      sourceReport.value?.schema !== SOURCE_SCHEMA ||
      sourceReport.value?.status !== "ready" ||
      sourceReport.value?.selectionMode !== "exact-container-route" ||
      sourceReport.value?.exactContainerRoute?.toLowerCase() !== route
    ) {
      fail(`source report does not prove selected exact route ${route}`);
    }
    planned.push({
      exactRoute: route,
      logicalName: sourceReport.value.logicalName,
      source,
      bytes: sourceReport.bytes.length,
      sha256: sha256(sourceReport.bytes),
      relative,
    });
  }
  planned.sort((left, right) => left.exactRoute.localeCompare(right.exactRoute));
  blocked.sort((left, right) => left.exactRoute.localeCompare(right.exactRoute));

  await mkdir(outputRoot, { recursive: false });
  try {
    for (const entry of planned) {
      const target = path.join(outputRoot, ...entry.relative.split("/"));
      await mkdir(path.dirname(target), { recursive: true });
      await copyFile(entry.source, target);
    }
  } catch (error) {
    fail(`selected source copy failed after creating ${outputRoot}: ${error.message}`);
  }

  const report = {
    schema: REPORT_SCHEMA,
    status: "ready",
    sourceAlias: "retrobution",
    sourceRole: "primary",
    selection: {
      path: selectionPath.replaceAll("\\", "/"),
      bytes: selection.bytes.length,
      sha256: sha256(selection.bytes),
    },
    sourceManifest: {
      path: sourceManifestPath.replaceAll("\\", "/"),
      bytes: sourceManifest.bytes.length,
      sha256: sha256(sourceManifest.bytes),
    },
    sourceRoot: sourceRoot.replaceAll("\\", "/"),
    outputRoot: outputRoot.replaceAll("\\", "/"),
    counts: { selected: planned.length, blocked: blocked.length },
    selected: planned.map(({ source, ...entry }) => ({
      ...entry,
      source: source.replaceAll("\\", "/"),
    })),
    blocked,
  };
  await writeFreshJson(reportPath, report);
  process.stdout.write(
    `selected ${planned.length} exact hostile source reports; blocked ${blocked.length}\n`,
  );
}

main().catch((error) => {
  process.stderr.write(`${error.stack ?? error.message}\n`);
  process.exitCode = 1;
});
