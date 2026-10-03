import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const PLAN_SCHEMA = "ffone.logical-model-gpu-batch-plan.v1";
const RUN_SCHEMA = "ffone.logical-model-gpu-batch-run.v1";
const BATCH_REPORT_SCHEMA = "ffone.logical-model-batch-publish-report.v4";

function fail(message) {
  console.error(message);
  process.exit(2);
}

function usage() {
  console.error(`usage:
  node tools/legacy-sources/ffone-migration/run-logical-model-gpu-batch.mjs plan \\
    --candidate <DIR> --preview <EXE> --output <FRESH.json> \\
    [--frames <COUNT>] [--timeout <SECONDS>]

  node tools/legacy-sources/ffone-migration/run-logical-model-gpu-batch.mjs run \\
    --plan <PLAN.json> --candidate <DIR> --evidence <DIR> \\
    --preview <EXE> --output <REPORT.json> [--resume] \\
    [--frames <COUNT>] [--timeout <SECONDS>]`);
}

function parseArguments(values) {
  const options = new Map();
  const flags = new Set();
  for (let index = 0; index < values.length; index += 1) {
    const name = values[index];
    if (name === "--resume") {
      if (flags.has(name)) {
        fail(`${name} may be supplied only once`);
      }
      flags.add(name);
      continue;
    }
    if (!name.startsWith("--")) {
      fail(`unexpected positional argument: ${name}`);
    }
    if (options.has(name)) {
      fail(`${name} may be supplied only once`);
    }
    const value = values[index + 1];
    if (!value || value.startsWith("--")) {
      fail(`${name} requires a value`);
    }
    options.set(name, value);
    index += 1;
  }
  return { options, flags };
}

function required(options, name) {
  const value = options.get(name);
  if (!value) {
    fail(`${name} is required`);
  }
  return value;
}

function positiveInteger(value, name) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    fail(`${name} must be a positive integer`);
  }
  return parsed;
}

function canonicalFile(value, name) {
  const resolved = fs.realpathSync(path.resolve(value));
  if (!fs.statSync(resolved).isFile()) {
    fail(`${name} must be a file: ${resolved}`);
  }
  return resolved;
}

function canonicalDirectory(value, name) {
  const resolved = fs.realpathSync(path.resolve(value));
  if (!fs.statSync(resolved).isDirectory()) {
    fail(`${name} must be a directory: ${resolved}`);
  }
  return resolved;
}

function slash(value) {
  return value.replaceAll("\\", "/");
}

function safeRelative(value, name) {
  if (
    !value ||
    path.isAbsolute(value) ||
    value.split(/[\\/]/).some((component) => component === "..")
  ) {
    fail(`${name} must be a safe relative path: ${JSON.stringify(value)}`);
  }
  return slash(value);
}

function freshOutput(value) {
  const output = path.resolve(value);
  if (fs.existsSync(output)) {
    fail(`output is never overwritten: ${output}`);
  }
  fs.mkdirSync(path.dirname(output), { recursive: true });
  return output;
}

function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function readJson(value, name) {
  let parsed;
  try {
    parsed = JSON.parse(fs.readFileSync(value, "utf8"));
  } catch (error) {
    fail(`cannot read ${name} ${value}: ${error.message}`);
  }
  return parsed;
}

function readGlbJson(glbPath) {
  const handle = fs.openSync(glbPath, "r");
  try {
    const header = Buffer.alloc(20);
    if (fs.readSync(handle, header, 0, header.length, 0) !== header.length) {
      fail(`short GLB header: ${glbPath}`);
    }
    if (header.toString("ascii", 0, 4) !== "glTF") {
      fail(`invalid GLB magic: ${glbPath}`);
    }
    if (header.readUInt32LE(4) !== 2) {
      fail(`unsupported GLB version: ${glbPath}`);
    }
    const declaredLength = header.readUInt32LE(8);
    const actualLength = fs.fstatSync(handle).size;
    if (declaredLength !== actualLength) {
      fail(`GLB declared/actual length differs: ${glbPath}`);
    }
    const jsonLength = header.readUInt32LE(12);
    if (header.toString("ascii", 16, 20) !== "JSON") {
      fail(`first GLB chunk is not JSON: ${glbPath}`);
    }
    const json = Buffer.alloc(jsonLength);
    if (fs.readSync(handle, json, 0, jsonLength, 20) !== jsonLength) {
      fail(`short GLB JSON chunk: ${glbPath}`);
    }
    return JSON.parse(json.toString("utf8").replace(/[\u0000 ]+$/, ""));
  } finally {
    fs.closeSync(handle);
  }
}

function evidencePaths(relativeGlb) {
  if (!relativeGlb.toLowerCase().endsWith(".glb")) {
    fail(`candidate model is not GLB: ${relativeGlb}`);
  }
  const stem = relativeGlb.slice(0, -4);
  return {
    json: `${stem}.gpu.json`,
    png: `${stem}.gpu.png`,
  };
}

function previewArguments(entry, candidateRoot, evidenceRoot, frames, timeout) {
  const args = [
    "--asset-root",
    candidateRoot,
    "--model",
    entry.relativeGlb,
  ];
  if (entry.selectedAnimation !== null) {
    args.push("--animation-name", entry.selectedAnimation);
  }
  args.push(
    "--evidence-root",
    evidenceRoot,
    "--outline",
    "source",
    "--frames",
    String(frames),
    "--timeout",
    String(timeout),
  );
  return args;
}

function writeCheckpoint(output, report) {
  const bytes = `${JSON.stringify(report, null, 2)}\n`;
  const temporary = `${output}.tmp.${process.pid}`;
  fs.writeFileSync(temporary, bytes, { encoding: "utf8", flag: "w" });
  try {
    fs.renameSync(temporary, output);
  } catch (error) {
    if (!["EEXIST", "EPERM"].includes(error.code) || !fs.existsSync(output)) {
      throw error;
    }
    fs.rmSync(output);
    fs.renameSync(temporary, output);
  }
}

function parseLastJson(stdout) {
  const lines = stdout
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  for (let index = lines.length - 1; index >= 0; index -= 1) {
    if (!lines[index].startsWith("{")) {
      continue;
    }
    try {
      return JSON.parse(lines[index]);
    } catch {
      // Keep looking for the final standalone JSON record.
    }
  }
  return null;
}

function buildPlan(parsed) {
  const allowed = new Set([
    "--candidate",
    "--preview",
    "--output",
    "--frames",
    "--timeout",
  ]);
  for (const name of parsed.options.keys()) {
    if (!allowed.has(name)) {
      fail(`unknown plan option: ${name}`);
    }
  }
  if (parsed.flags.size !== 0) {
    fail("plan does not accept flags");
  }

  const candidateRoot = canonicalDirectory(
    required(parsed.options, "--candidate"),
    "--candidate",
  );
  const previewExecutable = canonicalFile(
    required(parsed.options, "--preview"),
    "--preview",
  );
  const output = freshOutput(required(parsed.options, "--output"));
  const frames = positiveInteger(parsed.options.get("--frames") ?? "360", "--frames");
  const timeout = positiveInteger(
    parsed.options.get("--timeout") ?? "20",
    "--timeout",
  );
  const batchReportPath = path.join(
    candidateRoot,
    "logical-model-batch-report.json",
  );
  const batchReport = readJson(batchReportPath, "candidate batch report");
  if (batchReport.schema !== BATCH_REPORT_SCHEMA) {
    fail(`unsupported candidate report schema: ${batchReport.schema}`);
  }
  if (!Array.isArray(batchReport.models) || batchReport.models.length === 0) {
    fail("candidate batch report has no models");
  }

  const entries = [];
  const seenModels = new Set();
  for (const model of batchReport.models) {
    const relativeGlb = safeRelative(model.outputGlb, "model.outputGlb");
    if (seenModels.has(relativeGlb.toLowerCase())) {
      fail(`duplicate case-folded candidate model: ${relativeGlb}`);
    }
    seenModels.add(relativeGlb.toLowerCase());
    const glbPath = path.join(candidateRoot, relativeGlb);
    const document = readGlbJson(glbPath);
    const logicalName = model.logicalName;
    const scene = document.scenes?.[document.scene ?? 0];
    if (!scene || scene.name !== logicalName || scene.nodes?.length !== 1) {
      fail(`GLB scene/root contract differs for ${relativeGlb}`);
    }
    const root = document.nodes?.[scene.nodes[0]];
    if (!root || root.name !== logicalName) {
      fail(`GLB exact root name differs for ${relativeGlb}`);
    }
    const animationNames = (document.animations ?? []).map((animation) => {
      if (typeof animation.name !== "string" || animation.name.length === 0) {
        fail(`GLB has unnamed standard animation: ${relativeGlb}`);
      }
      return animation.name;
    });
    if (new Set(animationNames).size !== animationNames.length) {
      fail(`GLB has duplicate exact standard animation names: ${relativeGlb}`);
    }
    const selectedAnimation = animationNames.includes("stand1")
      ? "stand1"
      : (animationNames[0] ?? null);
    const evidence = evidencePaths(relativeGlb);
    const entry = {
      relativeGlb,
      trueName: logicalName,
      standardAnimationNames: animationNames,
      selectedAnimation,
      selectionRule:
        selectedAnimation === null
          ? "static-no-standard-animation"
          : selectedAnimation === "stand1"
            ? "prefer-exact-stand1"
            : "first-exact-standard-animation-in-glb-order",
      evidenceJson: evidence.json,
      evidencePng: evidence.png,
      standaloneGpuGate: true,
      characterSpawnPolicyAsserted: false,
    };
    entry.previewArguments = previewArguments(
      entry,
      candidateRoot,
      "<EVIDENCE_ROOT>",
      frames,
      timeout,
    );
    entries.push(entry);
  }

  entries.sort((left, right) =>
    left.relativeGlb.localeCompare(right.relativeGlb),
  );
  const animated = entries.filter(
    (entry) => entry.selectedAnimation !== null,
  );
  const plan = {
    schema: PLAN_SCHEMA,
    candidateRoot: slash(candidateRoot),
    candidateReport: slash(batchReportPath),
    previewExecutable: slash(previewExecutable),
    limits: {
      frames,
      timeoutSeconds: timeout,
      outerTimeoutSeconds: timeout + 15,
      executionOrder: "candidate-relative-glb-ordinal",
      concurrency: 1,
    },
    scope: {
      gpuLoadMaterialSkinAnimationRender: true,
      fixedAnimationSampleNormalizedPpm: 500_000,
      characterSpawnPolicyAsserted: false,
      visualParityAsserted: false,
      productionInstallAuthorized: false,
    },
    counts: {
      models: entries.length,
      animatedModels: animated.length,
      staticModels: entries.length - animated.length,
      standardAnimationClips: entries.reduce(
        (sum, entry) => sum + entry.standardAnimationNames.length,
        0,
      ),
      selectedStand1: animated.filter(
        (entry) => entry.selectedAnimation === "stand1",
      ).length,
      selectedOtherExactAnimation: animated.filter(
        (entry) => entry.selectedAnimation !== "stand1",
      ).length,
    },
    models: entries,
  };
  const bytes = `${JSON.stringify(plan, null, 2)}\n`;
  fs.writeFileSync(output, bytes, { encoding: "utf8", flag: "wx" });
  console.log(
    `planned ${plan.counts.models} models (${plan.counts.animatedModels} animated, ${plan.counts.staticModels} static, ${plan.counts.standardAnimationClips} exact clips) into ${output}`,
  );
}

function runPlan(parsed) {
  const allowed = new Set([
    "--plan",
    "--candidate",
    "--evidence",
    "--preview",
    "--output",
    "--frames",
    "--timeout",
  ]);
  for (const name of parsed.options.keys()) {
    if (!allowed.has(name)) {
      fail(`unknown run option: ${name}`);
    }
  }
  for (const name of parsed.flags) {
    if (name !== "--resume") {
      fail(`unknown run flag: ${name}`);
    }
  }
  const resume = parsed.flags.has("--resume");
  const planPath = canonicalFile(required(parsed.options, "--plan"), "--plan");
  const planBytes = fs.readFileSync(planPath);
  const plan = JSON.parse(planBytes.toString("utf8"));
  if (plan.schema !== PLAN_SCHEMA || !Array.isArray(plan.models)) {
    fail(`unsupported GPU batch plan: ${planPath}`);
  }
  const candidateRoot = canonicalDirectory(
    required(parsed.options, "--candidate"),
    "--candidate",
  );
  if (slash(candidateRoot) !== plan.candidateRoot) {
    fail("live --candidate differs from the dry-run plan");
  }
  const previewExecutable = canonicalFile(
    required(parsed.options, "--preview"),
    "--preview",
  );
  if (slash(previewExecutable) !== plan.previewExecutable) {
    fail("live --preview differs from the dry-run plan");
  }
  const frames = positiveInteger(
    parsed.options.get("--frames") ?? String(plan.limits.frames),
    "--frames",
  );
  const timeout = positiveInteger(
    parsed.options.get("--timeout") ?? String(plan.limits.timeoutSeconds),
    "--timeout",
  );
  const effectiveLimits = {
    ...plan.limits,
    frames,
    timeoutSeconds: timeout,
    outerTimeoutSeconds: timeout + 15,
  };
  const evidenceArgument = path.resolve(required(parsed.options, "--evidence"));
  fs.mkdirSync(evidenceArgument, { recursive: true });
  const evidenceRoot = canonicalDirectory(evidenceArgument, "--evidence");
  if (
    evidenceRoot.startsWith(`${candidateRoot}${path.sep}`) ||
    candidateRoot.startsWith(`${evidenceRoot}${path.sep}`) ||
    evidenceRoot === candidateRoot
  ) {
    fail("candidate and evidence roots must be disjoint");
  }
  const outputArgument = path.resolve(required(parsed.options, "--output"));
  if (fs.existsSync(outputArgument) && !resume) {
    fail(`run output exists; pass --resume to continue: ${outputArgument}`);
  }
  fs.mkdirSync(path.dirname(outputArgument), { recursive: true });

  let prior = null;
  if (resume && fs.existsSync(outputArgument)) {
    prior = readJson(outputArgument, "prior run report");
    if (
      prior.schema !== RUN_SCHEMA ||
      prior.planSha256 !== sha256(planBytes) ||
      prior.candidateRoot !== slash(candidateRoot) ||
      prior.evidenceRoot !== slash(evidenceRoot)
    ) {
      fail("prior run report does not match this plan/candidate/evidence root");
    }
  }

  const report = {
    schema: RUN_SCHEMA,
    status: "running",
    plan: slash(planPath),
    planSha256: sha256(planBytes),
    candidateRoot: slash(candidateRoot),
    evidenceRoot: slash(evidenceRoot),
    previewExecutable: slash(previewExecutable),
    limits: effectiveLimits,
    productionAssetsMutated: false,
    counts: {
      planned: plan.models.length,
      executed: 0,
      resumed: 0,
      passed: 0,
      failed: 0,
    },
    models: [],
  };
  const priorByModel = new Map(
    (prior?.models ?? []).map((entry) => [entry.relativeGlb, entry]),
  );

  for (let index = 0; index < plan.models.length; index += 1) {
    const entry = plan.models[index];
    const evidenceJson = path.join(evidenceRoot, entry.evidenceJson);
    const evidencePng = path.join(evidenceRoot, entry.evidencePng);
    const jsonExists = fs.existsSync(evidenceJson);
    const pngExists = fs.existsSync(evidencePng);
    const priorEntry = priorByModel.get(entry.relativeGlb);
    if (
      resume &&
      jsonExists &&
      pngExists &&
      ["passed", "resumed-passed"].includes(priorEntry?.status)
    ) {
      report.counts.resumed += 1;
      report.counts.passed += 1;
      report.models.push({ ...priorEntry, status: "resumed-passed" });
      console.log(
        `[${index + 1}/${plan.models.length}] RESUME ${entry.relativeGlb}`,
      );
      writeCheckpoint(outputArgument, report);
      continue;
    }
    if (jsonExists || pngExists) {
      report.counts.failed += 1;
      report.models.push({
        relativeGlb: entry.relativeGlb,
        selectedAnimation: entry.selectedAnimation,
        status: "blocked-existing-partial-evidence",
        evidenceJson: entry.evidenceJson,
        evidencePng: entry.evidencePng,
      });
      console.log(
        `[${index + 1}/${plan.models.length}] BLOCK ${entry.relativeGlb}: evidence output already exists without a resumable passed record`,
      );
      writeCheckpoint(outputArgument, report);
      continue;
    }

    const args = previewArguments(
      entry,
      candidateRoot,
      evidenceRoot,
      effectiveLimits.frames,
      effectiveLimits.timeoutSeconds,
    );
    const started = Date.now();
    const result = spawnSync(previewExecutable, args, {
      cwd: process.cwd(),
      encoding: "utf8",
      maxBuffer: 8 * 1024 * 1024,
      timeout: effectiveLimits.outerTimeoutSeconds * 1000,
      windowsHide: false,
    });
    const elapsedMilliseconds = Date.now() - started;
    const runtime = parseLastJson(result.stdout ?? "");
    const passed =
      result.status === 0 &&
      runtime?.status === "success" &&
      fs.existsSync(evidenceJson) &&
      fs.existsSync(evidencePng);
    report.counts.executed += 1;
    report.counts[passed ? "passed" : "failed"] += 1;
    const modelReport = {
      relativeGlb: entry.relativeGlb,
      trueName: entry.trueName,
      selectedAnimation: entry.selectedAnimation,
      status: passed ? "passed" : "failed",
      exitCode: result.status,
      signal: result.signal,
      elapsedMilliseconds,
      evidenceJson: entry.evidenceJson,
      evidencePng: entry.evidencePng,
      runtimeStatus: runtime?.status ?? null,
      runtimeError: runtime?.error ?? null,
      frames: runtime?.frames ?? null,
      sceneReady: runtime?.sceneReady ?? null,
      animationsLoaded: runtime?.animations ?? null,
      animationPlayers: runtime?.animationPlayers ?? null,
      meshes: runtime?.meshes ?? null,
      skinnedMeshes: runtime?.skinnedMeshes ?? null,
      materialErrors: runtime?.materialErrors ?? null,
      shaderErrors: runtime?.shaderErrors ?? null,
      processError: result.error?.message ?? null,
      stderrTail: (result.stderr ?? "").slice(-4000),
      stdoutTail: (result.stdout ?? "").slice(-4000),
    };
    report.models.push(modelReport);
    console.log(
      `[${index + 1}/${plan.models.length}] ${passed ? "PASS" : "FAIL"} ${entry.relativeGlb}${entry.selectedAnimation ? ` animation=${JSON.stringify(entry.selectedAnimation)}` : " static"} ${elapsedMilliseconds}ms`,
    );
    writeCheckpoint(outputArgument, report);
  }

  report.status = report.counts.failed === 0 ? "complete-passed" : "complete-with-failures";
  writeCheckpoint(outputArgument, report);
  console.log(
    `GPU batch ${report.status}: ${report.counts.passed}/${report.counts.planned} passed (${report.counts.executed} executed, ${report.counts.resumed} resumed, ${report.counts.failed} failed); report=${outputArgument}`,
  );
  if (report.counts.failed !== 0) {
    process.exitCode = 1;
  }
}

const [command, ...rest] = process.argv.slice(2);
if (!command || !["plan", "run"].includes(command)) {
  usage();
  process.exit(2);
}
const parsed = parseArguments(rest);
if (command === "plan") {
  buildPlan(parsed);
} else {
  runPlan(parsed);
}
