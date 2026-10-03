import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// Historical identity-wrapper evidence only; not a validator for current model routes.
const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
if (process.argv.length !== 4) {
  throw new Error('Usage: node generate_character_runtime_gpu_audit.mjs <historical-evidence-dir> <new-report.json>');
}
const evidenceRoot = resolve(process.argv[2]);
const outputPath = resolve(process.argv[3]);
const identity = {
  translation: [0, 0, 0],
  rotationXyzw: [0, 0, 0, 1],
  scale: [1, 1, 1],
};
const halfTurn = {
  translation: [0, 0, 0],
  rotationXyzw: [0, 1, 0, -4.371138828673793e-8],
  scale: [1, 1, 1],
};
const cases = [
  {
    id: "npc_smallturo",
    kind: "npc",
    model: "models/npc/mob/npc_arturo/npc_smallturo.glb",
    authoredTranslation: [-0.740397036075592, 0, 0],
    authoredScale: 1,
    appliedScale: 1,
  },
  {
    id: "npc_billybillyvonbilly",
    kind: "npc",
    model: "models/npc/mob/npc_bigbilly/npc_billybillyvonbilly.glb",
    authoredTranslation: [
      0.7833316326141357, -1.793362021446228, -2.6419334411621094,
    ],
    authoredScale: 1,
    appliedScale: 1.2999999523162842,
  },
  {
    id: "npc_grubbygrouper",
    kind: "npc",
    model: "models/npc/mob/npc_grubber/npc_grubbygrouper.glb",
    authoredTranslation: [0.7605713605880737, 0, 0],
    authoredScale: 1,
    appliedScale: 1.399999976158142,
  },
  {
    id: "npc_mandroidglasses",
    kind: "npc",
    model:
      "models/npc/mob/npc_mandroid_disguise/npc_mandroidglasses.glb",
    authoredTranslation: [0, 0, 0],
    authoredScale: 1.2999999523162842,
    appliedScale: 1.5,
  },
  {
    id: "npc_penguin",
    kind: "npc",
    model: "models/npc/mob/npc_penguin/npc_penguin.glb",
    authoredTranslation: [-1.8869621753692627, 0, 0],
    authoredScale: 1,
    appliedScale: 0.800000011920929,
  },
  {
    id: "npc_tuddrussel",
    kind: "npc",
    model: "models/npc/mob/npc_tuddrussel/npc_tuddrussel.glb",
    authoredTranslation: [-1.3079497814178467, 0, 0],
    authoredScale: 1,
    appliedScale: 1.149999976158142,
  },
  {
    id: "nano_coco",
    kind: "nano",
    model: "models/nano/nano/nano_coco/nano_coco.glb",
    authoredTranslation: [0, 0, 0],
    authoredScale: 1.350000023841858,
    appliedScale: 1.350000023841858,
  },
  {
    id: "nano_holonano",
    kind: "nano",
    model: "models/nano/nano/nano_holonano/nano_holonano.glb",
    authoredTranslation: [0, 0, 0],
    authoredScale: 0.8500000238418579,
    appliedScale: 0.8500000238418579,
  },
  {
    id: "nano_johnnybravo",
    kind: "nano",
    model: "models/nano/nano/nano_johnnybravo/nano_johnnybravo.glb",
    authoredTranslation: [0, 0, 0],
    authoredScale: 1.2999999523162842,
    appliedScale: 1.2999999523162842,
  },
];

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function projectPath(path) {
  return relative(projectRoot, path).replaceAll("\\", "/");
}

function uniformTrs(translation, rotationXyzw, scale) {
  return {
    translation,
    rotationXyzw,
    scale: [scale, scale, scale],
  };
}

function closeNumber(left, right, epsilon = 1e-6) {
  return (
    Number.isFinite(left) &&
    Number.isFinite(right) &&
    Math.abs(left - right) <= epsilon
  );
}

function closeArray(left, right, epsilon = 1e-6) {
  return (
    Array.isArray(left) &&
    left.length === right.length &&
    left.every((value, index) => closeNumber(value, right[index], epsilon))
  );
}

function closeTrs(left, right, epsilon = 1e-6) {
  return (
    left != null &&
    closeArray(left.translation, right.translation, epsilon) &&
    closeArray(left.rotationXyzw, right.rotationXyzw, epsilon) &&
    closeArray(left.scale, right.scale, epsilon)
  );
}

function unitQuaternion(rotationXyzw, epsilon = 1e-4) {
  return (
    Array.isArray(rotationXyzw) &&
    rotationXyzw.length === 4 &&
    rotationXyzw.every(Number.isFinite) &&
    Math.abs(
      rotationXyzw.reduce((sum, component) => sum + component * component, 0) -
        1,
    ) <= epsilon
  );
}

function finiteNondegenerateBounds(bounds) {
  return (
    bounds != null &&
    closeArray(bounds.min, bounds.min, 0) &&
    closeArray(bounds.max, bounds.max, 0) &&
    bounds.min.every((value, index) => value < bounds.max[index])
  );
}

const auditedCases = cases.map((spec) => {
  const reportPath = join(evidenceRoot, `${spec.id}.json`);
  const pngPath = join(evidenceRoot, `${spec.id}.png`);
  const reportBytes = readFileSync(reportPath);
  const pngBytes = readFileSync(pngPath);
  const report = JSON.parse(reportBytes.toString("utf8"));
  const actual = report.characterRuntime;
  const expectedAuthored = uniformTrs(
    spec.authoredTranslation,
    [0, 0, 0, 1],
    spec.authoredScale,
  );
  const expectedApplied = uniformTrs(
    [0, 0, 0],
    [0, 0, 0, 1],
    spec.appliedScale,
  );
  const expectedRootWorld = uniformTrs(
    [0, 0, 0],
    halfTurn.rotationXyzw,
    spec.appliedScale,
  );
  const checks = {
    reportSuccess: report.status === "success" && report.error == null,
    sceneReady: report.sceneReady === true,
    exactModelPath: report.model === spec.model,
    exactRootName: actual?.exactRootName === spec.id,
    exactCharacterKind: actual?.kind === spec.kind,
    identitySceneWrapperGatePassed:
      report.status === "success" &&
      report.sceneReady === true &&
      actual != null,
    authoredRootQuaternionIsUnit: unitQuaternion(
      actual?.authoredRootLocal?.rotationXyzw,
    ),
    authoredTrsMatchesSourceAudit: closeTrs(
      actual?.authoredRootLocal,
      expectedAuthored,
    ),
    appliedTrsMatchesTypedPolicy: closeTrs(
      actual?.appliedRootLocal,
      expectedApplied,
    ),
    actualRootEqualsAppliedRoot: closeTrs(
      actual?.actualRootLocal,
      expectedApplied,
    ),
    gameplayRootIsIdentity: closeTrs(actual?.gameplayRootLocal, identity),
    exactlyOneVisualRy180:
      actual?.oneCharacterHalfTurn === true &&
      closeTrs(actual?.visualContainerLocal, halfTurn) &&
      closeTrs(actual?.visualContainerWorld, halfTurn) &&
      closeTrs(actual?.rootWorld, expectedRootWorld),
    appliedOriginIsZero: closeArray(
      actual?.actualRootLocal?.translation,
      [0, 0, 0],
    ),
    appliedScaleMatchesPolicy: closeArray(
      actual?.actualRootLocal?.scale,
      expectedApplied.scale,
    ),
    animatedSkinnedBoundsAreFinite:
      report.animationPlayers > 0 &&
      report.skinnedMeshes > 0 &&
      finiteNondegenerateBounds(report.bounds),
    materialsAndShadersAreClean:
      report.materialErrors === 0 && report.shaderErrors === 0,
    screenshotWasCaptured:
      report.screenshotSaved === true &&
      report.foregroundPixels > 0 &&
      report.foregroundCoverage > 0,
  };
  const violations = Object.entries(checks)
    .filter(([, passed]) => !passed)
    .map(([name]) => name);
  return {
    id: spec.id,
    kind: spec.kind,
    model: spec.model,
    artifacts: {
      report: {
        path: projectPath(reportPath),
        byteLength: reportBytes.length,
        sha256: sha256(reportBytes),
      },
      screenshot: {
        path: projectPath(pngPath),
        byteLength: pngBytes.length,
        sha256: sha256(pngBytes),
      },
    },
    expected: {
      authoredRootLocal: expectedAuthored,
      appliedRootLocal: expectedApplied,
      gameplayRootLocal: identity,
      visualContainerLocal: halfTurn,
      visualContainerWorld: halfTurn,
      rootWorld: expectedRootWorld,
    },
    actual: {
      authoredRootLocal: actual?.authoredRootLocal,
      appliedRootLocal: actual?.appliedRootLocal,
      actualRootLocal: actual?.actualRootLocal,
      gameplayRootLocal: actual?.gameplayRootLocal,
      visualContainerLocal: actual?.visualContainerLocal,
      visualContainerWorld: actual?.visualContainerWorld,
      rootWorld: actual?.rootWorld,
      bounds: report.bounds,
      animationPlayers: report.animationPlayers,
      skinnedMeshes: report.skinnedMeshes,
    },
    checks,
    passed: violations.length === 0,
    violations,
  };
});

const violations = auditedCases.flatMap((entry) =>
  entry.violations.map((check) => `${entry.id}:${check}`),
);
const passCount = auditedCases.filter((entry) => entry.passed).length;
const audit = {
  schema: "ffone.character-runtime-gpu-audit.v2",
  assetContract: {
    source: "native GLB/PNG only",
    glbInputsModified: false,
    candidateRoot:
      "target/scale-origin-exceptions-candidate-v7-texture-defaults",
    coordinateBasis: "H=diag(-1,1,1)",
    characterFacing: "one visual-container Ry(180 degrees)",
    sceneWrapperPolicy:
      "every intermediary Bevy loader wrapper between SceneRoot and the topmost exact m_Name root has identity TRS",
    authoredRootQuaternionPolicy:
      "finite unit quaternion, abs(lengthSquared-1)<=0.0001",
  },
  cases: auditedCases,
  summary: {
    caseCount: auditedCases.length,
    passCount,
    violationCount: violations.length,
    violations,
  },
};

if (auditedCases.length !== 9 || passCount !== 9 || violations.length !== 0) {
  throw new Error(
    `character runtime audit failed: ${passCount}/${auditedCases.length}, violations=${JSON.stringify(violations)}`,
  );
}
writeFileSync(outputPath, `${JSON.stringify(audit, null, 2)}\n`, {
  flag: "wx",
});
console.log(
  JSON.stringify({
    output: projectPath(outputPath),
    caseCount: auditedCases.length,
    passCount,
    violations: violations.length,
  }),
);
