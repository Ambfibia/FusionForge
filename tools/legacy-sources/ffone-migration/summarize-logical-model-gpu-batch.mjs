import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";

const RUN_SCHEMA = "ffone.logical-model-gpu-batch-run.v1";
const AUDIT_SCHEMA = "ffone.logical-model-gpu-evidence-audit.v1";
const OUTPUT_SCHEMA = "ffone.logical-model-gpu-blockers.v1";

function fail(message) {
  console.error(message);
  process.exit(2);
}

function parseArguments(values) {
  const options = new Map();
  for (let index = 0; index < values.length; index += 2) {
    const name = values[index];
    const value = values[index + 1];
    if (!name?.startsWith("--") || !value || value.startsWith("--")) {
      fail(
        "usage: node tools/legacy-sources/ffone-migration/summarize-logical-model-gpu-batch.mjs " +
          "--run <RUN.json> --audit <AUDIT.json> --output <FRESH.json>",
      );
    }
    if (options.has(name)) {
      fail(`${name} may be supplied only once`);
    }
    options.set(name, value);
  }
  for (const name of options.keys()) {
    if (!["--run", "--audit", "--output"].includes(name)) {
      fail(`unknown option: ${name}`);
    }
  }
  return options;
}

function required(options, name) {
  const value = options.get(name);
  if (!value) {
    fail(`${name} is required`);
  }
  return value;
}

function canonicalFile(value, name) {
  let resolved;
  try {
    resolved = fs.realpathSync(path.resolve(value));
  } catch (error) {
    fail(`${name} is not readable: ${error.message}`);
  }
  if (!fs.statSync(resolved).isFile()) {
    fail(`${name} must be a file: ${resolved}`);
  }
  return resolved;
}

function freshOutput(value) {
  const output = path.resolve(value);
  if (fs.existsSync(output)) {
    fail(`output is never overwritten: ${output}`);
  }
  fs.mkdirSync(path.dirname(output), { recursive: true });
  return output;
}

function readJson(file, name) {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  } catch (error) {
    fail(`cannot read ${name} ${file}: ${error.message}`);
  }
}

function sha256(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function slash(value) {
  return value.replaceAll("\\", "/");
}

function sortedUnique(values) {
  return [...new Set(values)].sort((left, right) =>
    left.localeCompare(right, "en"),
  );
}

function classifyFailure(model) {
  const detail = model.runtimeError ?? model.processError ?? "";

  if (detail.includes("unsupported exact legacy shader name:")) {
    const shaderNames = sortedUnique(
      [...detail.matchAll(/unsupported exact legacy shader name: ([^|]+)/g)].map(
        (match) => match[1].trim(),
      ),
    );
    if (shaderNames.length === 0) {
      fail(`cannot extract exact shader name for ${model.relativeGlb}`);
    }
    return {
      code: "runtimeLegacyShaderUnsupported",
      detail,
      exactShaderNames: shaderNames,
      requiredResolution:
        "Implement only an exact runtime shader contract backed by the audited ShaderLab program/hash/pass state; do not infer behavior from a fuzzy shader name.",
    };
  }

  if (detail.includes("lacks required exact texture bindings for")) {
    const match = detail.match(
      /material (.+?) lacks required exact texture bindings for ([^|]+)/,
    );
    if (!match) {
      fail(`cannot extract exact texture-binding failure for ${model.relativeGlb}`);
    }
    return {
      code: "runtimeExactTextureBindingMissing",
      detail,
      material: match[1].trim(),
      exactShaderName: match[2].trim(),
      requiredResolution:
        "Recover the authoritative required texture-slot binding from exact serialized material/source evidence and republish; do not substitute a blank or guessed texture.",
    };
  }

  if (detail.includes("contradicts the canonical pass:")) {
    const match = detail.match(/typed pass (\d+) for (.+?) contradicts/);
    if (!match) {
      fail(`cannot extract typed-pass contradiction for ${model.relativeGlb}`);
    }
    return {
      code: "runtimeTypedPassContradiction",
      detail,
      passIndex: Number(match[1]),
      exactShaderName: match[2],
      requiredResolution:
        "Resolve the exact shader-program/pass identity and republish its typed pass; do not choose either render state heuristically.",
    };
  }

  fail(
    `unclassified GPU failure for ${model.relativeGlb}: ` +
      (detail || "<no runtime/process error>"),
  );
}

const options = parseArguments(process.argv.slice(2));
const runPath = canonicalFile(required(options, "--run"), "--run");
const auditPath = canonicalFile(required(options, "--audit"), "--audit");
const outputPath = freshOutput(required(options, "--output"));
const run = readJson(runPath, "GPU batch run");
const audit = readJson(auditPath, "GPU evidence audit");

if (run.schema !== RUN_SCHEMA) {
  fail(`unexpected run schema: ${JSON.stringify(run.schema)}`);
}
if (audit.schema !== AUDIT_SCHEMA) {
  fail(`unexpected audit schema: ${JSON.stringify(audit.schema)}`);
}
if (!Array.isArray(run.models) || !Array.isArray(audit.models)) {
  fail("run and audit must contain model arrays");
}
if (run.counts?.planned !== run.models.length) {
  fail("run planned count does not match its model array");
}
if (audit.counts?.candidateGlbs !== audit.models.length) {
  fail("audit candidate count does not match its model array");
}
if (run.models.length !== audit.models.length) {
  fail("run and audit candidate model counts differ");
}

const failedModels = run.models.filter((model) => model.status === "failed");
const acceptedStatuses = new Set(["passed", "resumed-passed", "failed"]);
const unknownStatuses = run.models.filter(
  (model) => !acceptedStatuses.has(model.status),
);
if (unknownStatuses.length !== 0) {
  fail(
    `run contains non-terminal model statuses: ${unknownStatuses
      .map((model) => `${model.relativeGlb}=${model.status}`)
      .join(", ")}`,
  );
}
if (
  run.counts.failed !== failedModels.length ||
  run.counts.passed !== run.models.length - failedModels.length
) {
  fail("run terminal counts do not match its model statuses");
}

const failedPaths = failedModels
  .map((model) => model.relativeGlb)
  .sort((left, right) => left.localeCompare(right, "en"));
const auditMissingPaths = audit.models
  .filter((model) => !model.passed)
  .map((model) => model.relativeGlb)
  .sort((left, right) => left.localeCompare(right, "en"));
if (JSON.stringify(failedPaths) !== JSON.stringify(auditMissingPaths)) {
  fail("run failures and independently audited missing evidence differ");
}

const blockers = failedModels
  .map((model) => ({
    relativeGlb: model.relativeGlb,
    trueName: model.trueName,
    selectedAnimation: model.selectedAnimation,
    ...classifyFailure(model),
  }))
  .sort((left, right) => left.relativeGlb.localeCompare(right.relativeGlb, "en"));
const blockerClasses = Object.fromEntries(
  sortedUnique(blockers.map((blocker) => blocker.code)).map((code) => [
    code,
    blockers.filter((blocker) => blocker.code === code).length,
  ]),
);

const report = {
  schema: OUTPUT_SCHEMA,
  inputs: {
    runReport: slash(runPath),
    runReportSha256: sha256(runPath),
    independentEvidenceAudit: slash(auditPath),
    independentEvidenceAuditSha256: sha256(auditPath),
    candidateRoot: run.candidateRoot,
    evidenceRoot: run.evidenceRoot,
  },
  status: blockers.length === 0 ? "gpu-complete" : "blocked",
  counts: {
    candidates: run.models.length,
    gpuPassed: run.models.length - blockers.length,
    blocked: blockers.length,
    blockerClasses,
  },
  gates: {
    structuralPassed: audit.structuralPassed === true,
    automatedGpuPassed: audit.automatedGpuPassed === true,
    visualParityPending: audit.visualParityPending === true,
    runtimeSpawnPolicyPending: true,
    publishable: audit.publishable === true,
    productionAssetsMutated: run.productionAssetsMutated === true,
  },
  policy: {
    allCandidateModelsRequireGpuEvidence: true,
    lossyOrFuzzyFallbackAllowed: false,
    partialProductionInstallAllowed: false,
  },
  blockers,
};

fs.writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`, {
  flag: "wx",
});
console.log(
  `GPU blockers: ${report.counts.gpuPassed}/${report.counts.candidates} passed, ` +
    `${report.counts.blocked} blocked; report=${outputPath}`,
);
