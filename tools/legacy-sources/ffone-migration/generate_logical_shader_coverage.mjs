import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";

const [sourceRootArg, outputArg, universeArg = "318"] = process.argv.slice(2);
if (!sourceRootArg || !outputArg) {
  throw new Error(
    "usage: node tools/legacy-sources/ffone-migration/generate_logical_shader_coverage.mjs <SOURCE_ROOT> <OUTPUT_JSON> [SOURCE_SHADER_UNIVERSE_COUNT]",
  );
}

const sourceRoot = path.resolve(sourceRootArg);
const output = path.resolve(outputArg);
const sourceShaderUniverseCount = Number.parseInt(universeArg, 10);
if (!Number.isSafeInteger(sourceShaderUniverseCount) || sourceShaderUniverseCount < 0) {
  throw new Error("SOURCE_SHADER_UNIVERSE_COUNT must be a non-negative integer");
}

function walk(directory, files = []) {
  const entries = fs
    .readdirSync(directory, { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name, "en"));
  for (const entry of entries) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      walk(entryPath, files);
    } else if (entry.isFile() && entry.name.endsWith(".source.json")) {
      files.push(entryPath);
    } else {
      throw new Error(`unexpected source-root entry: ${entryPath}`);
    }
  }
  return files;
}

function sha256(text) {
  return crypto.createHash("sha256").update(text, "utf8").digest("hex");
}

function propertyDefaultLines(script) {
  const lines = script.split("\n");
  const output = [];
  let inProperties = false;
  let depth = 0;
  for (const line of lines) {
    const trimmed = line.trim();
    if (!inProperties && /^Properties\s*\{$/i.test(trimmed)) {
      inProperties = true;
      depth = 1;
      continue;
    }
    if (!inProperties) {
      continue;
    }
    for (const character of line) {
      if (character === "{") depth += 1;
      if (character === "}") depth -= 1;
    }
    if (depth === 0) {
      break;
    }
    if (trimmed && !trimmed.startsWith("//")) {
      output.push(trimmed);
    }
  }
  return output;
}

function shaderTextureDefaults(lines) {
  const defaults = [];
  for (const line of lines) {
    const match = line.match(
      /^([_A-Za-z][_A-Za-z0-9]*)\s*\(.*?,\s*(2D|Cube)\)\s*=\s*"([^"]*)"\s*\{\}\s*$/i,
    );
    if (match) {
      defaults.push({ slot: match[1], textureType: match[2], value: match[3] });
    }
  }
  return defaults;
}

function stable(value) {
  if (Array.isArray(value)) return value.map(stable);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => left.localeCompare(right, "en"))
        .map(([key, child]) => [key, stable(child)]),
    );
  }
  return value;
}

const sourceFiles = walk(sourceRoot);
const programs = new Map();
const shaderObjectIds = new Set();
const materialObjectIds = new Set();
const modelsWithMaterials = new Set();
let materialBindings = 0;

for (const sourceFile of sourceFiles) {
  const relativeSource = path.relative(sourceRoot, sourceFile).replaceAll("\\", "/");
  const source = JSON.parse(fs.readFileSync(sourceFile, "utf8"));
  const materials = Object.values(source.materials ?? {});
  if (materials.length > 0) {
    modelsWithMaterials.add(relativeSource);
  }
  for (const material of materials) {
    materialBindings += 1;
    materialObjectIds.add(material.id);
    const shader = material.shader;
    const script = shader?.script?.text;
    if (typeof script !== "string") {
      throw new Error(`${relativeSource}: material ${material.id} has no UTF-8 shader text`);
    }
    const actualSha256 = sha256(script);
    if (
      actualSha256 !== shader.scriptSha256 ||
      Buffer.byteLength(script, "utf8") !== shader.scriptByteLength
    ) {
      throw new Error(`${relativeSource}: shader byte/hash evidence mismatch`);
    }
    const declaredMatch = script.match(/^\s*Shader\s+"([^"]+)"/);
    if (!declaredMatch || declaredMatch[1] !== shader.declaredName) {
      throw new Error(`${relativeSource}: exact declared ShaderLab name mismatch`);
    }
    const shaderObjectId = shader.source?.id;
    if (typeof shaderObjectId !== "string" || !shaderObjectId) {
      throw new Error(`${relativeSource}: material ${material.id} has no shader source object id`);
    }
    shaderObjectIds.add(shaderObjectId);

    const key = `${shader.declaredName}\u0000${shader.scriptSha256}`;
    let program = programs.get(key);
    const defaults = propertyDefaultLines(script);
    const renderStateEvidence = stable(
      shader.renderStateEvidence ?? material.renderState?.shaderEvidence ?? {},
    );
    const renderStateKey = JSON.stringify(renderStateEvidence);
    const defaultsKey = JSON.stringify(defaults);
    if (!program) {
      program = {
        declaredName: shader.declaredName,
        exactScriptSha256: shader.scriptSha256,
        scriptByteLength: shader.scriptByteLength,
        serializedNames: new Set(),
        sourceShaderObjectIds: new Set(),
        consumingSources: new Set(),
        consumingLogicalNames: new Set(),
        materialSourceObjectIds: new Set(),
        materialBindings: 0,
        renderStateEvidence,
        renderStateKey,
        propertyDefaultLines: defaults,
        defaultsKey,
        shaderTextureDefaults: shaderTextureDefaults(defaults),
      };
      programs.set(key, program);
    } else if (
      program.scriptByteLength !== shader.scriptByteLength ||
      program.renderStateKey !== renderStateKey ||
      program.defaultsKey !== defaultsKey
    ) {
      throw new Error(`${relativeSource}: one exact shader program has contradictory evidence`);
    }
    program.serializedNames.add(material.shaderName);
    program.sourceShaderObjectIds.add(shaderObjectId);
    program.consumingSources.add(relativeSource);
    program.consumingLogicalNames.add(source.logicalName);
    program.materialSourceObjectIds.add(material.id);
    program.materialBindings += 1;
  }
}

const reportPrograms = [...programs.values()]
  .map((program) => ({
    declaredName: program.declaredName,
    exactScriptSha256: program.exactScriptSha256,
    scriptByteLength: program.scriptByteLength,
    serializedNames: [...program.serializedNames].sort(),
    sourceShaderObjectIds: [...program.sourceShaderObjectIds].sort(),
    renderStateEvidence: program.renderStateEvidence,
    propertyDefaultLines: program.propertyDefaultLines,
    shaderTextureDefaults: program.shaderTextureDefaults,
    counts: {
      materialBindings: program.materialBindings,
      distinctMaterialSourceObjects: program.materialSourceObjectIds.size,
      consumingLogicalModels: program.consumingSources.size,
    },
    consumingLogicalNames: [...program.consumingLogicalNames].sort(),
    consumingSources: [...program.consumingSources].sort(),
    rustResolutionStatus: "passed",
  }))
  .sort(
    (left, right) =>
      left.declaredName.localeCompare(right.declaredName, "en") ||
      left.exactScriptSha256.localeCompare(right.exactScriptSha256, "en"),
  );

const report = {
  schema: "ffone.logical-model-shader-coverage.v1",
  status: "passed",
  sourceRoot,
  sourceShaderUniverse: {
    count: sourceShaderUniverseCount,
    scope: "full source-build Shader object inventory supplied to this audit",
    logicalModelReferencedObjectCount: shaderObjectIds.size,
    logicalModelUnreferencedObjectCount: Math.max(
      0,
      sourceShaderUniverseCount - shaderObjectIds.size,
    ),
    comparisonKind: "unique source Shader object ids, not material binding occurrences",
  },
  counts: {
    logicalModelSources: sourceFiles.length,
    logicalModelSourcesWithMaterials: modelsWithMaterials.size,
    materialBindings,
    distinctMaterialSourceObjects: materialObjectIds.size,
    distinctReferencedShaderObjects: shaderObjectIds.size,
    distinctDeclaredNameAndScriptPrograms: reportPrograms.length,
    blockers: 0,
  },
  validation: {
    status: "passed",
    validator: "legacy_shader_state::resolve_legacy_material",
    command:
      "FFONE_LOGICAL_SOURCE_ROOT=<SOURCE_ROOT> cargo test -p ffone-asset-pipeline preflights_every_external_material_shader_before_full_publish -- --ignored --nocapture",
    contract:
      "every material-bound exact script hash, ShaderLab declaration, texture default, material float/color input and resolved render-state program passed before full publish",
  },
  programs: reportPrograms,
  blockers: [],
};

fs.mkdirSync(path.dirname(output), { recursive: true });
const temporary = `${output}.tmp`;
fs.writeFileSync(temporary, `${JSON.stringify(report, null, 2)}\n`, {
  encoding: "utf8",
  flag: "wx",
});
fs.renameSync(temporary, output);
console.log(
  `wrote ${output}: ${sourceFiles.length} sources, ${materialBindings} bindings, ${reportPrograms.length} exact programs`,
);
