import fs from "node:fs";
import path from "node:path";

const [catalogArgument, outputArgument] = process.argv.slice(2);
if (!catalogArgument || !outputArgument) {
  console.error(
    "usage: node tools/legacy-sources/ffone-migration/report-logical-prop-candidates.mjs <logical-model-catalog.json> <fresh-output.json>",
  );
  process.exit(2);
}

const catalogPath = path.resolve(catalogArgument);
const outputPath = path.resolve(outputArgument);
if (fs.existsSync(outputPath)) {
  throw new Error(`output is never overwritten: ${outputPath}`);
}

const catalog = JSON.parse(fs.readFileSync(catalogPath, "utf8"));
if (catalog.schema !== "ffclient.logical-model-catalog.v1") {
  throw new Error(`unsupported catalog schema: ${JSON.stringify(catalog.schema)}`);
}
if (!Array.isArray(catalog.routeGroups)) {
  throw new Error("logical-model catalog has no routeGroups array");
}

const rules = [
  ["props/vegetation/trees", /^(?:tree|trees)\d*$/],
  [
    "props/vegetation/bushes",
    /^(?:bush|bushes|shrub|shrubs|hedge|hedges)\d*$/,
  ],
  [
    "props/vegetation/grass",
    /^(?:grass|grasses|weed|weeds|vine|vines|flower|flowers|plant|plants|fern|ferns)\d*$/,
  ],
  [
    "props/nature/rocks",
    /^(?:rock|rocks|stone|stones|boulder|boulders|cliff|cliffs)\d*$/,
  ],
];

const candidates = [];
const nifGroups = catalog.routeGroups.filter((group) => group.kind === "nif");
for (const group of nifGroups) {
  // Props are world/map candidates. This deliberately excludes wearables such
  // as wear/back_bush.nif even though their basenames contain a review token.
  if (!group.normalizedRoute.startsWith("map/")) {
    continue;
  }
  const tokens = group.normalizedRoute
    .toLowerCase()
    .replace(/\.nif$/, "")
    .split(/[^a-z0-9]+/)
    .filter(Boolean);

  for (const [category, pattern] of rules) {
    const matchedTokens = [...new Set(tokens.filter((token) => pattern.test(token)))];
    if (matchedTokens.length === 0) {
      continue;
    }
    candidates.push({
      normalizedRoute: group.normalizedRoute,
      exactRoutes: group.exactRoutes,
      candidateCategory: category,
      classificationStatus: "inferred_pending_review",
      runtimeEligible: false,
      inferenceEvidence: {
        kind: "normalizedRouteTokenOnly",
        matchedTokens,
      },
      ownershipReview: {
        occurrenceCount: group.occurrenceCount,
        ownerCount: group.owners.length,
        ambiguousOwner: group.ambiguousOwner,
        preferredOwnerBasis: group.preferredOwnerBasis ?? null,
        owners: group.owners,
      },
      requiredPromotionEvidence: [
        "manual visual/category review",
        "exact serialized root or world placement identity",
        "complete standalone logical-closure proof",
        "native roundtrip and runtime acceptance gates",
      ],
    });
    break;
  }
}

candidates.sort((left, right) =>
  left.candidateCategory.localeCompare(right.candidateCategory) ||
  left.normalizedRoute.localeCompare(right.normalizedRoute),
);

const byCategory = Object.fromEntries(
  rules.map(([category]) => [
    category,
    candidates.filter((candidate) => candidate.candidateCategory === category).length,
  ]),
);
const report = {
  schema: "ffone.logical-prop-candidates.v1",
  sourceCatalog: path.relative(process.cwd(), catalogPath).replaceAll("\\", "/"),
  sourceCatalogSchema: catalog.schema,
  classificationStatus: "inferred_pending_review",
  productionAssetsMutated: false,
  verified: [],
  counts: {
    catalogNifRouteCount: nifGroups.length,
    inferredPendingReviewCount: candidates.length,
    soleOwnerCandidateCount: candidates.filter(
      (candidate) => !candidate.ownershipReview.ambiguousOwner,
    ).length,
    ambiguousOwnerCandidateCount: candidates.filter(
      (candidate) => candidate.ownershipReview.ambiguousOwner,
    ).length,
    byCategory,
  },
  warning:
    "Names are discovery hints only. No candidate is verified or runtime-eligible until the complete logical closure and runtime gates pass.",
  candidates,
};

fs.mkdirSync(path.dirname(outputPath), { recursive: true });
fs.writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`, {
  encoding: "utf8",
  flag: "wx",
});
console.log(
  `wrote ${candidates.length} inferred_pending_review candidates to ${outputPath}`,
);
