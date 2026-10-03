#!/usr/bin/env node

import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, relative, resolve } from "node:path";

const EDITOR_ROOT = resolve(import.meta.dirname, "../..");
const args = process.argv.slice(2);

function optionValue(name) {
  const positions = args.flatMap((value, index) => (value === name ? [index] : []));
  if (positions.length > 1) throw new Error(`${name} may be specified only once`);
  if (!positions.length) return undefined;
  const value = args[positions[0] + 1];
  if (!value || value.startsWith("--")) throw new Error(`${name} requires a path`);
  return value;
}

const HELP = args.includes("--help");
if (HELP) {
  process.stdout.write(
    "usage: node tools/legacy-sources/import-tabledata-localization.mjs --native-target <assets/game> --world-name-object <editor-work-object.json> [--translate-missing] [--dry-run]\n",
  );
  process.exit(0);
}
const knownOptions = new Set([
  "--native-target",
  "--world-name-object",
  "--translate-missing",
  "--dry-run",
]);
for (let index = 0; index < args.length; index += 1) {
  const argument = args[index];
  if (!knownOptions.has(argument)) throw new Error(`unknown option ${argument}`);
  if (argument === "--native-target" || argument === "--world-name-object") index += 1;
}
const nativeTargetArgument = optionValue("--native-target");
const worldNameArgument = optionValue("--world-name-object");
if (!nativeTargetArgument || !worldNameArgument) {
  throw new Error("--native-target and --world-name-object are required; use --help for usage");
}
const NATIVE_TARGET = resolve(nativeTargetArgument);
const TABLE_SET = resolve(NATIVE_TARGET, "data/tables/table-set.json");
const WORLD_NAME_OBJECT = resolve(worldNameArgument);
const RUNTIME_EN = resolve(NATIVE_TARGET, "localization/en.json");
const RUNTIME_RU = resolve(NATIVE_TARGET, "localization/ru.json");
const REPORT = resolve(
  EDITOR_ROOT,
  "work/legacy-sources/tabledata-localization/import-report.json",
);
const PREFIX = "content.tabledata.";
const MISSION_PREFIX = "content.mission.task.";
const NPC_NAME_PREFIX = "content.npc.";
const QUEST_ITEM_NAME_PREFIX = "content.quest_item.";
const WORLD_LOCATION_PREFIX = "content.location.world.";
const TRANSLATE = args.includes("--translate-missing");
const DRY_RUN = args.includes("--dry-run");
const CONCURRENCY = 8;
const MAX_BATCH_CHARS = 3_400;

// These are the user-visible string owners in clean XDT TableData. Numeric
// reference columns, model/texture/audio paths, animation names, chat filters,
// and character-name construction fragments are deliberately excluded.
const DISPLAY_FIELDS = {
  m_pBackItemTable: { m_pItemStringData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
  m_pChestItemTable: { m_pItemStringData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
  m_pChatTable: { m_pChatStringData: ["m_pstrFMeshModelString"] },
  m_pClassSkillTable: { m_pSkillString: ["m_ClassName", "m_SkillName", "m_WpnName", "m_skillAccount1", "m_skillAccount2"] },
  m_pClassTable: { m_pClassString: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
  m_pFaceItemTable: { m_pItemStringData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
  m_pFirstUseTable: { m_pFirstUseString: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
  m_pGeneralItemTable: { m_pItemStringData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
  m_pGlassItemTable: { m_pItemStringData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
  m_pGuideTable: { m_pGuideStringData: ["m_pszString"] },
  m_pHatItemTable: { m_pItemStringData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
  m_pHeadItemTable: { m_pItemStringData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
  m_pHelpTable: {
    m_pHelpPageString: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"],
    m_pHelpString: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"],
  },
  m_pInstanceTable: { m_pWarpNameData: ["m_pstrNameString"] },
  m_pMessageTable: { m_pMessageData: ["m_szString"] },
  m_pMissionTable: { m_pMissionStringData: ["m_pstrNameString"] },
  m_pNanoTable: {
    m_pNanoStringData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"],
    m_pNanoTuneStringData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"],
  },
  m_pNpcTable: {
    m_pNpcBarkerData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"],
    m_pNpcStringData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"],
  },
  m_pPantsItemTable: { m_pItemStringData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
  m_pQuestItemTable: { m_pItemStringData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
  m_pRulesTable: { m_pRulesString: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
  m_pShinyTable: { m_pShinyStringData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
  m_pShirtsItemTable: { m_pItemStringData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
  m_pShoesItemTable: { m_pItemStringData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
  m_pSkillBookTable: { m_pSkillBookString: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
  m_pSkillTable: { m_pSkillStringData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
  m_pTransportationTable: {
    m_pBroomstickString: ["m_pstrLocationName", "m_pstrLocationInfo"],
    m_pTransportationWarpString: ["m_pstrLocationName", "m_pstrLocationInfo"],
  },
  m_pVehicleItemTable: { m_pItemStringData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
  m_pWeaponItemTable: { m_pItemStringData: ["m_strName", "m_strComment", "m_strComment1", "m_strComment2"] },
};

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

function token(name) {
  return name
    .replace(/^m_[pi]+/, "")
    .replace(/^m_/, "")
    .replace(/Table$/, "")
    .replace(/Data$/, "")
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .replace(/__+/g, "_")
    .toLowerCase();
}

function importedEntries(value) {
  const entries = new Map();
  for (const [tableName, collections] of Object.entries(DISPLAY_FIELDS)) {
    const table = value[tableName];
    if (!table) throw new Error(`TableData is missing ${tableName}`);
    for (const [collectionName, fields] of Object.entries(collections)) {
      const rows = table[collectionName];
      if (!Array.isArray(rows)) {
        throw new Error(`TableData is missing ${tableName}.${collectionName}`);
      }
      rows.forEach((row, index) => {
        for (const field of fields) {
          if (!(field in row)) {
            throw new Error(`TableData is missing ${tableName}.${collectionName}[${index}].${field}`);
          }
          const text = row[field];
          if (typeof text !== "string" || text.trim() === "") continue;
          const key = `${PREFIX}${token(tableName)}.${token(collectionName)}.${index}.${token(field)}`;
          entries.set(key, text);
        }
      });
    }
  }
  return new Map([...entries].sort(([a], [b]) => a.localeCompare(b)));
}

function addMissionAliases(entries, value) {
  const missionTable = value.m_pMissionTable;
  const strings = missionTable.m_pMissionStringData;
  const journals = missionTable.m_pJournalData;
  const stringAt = (id, context) => {
    const row = strings[id];
    if (!row || typeof row.m_pstrNameString !== "string") {
      throw new Error(`${context} references missing mission string ${id}`);
    }
    return row.m_pstrNameString;
  };
  let aliases = 0;
  for (const [rowIndex, row] of missionTable.m_pMissionData.entries()) {
    const taskId = row.m_iHTaskID;
    if (!Number.isInteger(taskId) || taskId <= 0) continue;
    const journal = journals[row.m_iSTJournalIDAdd];
    if (!journal) {
      throw new Error(`m_pMissionData[${rowIndex}] references missing journal ${row.m_iSTJournalIDAdd}`);
    }
    const fields = {
      title: row.m_iHMissionName,
      objective: row.m_iHCurrentObjective,
      offer_description: journal.m_iDetaileMissionDesc,
      task_description: journal.m_iDetailedTaskDesc,
      mission_summary: journal.m_iMissionSummary,
      mission_complete_summary: journal.m_iMissionCompleteSummary,
      completion_description: journal.m_iDetaileMissionCompleteSummary,
    };
    for (const [field, stringId] of Object.entries(fields)) {
      const text = stringAt(stringId, `mission task ${taskId} ${field}`);
      if (text.trim() === "") continue;
      const key = `${MISSION_PREFIX}${taskId}.${field}`;
      if (entries.has(key)) throw new Error(`duplicate mission localization key ${key}`);
      entries.set(key, text);
      aliases += 1;
    }
  }
  return aliases;
}

function addIndexedNameAliases(entries, value) {
  const aliases = { npc: 0, questItem: 0 };
  const addAliases = (rows, strings, idField, stringField, prefix, kind) => {
    for (const [rowIndex, row] of rows.entries()) {
      const id = row[idField];
      if (!Number.isInteger(id) || id <= 0) continue;
      const stringId = row[stringField];
      const stringRow = strings[stringId];
      const text = stringRow?.m_strName;
      if (typeof text !== "string" || text.trim() === "") {
        throw new Error(`${kind} row ${rowIndex} references missing name string ${stringId}`);
      }
      const key = `${prefix}${id}.name`;
      const previous = entries.get(key);
      if (previous !== undefined && previous !== text) {
        throw new Error(`contradictory ${kind} localization key ${key}: ${previous} vs ${text}`);
      }
      if (previous === undefined) {
        entries.set(key, text);
        aliases[kind] += 1;
      }
    }
  };

  const npcTable = value.m_pNpcTable;
  addAliases(
    npcTable.m_pNpcData,
    npcTable.m_pNpcStringData,
    "m_iNpcNumber",
    "m_iNpcName",
    NPC_NAME_PREFIX,
    "npc",
  );
  const questItemTable = value.m_pQuestItemTable;
  addAliases(
    questItemTable.m_pItemData,
    questItemTable.m_pItemStringData,
    "m_iItemNumber",
    "m_iItemName",
    QUEST_ITEM_NAME_PREFIX,
    "questItem",
  );
  return aliases;
}

function semanticSlug(value) {
  return value
    .normalize("NFKD")
    .replace(/[^A-Za-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .toLowerCase();
}

function addWorldNames(entries, object) {
  if (object.name !== "worldname" || object.pathId !== 8 || !Array.isArray(object.value?.m_pWorldNameData)) {
    throw new Error("primary worldname object must be TableData pathId 8");
  }
  const unique = new Map();
  for (const [index, row] of object.value.m_pWorldNameData.entries()) {
    for (const field of ["DongName", "ZoneName"]) {
      const text = row[field];
      if (typeof text !== "string" || text.trim() === "" || text === "null" || text === "unknown") continue;
      entries.set(`${PREFIX}world_name.world_name.${index}.${token(field)}`, text);
      unique.set(text, `${WORLD_LOCATION_PREFIX}${semanticSlug(text)}`);
    }
  }
  for (const [text, key] of unique) entries.set(key, text);
  return { rows: object.value.m_pWorldNameData.length, semanticAliases: unique.size };
}

function stripImported(entries) {
  return Object.fromEntries(
    Object.entries(entries).filter(
      ([key]) =>
        !key.startsWith(PREFIX) &&
        !key.startsWith(MISSION_PREFIX) &&
        !key.startsWith(NPC_NAME_PREFIX) &&
        !key.startsWith(QUEST_ITEM_NAME_PREFIX) &&
        !key.startsWith(WORLD_LOCATION_PREFIX),
    ),
  );
}

function jsonBundle(locale, entries) {
  return `${JSON.stringify({ schema: "ffone.text-bundle.v1", locale, entries }, null, 2)}\n`;
}

function protectTokens(text) {
  const tokens = [];
  const protectedText = text.replace(/\{[^{}]+\}|%[-+0-9.#]*[a-zA-Z]|<[^>]+>/g, (match) => {
    const marker = `ZXQPH${tokens.length}QXZ`;
    tokens.push(match);
    return marker;
  });
  return { protectedText, tokens };
}

function restoreTokens(text, tokens) {
  let restored = text;
  tokens.forEach((tokenValue, index) => {
    restored = restored.replaceAll(`ZXQPH${index}QXZ`, tokenValue);
  });
  return restored;
}

function makeBatches(texts) {
  const batches = [];
  let current = [];
  let chars = 0;
  for (const text of texts) {
    const extra = text.length + 32;
    if (current.length && chars + extra > MAX_BATCH_CHARS) {
      batches.push(current);
      current = [];
      chars = 0;
    }
    current.push(text);
    chars += extra;
  }
  if (current.length) batches.push(current);
  return batches;
}

async function translateBatch(batch, attempt = 0) {
  const protectedBatch = batch.map(protectTokens);
  const joined = protectedBatch
    .map(({ protectedText }, index) => `${index ? `\n<<<FFONE_${String(index).padStart(4, "0")}>>>\n` : ""}${protectedText}`)
    .join("");
  const url = new URL("https://translate.googleapis.com/translate_a/single");
  url.search = new URLSearchParams({ client: "gtx", sl: "en", tl: "ru", dt: "t", q: joined });
  try {
    const response = await fetch(url, { signal: AbortSignal.timeout(30_000) });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const payload = await response.json();
    const translated = payload[0].map((part) => part[0]).join("");
    const pieces = translated.split(/\n?<<<FFONE_\d{4}>>>\n?/);
    if (pieces.length !== batch.length) {
      throw new Error(`marker count mismatch: expected ${batch.length}, found ${pieces.length}`);
    }
    return pieces.map((piece, index) => restoreTokens(piece, protectedBatch[index].tokens).trim());
  } catch (error) {
    if (attempt >= 5) throw error;
    await new Promise((done) => setTimeout(done, 500 * 2 ** attempt));
    return translateBatch(batch, attempt + 1);
  }
}

async function translateMissing(texts) {
  const batches = makeBatches(texts);
  const result = new Map();
  let cursor = 0;
  async function worker() {
    while (true) {
      const index = cursor++;
      if (index >= batches.length) return;
      const translated = await translateBatch(batches[index]);
      batches[index].forEach((source, item) => result.set(source, translated[item]));
      if ((index + 1) % 25 === 0 || index + 1 === batches.length) {
        process.stderr.write(`translated ${index + 1}/${batches.length} batches\n`);
      }
    }
  }
  await Promise.all(Array.from({ length: Math.min(CONCURRENCY, batches.length) }, worker));
  return result;
}

const [tableBytes, worldNameBytes] = await Promise.all([
  readFile(TABLE_SET),
  readFile(WORLD_NAME_OBJECT).catch((error) => {
    throw new Error(
      `${WORLD_NAME_OBJECT} is required; recover primary TableData pathId 8 with fusionforge dump-object (${error.message})`,
    );
  }),
]);
const tableSet = JSON.parse(tableBytes);
const worldNameObject = JSON.parse(worldNameBytes);
if (tableSet.schema !== "ffone.table-set.v1") throw new Error(`unsupported ${tableSet.schema}`);
const tables = tableSet.tables.filter((table) => table.name === "npc_imports_consolidated");
if (tables.length !== 1) throw new Error(`expected one npc_imports_consolidated table, found ${tables.length}`);

const [enBytes, ruBytes] = await Promise.all([readFile(RUNTIME_EN), readFile(RUNTIME_RU)]);
const en = JSON.parse(enBytes);
const ru = JSON.parse(ruBytes);
const imported = importedEntries(tables[0].value);
const worldNames = addWorldNames(imported, worldNameObject);
const directTableDataEntries = imported.size - worldNames.semanticAliases;
const missionSemanticAliases = addMissionAliases(imported, tables[0].value);
const indexedNameSemanticAliases = addIndexedNameAliases(imported, tables[0].value);
const manualEn = stripImported(en.entries);
const manualRu = stripImported(ru.entries);

const knownTranslation = new Map();
for (const [key, source] of Object.entries(manualEn)) {
  const translated = manualRu[key];
  if (typeof translated === "string" && translated !== source) knownTranslation.set(source, translated);
}
for (const [key, source] of imported) {
  const translated = ru.entries[key];
  if (typeof translated === "string") knownTranslation.set(source, translated);
}

const uniqueSources = [...new Set(imported.values())];
const missing = uniqueSources.filter((source) => !knownTranslation.has(source));
if (missing.length && !TRANSLATE) {
  process.stderr.write(
    `${missing.length} unique TableData strings need Russian copy; rerun with --translate-missing\n`,
  );
}
if (missing.length && TRANSLATE) {
  const translations = await translateMissing(missing);
  for (const [source, translated] of translations) knownTranslation.set(source, translated);
}

const importedRu = new Map();
for (const [key, source] of imported) importedRu.set(key, knownTranslation.get(source) ?? source);
const mergedEn = { ...manualEn, ...Object.fromEntries(imported) };
const mergedRu = { ...manualRu, ...Object.fromEntries(importedRu) };
const untranslated = [...imported].filter(([, source]) => !knownTranslation.has(source)).length;
const report = {
  schema: "ffone.tabledata-localization-import.v1",
  source: {
    alias: "patched-published-native",
    relativePath: "data/tables/table-set.json",
    tableName: tables[0].name,
    tableKey: tables[0].key,
    bytes: tableBytes.length,
    sha256: sha256(tableBytes),
    primaryAuthority: {
      alias: "primary",
      relativePath: "TableData.resourceFile",
      bytes: 784963,
      sha256: "6d4cea151152e2ab75b7d16590bda00fba172600a163e0b3a5318564df0d7e3b",
    },
    intentionalDivergence: "The published table set retains patched new-NPC rows in addition to primary TableData content.",
    worldNameObject: {
      alias: "primary",
      relativePath: "TableData.resourceFile/CustomAssetBundle-1dca92eecee4742d985b799d8226666d/pathId-8-worldname",
      recoveredPath: relative(EDITOR_ROOT, WORLD_NAME_OBJECT).replaceAll("\\", "/"),
      bytes: worldNameBytes.length,
      sha256: sha256(worldNameBytes),
    },
  },
  command: "node tools/legacy-sources/import-tabledata-localization.mjs --native-target <assets/game> --world-name-object <editor-work-object.json> --translate-missing",
  translator: "Google Translate web endpoint (en -> ru); existing native translations take precedence",
  importedEntries: imported.size,
  directTableDataEntries,
  missionSemanticAliases,
  npcNameSemanticAliases: indexedNameSemanticAliases.npc,
  questItemNameSemanticAliases: indexedNameSemanticAliases.questItem,
  worldNameRows: worldNames.rows,
  worldLocationSemanticAliases: worldNames.semanticAliases,
  uniqueSourceStrings: uniqueSources.length,
  untranslatedEntries: untranslated,
  excluded: "numeric references, model/texture/audio/animation paths, chat filters, and avatar name fragments",
};

process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
if (!DRY_RUN) {
  if (untranslated) throw new Error(`${untranslated} imported entries still equal English; translate before publishing`);
  const enOutput = jsonBundle("en", mergedEn);
  const ruOutput = jsonBundle("ru", mergedRu);
  await mkdir(dirname(REPORT), { recursive: true });
  await Promise.all([
    writeFile(RUNTIME_EN, enOutput),
    writeFile(RUNTIME_RU, ruOutput),
    writeFile(REPORT, `${JSON.stringify(report, null, 2)}\n`),
  ]);
}
