
pub const STATIC_WORLD_WINDING_REPAIR_SCHEMA: &str = "ffone.static-world-winding-repair.v1";

pub const STATIC_WORLD_WINDING_REPAIR_PROOF_SCHEMA: &str =
    "ffone.static-world-winding-repair-proof.v1";

pub const STATIC_WORLD_WINDING_REPAIR_TOOL: &str =
    "ffone-asset-pipeline/repair-static-world-winding";

pub const STATIC_WORLD_WINDING_ARCHIVE_SCHEMA: &str = "ffone.static-world-winding-archive.v1";

pub(super) const REVISION_REPORT_SCHEMA: &str = "ffone.conversion-metadata-revision-report.v1";

pub(super) const REVISION_PLAN_SCHEMA: &str = "ffone.conversion-metadata-revision-plan.v1";

pub(super) const OPERATION: &str = "swap index 1 and 2 for each triangle when published geometric winding opposes authored normals; normal-less visual payloads follow the observed legacy source convention";

pub(super) const SOURCE_CONTRACT: &str = "primary retrobution-20260613 static-world GLBs; H=diag(-1,1,1); authored normals retained; glTF front faces must agree with authored normals";
