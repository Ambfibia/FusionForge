#!/usr/bin/env node

import { createHash } from "node:crypto";
import { lstat, mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const TABLE_SET_SCHEMA = "ffone.table-set.v1";
const CONSOLIDATED_TABLE = "npc_imports_consolidated";
const REPORT_SCHEMA = "ffone.retrobution-hostile-visual-overlay.v1";
const VISUAL_FIELDS = ["m_iMesh", "m_iTexture", "m_iTexture2"];

function fail(message) {
  throw new Error(message);
}

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined || value.startsWith("--")) {
      fail("usage: prepare-retrobution-hostile-overlay --base <TABLE_SET> --primary-xdt <XDT_JSON> --output <TABLE_SET> --report <REPORT> --source-build <ID>");
    }
    if (values.has(key)) fail(`duplicate argument ${key}`);
    values.set(key, value);
  }
  for (const key of ["--base", "--primary-xdt", "--output", "--report", "--source-build"]) {
    if (!values.has(key)) fail(`missing required argument ${key}`);
  }
  return values;
}

async function readJson(file, label) {
  let bytes;
  try {
    bytes = await readFile(file);
  } catch (error) {
    fail(`${label} is unreadable: ${error.message}`);
  }
  try {
    return { bytes, value: JSON.parse(bytes.toString("utf8")) };
  } catch (error) {
    fail(`${label} is not valid JSON: ${error.message}`);
  }
}

async function mustNotExist(file, label) {
  try {
    await lstat(file);
  } catch (error) {
    if (error.code === "ENOENT") return;
    throw error;
  }
  fail(`${label} already exists: ${file}`);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function stableRow(row) {
  return JSON.stringify(row);
}

function npcNumber(row, label) {
  const value = row?.m_iNpcNumber;
  if (!Number.isSafeInteger(value)) fail(`${label} has no safe m_iNpcNumber`);
  return value;
}

function isHostile(row) {
  return row?.m_iTeam === 2 && row?.m_iHNpc === 0;
}

function modelStem(mesh, label) {
  const value = mesh?.m_pstrMMeshModelString;
  if (typeof value !== "string" || !/^[A-Za-z0-9_]+$/.test(value)) {
    fail(`${label} has invalid m_pstrMMeshModelString ${JSON.stringify(value)}`);
  }
  return value;
}

function exactRoute(stem) {
  return `mob/${stem.toLowerCase()}.kfm`;
}

function consolidatedValue(tableSet) {
  if (tableSet?.schema !== TABLE_SET_SCHEMA || !Array.isArray(tableSet.tables)) {
    fail(`base table-set must use ${TABLE_SET_SCHEMA}`);
  }
  const matches = tableSet.tables.filter((table) => table?.name === CONSOLIDATED_TABLE);
  if (matches.length !== 1 || typeof matches[0].value !== "object") {
    fail(`base table-set must contain exactly one ${CONSOLIDATED_TABLE}`);
  }
  return matches[0];
}

function npcTables(root, label) {
  const table = root?.m_pNpcTable;
  if (!table || !Array.isArray(table.m_pNpcData) || !Array.isArray(table.m_pNpcMeshData)) {
    fail(`${label} has no typed m_pNpcTable arrays`);
  }
  return { rows: table.m_pNpcData, meshes: table.m_pNpcMeshData };
}

function uniqueRowsByNpcNumber(rows, label) {
  const result = new Map();
  for (const [index, row] of rows.entries()) {
    const number = npcNumber(row, `${label}[${index}]`);
    if (result.has(number)) fail(`${label} duplicates m_iNpcNumber ${number}`);
    result.set(number, { index, row });
  }
  return result;
}

async function writeFreshJson(file, value) {
  await mustNotExist(file, "output");
  await mkdir(path.dirname(file), { recursive: true });
  const next = `${file}.next-${process.pid}`;
  await mustNotExist(next, "temporary output");
  await writeFile(next, `${JSON.stringify(value, null, 2)}\n`, { flag: "wx" });
  await rename(next, file);
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const basePath = path.resolve(args.get("--base"));
  const primaryPath = path.resolve(args.get("--primary-xdt"));
  const outputPath = path.resolve(args.get("--output"));
  const reportPath = path.resolve(args.get("--report"));
  if (outputPath === basePath || reportPath === basePath || outputPath === reportPath) {
    fail("base, output and report paths must be distinct");
  }
  const [{ bytes: baseBytes, value: base }, { bytes: primaryBytes, value: primary }] =
    await Promise.all([
      readJson(basePath, "base table-set"),
      readJson(primaryPath, "primary XDT"),
      mustNotExist(outputPath, "table output"),
      mustNotExist(reportPath, "report output"),
    ]);

  const tableDocument = consolidatedValue(base);
  const baseTables = npcTables(tableDocument.value, "base table-set");
  const primaryTables = npcTables(primary, "primary XDT");
  const baseMeshRowsBefore = baseTables.meshes.length;
  const baseByNumber = uniqueRowsByNpcNumber(baseTables.rows, "base NPC rows");
  const primaryByNumber = uniqueRowsByNpcNumber(primaryTables.rows, "primary NPC rows");
  const nonHostileBefore = new Map(
    baseTables.rows
      .filter((row) => !isHostile(row))
      .map((row) => [npcNumber(row, "base non-hostile row"), stableRow(row)]),
  );

  const meshByIdentity = new Map();
  for (const [index, mesh] of baseTables.meshes.entries()) {
    const identity = stableRow(mesh);
    if (!meshByIdentity.has(identity)) meshByIdentity.set(identity, index);
  }

  const changes = [];
  for (const [number, primaryEntry] of primaryByNumber) {
    if (!isHostile(primaryEntry.row)) continue;
    const baseEntry = baseByNumber.get(number);
    if (!baseEntry) fail(`primary hostile NPC ${number} is absent from the base table-set`);
    if (!isHostile(baseEntry.row)) {
      fail(`primary hostile NPC ${number} contradicts the base team/HNPC role`);
    }

    const sourceMeshIndex = primaryEntry.row.m_iMesh;
    if (!Number.isSafeInteger(sourceMeshIndex) || !primaryTables.meshes[sourceMeshIndex]) {
      fail(`primary hostile NPC ${number} references invalid mesh ${sourceMeshIndex}`);
    }
    const sourceMesh = structuredClone(primaryTables.meshes[sourceMeshIndex]);
    const baseMeshIndex = baseEntry.row.m_iMesh;
    if (!Number.isSafeInteger(baseMeshIndex) || !baseTables.meshes[baseMeshIndex]) {
      fail(`base hostile NPC ${number} references invalid mesh ${baseMeshIndex}`);
    }
    const visualFieldsChanged = VISUAL_FIELDS.some(
      (field) => baseEntry.row[field] !== primaryEntry.row[field],
    );
    const meshDefinitionChanged =
      stableRow(baseTables.meshes[baseMeshIndex]) !== stableRow(sourceMesh);
    if (!visualFieldsChanged && !meshDefinitionChanged) continue;
    const identity = stableRow(sourceMesh);
    let installedMeshIndex = meshByIdentity.get(identity);
    if (installedMeshIndex === undefined) {
      installedMeshIndex = baseTables.meshes.length;
      baseTables.meshes.push(sourceMesh);
      meshByIdentity.set(identity, installedMeshIndex);
    }
    const previous = Object.fromEntries(VISUAL_FIELDS.map((field) => [field, baseEntry.row[field]]));
    baseEntry.row.m_iMesh = installedMeshIndex;
    baseEntry.row.m_iTexture = primaryEntry.row.m_iTexture;
    baseEntry.row.m_iTexture2 = primaryEntry.row.m_iTexture2;
    changes.push({
      npcNumber: number,
      baseRowIndex: baseEntry.index,
      primaryRowIndex: primaryEntry.index,
      previous,
      primary: {
        m_iMesh: sourceMeshIndex,
        m_iTexture: primaryEntry.row.m_iTexture,
        m_iTexture2: primaryEntry.row.m_iTexture2,
      },
      installedMeshIndex,
      model: modelStem(sourceMesh, `primary mesh ${sourceMeshIndex}`),
      mainTexture: sourceMesh.m_pstrMTextureString ?? null,
      subTexture: sourceMesh.m_pstrMTextureString2 ?? null,
    });
  }
  changes.sort((left, right) => left.npcNumber - right.npcNumber);

  const changedNumbers = new Set(changes.map((change) => change.npcNumber));
  for (const row of baseTables.rows) {
    const number = npcNumber(row, "merged NPC row");
    if (changedNumbers.has(number)) {
      if (!isHostile(row)) fail(`changed NPC ${number} is not hostile after merge`);
      continue;
    }
    const original = baseByNumber.get(number).row;
    if (stableRow(row) !== stableRow(original)) {
      fail(`non-target NPC ${number} changed during hostile overlay`);
    }
  }
  for (const [number, before] of nonHostileBefore) {
    if (stableRow(baseByNumber.get(number).row) !== before) {
      fail(`friendly/HNPC row ${number} changed during hostile overlay`);
    }
  }

  const references = new Map();
  for (const row of baseTables.rows) {
    const mesh = baseTables.meshes[row.m_iMesh];
    if (!mesh) fail(`merged NPC ${row.m_iNpcNumber} references invalid mesh ${row.m_iMesh}`);
    const rawStem = mesh.m_pstrMMeshModelString;
    if (rawStem === "") continue;
    if (typeof rawStem !== "string" || !/^[A-Za-z0-9_]+$/.test(rawStem)) {
      fail(`merged mesh ${row.m_iMesh} has unsafe model stem ${JSON.stringify(rawStem)}`);
    }
    const stem = rawStem.toLowerCase();
    const entry = references.get(stem) ?? { hostileNpcNumbers: [], excludedNpcNumbers: [] };
    (isHostile(row) ? entry.hostileNpcNumbers : entry.excludedNpcNumbers).push(row.m_iNpcNumber);
    references.set(stem, entry);
  }

  const selectedModels = [];
  const sharedModels = [];
  for (const [stem, refs] of references) {
    refs.hostileNpcNumbers.sort((a, b) => a - b);
    refs.excludedNpcNumbers.sort((a, b) => a - b);
    if (refs.hostileNpcNumbers.length === 0) continue;
    const record = { model: stem, exactRoute: exactRoute(stem), ...refs };
    if (refs.excludedNpcNumbers.length === 0) selectedModels.push(record);
    else sharedModels.push(record);
  }
  selectedModels.sort((left, right) => left.exactRoute.localeCompare(right.exactRoute));
  sharedModels.sort((left, right) => left.exactRoute.localeCompare(right.exactRoute));

  const oldKey = tableDocument.key;
  tableDocument.key = sha256(Buffer.from(JSON.stringify(tableDocument.value)));
  const report = {
    schema: REPORT_SCHEMA,
    status: "ready",
    sourceAlias: "retrobution",
    sourceRole: "primary",
    sourceBuild: args.get("--source-build"),
    policy: {
      rowSelection: "m_iTeam == 2 && m_iHNpc == 0",
      mutableNpcFields: VISUAL_FIELDS,
      meshPolicy: "append exact primary rows and remap hostile rows only",
      modelSelection: "hostile-referenced stems with zero friendly/HNPC references",
    },
    inputs: {
      baseTableSet: { path: basePath.replaceAll("\\", "/"), bytes: baseBytes.length, sha256: sha256(baseBytes) },
      primaryXdt: { path: primaryPath.replaceAll("\\", "/"), bytes: primaryBytes.length, sha256: sha256(primaryBytes) },
    },
    output: {
      tableSet: outputPath.replaceAll("\\", "/"),
      previousKey: oldKey,
      key: tableDocument.key,
    },
    counts: {
      baseNpcRows: baseTables.rows.length,
      primaryNpcRows: primaryTables.rows.length,
      changedHostileRows: changes.length,
      appendedMeshRows: baseTables.meshes.length - baseMeshRowsBefore,
      selectedHostileModels: selectedModels.length,
      excludedSharedModels: sharedModels.length,
      changedFriendlyOrHnpcRows: 0,
    },
    changes,
    selectedModels,
    excludedSharedModels: sharedModels,
  };

  await writeFreshJson(outputPath, base);
  await writeFreshJson(reportPath, report);
  process.stdout.write(
    `prepared hostile overlay: changedRows=${changes.length}, meshRows=${baseTables.meshes.length}, selectedModels=${selectedModels.length}, sharedExcluded=${sharedModels.length}\n`,
  );
}

main().catch((error) => {
  process.stderr.write(`${error.stack ?? error.message}\n`);
  process.exitCode = 1;
});
