#!/usr/bin/env node

import { createHash } from "node:crypto";
import { lstat, mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const TABLE_SET_SCHEMA = "ffone.table-set.v1";

function fail(message) {
  throw new Error(message);
}

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined || value.startsWith("--")) {
      fail("usage: extract-table-set-table --input <TABLE_SET> --table <NAME> --output <JSON>");
    }
    if (values.has(key)) fail(`duplicate argument ${key}`);
    values.set(key, value);
  }
  for (const key of ["--input", "--table", "--output"]) {
    if (!values.has(key)) fail(`missing required argument ${key}`);
  }
  return values;
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

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const inputPath = path.resolve(args.get("--input"));
  const outputPath = path.resolve(args.get("--output"));
  if (inputPath === outputPath) fail("input and output paths must be distinct");

  const inputBytes = await readFile(inputPath);
  let tableSet;
  try {
    tableSet = JSON.parse(inputBytes.toString("utf8"));
  } catch (error) {
    fail(`input is not valid JSON: ${error.message}`);
  }
  if (tableSet?.schema !== TABLE_SET_SCHEMA || !Array.isArray(tableSet.tables)) {
    fail(`input must use ${TABLE_SET_SCHEMA}`);
  }
  const tableName = args.get("--table");
  const matches = tableSet.tables.filter((table) => table?.name === tableName);
  if (
    matches.length !== 1 ||
    typeof matches[0].value !== "object" ||
    matches[0].value === null ||
    Array.isArray(matches[0].value)
  ) {
    fail(`table-set must contain exactly one object table named ${JSON.stringify(tableName)}`);
  }

  const outputBytes = Buffer.from(`${JSON.stringify(matches[0].value, null, 2)}\n`, "utf8");
  await mustNotExist(outputPath, "output");
  await mkdir(path.dirname(outputPath), { recursive: true });
  const next = `${outputPath}.next-${process.pid}`;
  await mustNotExist(next, "temporary output");
  await writeFile(next, outputBytes, { flag: "wx" });
  await rename(next, outputPath);
  process.stdout.write(
    `extracted ${tableName}: bytes=${outputBytes.length}, sha256=${sha256(outputBytes)}, ` +
      `inputSha256=${sha256(inputBytes)}\n`,
  );
}

main().catch((error) => {
  process.stderr.write(`extract-table-set-table: ${error.message}\n`);
  process.exitCode = 1;
});
