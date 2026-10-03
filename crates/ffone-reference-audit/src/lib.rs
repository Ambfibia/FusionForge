//! Deterministic reference inventory for the native FusionFall rewrite.
//!
//! This crate never executes or compiles legacy code. It reads decompiled text and OpenFusion
//! source as evidence, hashes every input, extracts a conservative set of structural facts, and
//! joins them with an explicit, reviewable coverage map.

#![forbid(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub const INVENTORY_SCHEMA: &str = "ffone.reference-inventory.v1";
pub const COVERAGE_SCHEMA: &str = "ffone.coverage-map.v1";
pub const UI_PARITY_SCHEMA: &str = "ffone.ui-parity-matrix.v1";

pub const REQUIRED_UI_MODE_IDS: [&str; 32] = [
    "mode.null",
    "mode.login",
    "mode.character_selection",
    "mode.character_creation",
    "mode.name_creation",
    "mode.main_game",
    "mode.user_equip",
    "mode.mission_system",
    "mode.vendor",
    "mode.pc2pc",
    "mode.npc_icon",
    "mode.bank",
    "mode.option",
    "mode.launcher",
    "mode.resurrect",
    "mode.world_map",
    "mode.race_mode",
    "mode.race_rank_mode",
    "mode.email",
    "mode.transpotation",
    "mode.guide",
    "mode.dexter_scene",
    "mode.nano_free_tuning",
    "mode.quit_menu",
    "mode.upsell",
    "mode.combi",
    "mode.server_selection",
    "mode.cashmall",
    "mode.user_store",
    "mode.enchant",
    "mode.rule",
    "mode.max",
];

pub const REQUIRED_UI_FAMILY_IDS: [&str; 9] = [
    "family.hud",
    "family.chat",
    "family.group",
    "family.buddy",
    "family.system_messages",
    "family.quick_slots",
    "family.buffs",
    "family.overheat",
    "family.loading",
];

const REQUIRED_UI_RAW_CONTAINERS: [&str; 6] = [
    "main.unity3d",
    "CharacterCreation.resourceFile",
    "CharacterSelection.resourceFile",
    "TableData.resourceFile",
    "Icons.resourceFile",
    "Tutorial.resourceFile",
];

#[derive(Debug, Clone)]
pub struct AuditOptions {
    pub assembly_csharp: PathBuf,
    pub assembly_firstpass: PathBuf,
    pub openfusion_src: PathBuf,
    pub coverage: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceInventory {
    pub schema: String,
    pub protocol: u32,
    pub roots: ReferenceRoots,
    pub summary: InventorySummary,
    pub scripts: Vec<ScriptInventory>,
    pub open_fusion: OpenFusionInventory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceRoots {
    pub assembly_csharp: String,
    pub assembly_firstpass: String,
    pub open_fusion: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InventorySummary {
    pub script_files: usize,
    pub classes: usize,
    pub mono_behaviours: usize,
    pub on_gui_scripts: usize,
    pub packet_layout_scripts: usize,
    pub registered_shard_packets: usize,
    pub login_packet_cases: usize,
    pub packet_definitions: usize,
    pub coverage: BTreeMap<CoverageStatus, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptInventory {
    pub source: String,
    pub bytes: u64,
    pub blake3: String,
    pub classes: Vec<ClassInventory>,
    pub has_on_gui: bool,
    pub has_network_symbols: bool,
    pub is_packet_layout: bool,
    pub coverage: CoverageStatus,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub native_modules: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClassInventory {
    pub name: String,
    pub bases: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OpenFusionInventory {
    pub packet_definitions: Vec<PacketDefinition>,
    pub shard_registrations: Vec<PacketRegistration>,
    pub login_cases: Vec<PacketCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct PacketDefinition {
    pub packet: String,
    pub source: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct PacketRegistration {
    pub packet: String,
    pub handler: String,
    pub source: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct PacketCase {
    pub packet: String,
    pub source: String,
    pub line: usize,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum CoverageStatus {
    #[default]
    Unclassified,
    NotStarted,
    Partial,
    ReferenceMatched,
    BlockedByAssets,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CoverageMap {
    pub schema: String,
    pub entries: Vec<CoverageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CoverageEntry {
    pub source: String,
    pub status: CoverageStatus,
    #[serde(default)]
    pub native_modules: Vec<String>,
    #[serde(default)]
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UiParityMatrix {
    pub schema: String,
    pub metadata: UiParityMetadata,
    pub required_mode_ids: Vec<String>,
    pub required_family_ids: Vec<String>,
    pub entries: Vec<UiParityEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UiParityMetadata {
    pub primary_build: UiPrimaryBuild,
    pub design_reference_resolution: UiViewport,
    pub authority_client_viewport: UiViewport,
    pub outer_window_reference: UiViewport,
    pub capture_harness_viewport: UiCaptureHarnessViewport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UiPrimaryBuild {
    pub source_alias: String,
    pub build_id: String,
    pub display_name: String,
    pub root: String,
    pub source_map: String,
    pub raw_container_hashes: Vec<UiRawContainerHash>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UiRawContainerHash {
    pub relative_path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UiViewport {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UiCaptureHarnessViewport {
    pub width: u32,
    pub height: u32,
    pub matches_authority: bool,
    pub parity_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UiParityEntry {
    pub id: String,
    pub kind: UiParityEntryKind,
    pub legacy_mode_or_family: String,
    pub legacy_anchors: UiLegacyAnchors,
    pub native_status: UiNativeStatus,
    pub native_modules: Vec<String>,
    pub native_assets: Vec<String>,
    pub native_tests: Vec<String>,
    pub native_captures: Vec<String>,
    pub functional_gaps: Vec<String>,
    pub evidence_authority: Vec<UiEvidenceAuthority>,
    pub acceptance_gates: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UiParityEntryKind {
    Mode,
    Family,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UiLegacyAnchors {
    pub classes: Vec<String>,
    pub roots: Vec<String>,
    pub skins: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UiNativeStatus {
    Missing,
    Partial,
    Implemented,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UiEvidenceAuthority {
    pub authority: UiEvidenceAuthorityKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_alias: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    pub locator: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UiEvidenceAuthorityKind {
    PrimaryRaw,
    PrimaryVerifiedDerivative,
    PrimaryLiveCapture,
    PrimaryRuntime,
    NativeEvidence,
    NativePublished,
    PinnedServerSource,
}

pub fn load_ui_parity_matrix(path: &Path) -> Result<UiParityMatrix, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let matrix: UiParityMatrix = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid UI parity matrix {}: {error}", path.display()))?;
    validate_ui_parity_matrix(&matrix)?;
    Ok(matrix)
}

pub fn validate_ui_parity_matrix(matrix: &UiParityMatrix) -> Result<(), String> {
    if matrix.schema != UI_PARITY_SCHEMA {
        return Err(format!(
            "unsupported UI parity schema {:?}; expected {UI_PARITY_SCHEMA}",
            matrix.schema
        ));
    }

    let expected_modes = REQUIRED_UI_MODE_IDS
        .iter()
        .map(|id| (*id).to_owned())
        .collect::<Vec<_>>();
    if matrix.required_mode_ids != expected_modes {
        return Err(
            "required_mode_ids must contain the canonical 32 eGameMode IDs in order".into(),
        );
    }
    let expected_families = REQUIRED_UI_FAMILY_IDS
        .iter()
        .map(|id| (*id).to_owned())
        .collect::<Vec<_>>();
    if matrix.required_family_ids != expected_families {
        return Err(
            "required_family_ids must contain the canonical cross-mode family IDs in order".into(),
        );
    }

    validate_ui_parity_metadata(&matrix.metadata)?;

    let required_modes = REQUIRED_UI_MODE_IDS.into_iter().collect::<BTreeSet<_>>();
    let required_families = REQUIRED_UI_FAMILY_IDS.into_iter().collect::<BTreeSet<_>>();
    let mut seen_ids = BTreeSet::new();
    let mut found_modes = BTreeSet::new();
    let mut found_families = BTreeSet::new();

    for entry in &matrix.entries {
        if entry.id.trim().is_empty() {
            return Err("UI parity entry ID may not be empty".into());
        }
        if !seen_ids.insert(entry.id.as_str()) {
            return Err(format!("duplicate UI parity entry ID {:?}", entry.id));
        }
        match entry.kind {
            UiParityEntryKind::Mode => {
                if !required_modes.contains(entry.id.as_str()) {
                    return Err(format!("unexpected eGameMode entry ID {:?}", entry.id));
                }
                found_modes.insert(entry.id.as_str());
            }
            UiParityEntryKind::Family => {
                if !required_families.contains(entry.id.as_str()) {
                    return Err(format!(
                        "unexpected cross-mode family entry ID {:?}",
                        entry.id
                    ));
                }
                found_families.insert(entry.id.as_str());
            }
        }

        validate_non_empty_string(
            &entry.legacy_mode_or_family,
            &entry.id,
            "legacy_mode_or_family",
        )?;
        validate_string_array(
            &entry.legacy_anchors.classes,
            &entry.id,
            "legacy_anchors.classes",
        )?;
        validate_string_array(
            &entry.legacy_anchors.roots,
            &entry.id,
            "legacy_anchors.roots",
        )?;
        validate_string_array(
            &entry.legacy_anchors.skins,
            &entry.id,
            "legacy_anchors.skins",
        )?;
        if entry.legacy_anchors.classes.is_empty()
            && entry.legacy_anchors.roots.is_empty()
            && entry.legacy_anchors.skins.is_empty()
        {
            return Err(format!(
                "{} has no legacy class, root, or skin anchor",
                entry.id
            ));
        }

        validate_string_array(&entry.native_modules, &entry.id, "native_modules")?;
        validate_string_array(&entry.native_assets, &entry.id, "native_assets")?;
        validate_string_array(&entry.native_tests, &entry.id, "native_tests")?;
        validate_string_array(&entry.native_captures, &entry.id, "native_captures")?;
        validate_string_array(&entry.functional_gaps, &entry.id, "functional_gaps")?;
        validate_string_array(&entry.acceptance_gates, &entry.id, "acceptance_gates")?;

        if entry.evidence_authority.is_empty() {
            return Err(format!("{} has no legacy evidence authority", entry.id));
        }
        for evidence in &entry.evidence_authority {
            match evidence.authority {
                UiEvidenceAuthorityKind::PrimaryRaw
                | UiEvidenceAuthorityKind::PrimaryVerifiedDerivative
                | UiEvidenceAuthorityKind::PrimaryLiveCapture
                | UiEvidenceAuthorityKind::PrimaryRuntime => {
                    validate_non_empty_string(
                        &evidence.source_alias,
                        &entry.id,
                        "evidence_authority.source_alias",
                    )?;
                    if evidence.source_alias != matrix.metadata.primary_build.source_alias {
                        return Err(format!(
                            "{} primary evidence uses source alias {:?}",
                            entry.id, evidence.source_alias
                        ));
                    }
                }
                UiEvidenceAuthorityKind::NativePublished
                | UiEvidenceAuthorityKind::PinnedServerSource => validate_non_empty_string(
                    &evidence.source_alias,
                    &entry.id,
                    "evidence_authority.source_alias",
                )?,
                UiEvidenceAuthorityKind::NativeEvidence => {}
            }
            validate_non_empty_string(&evidence.path, &entry.id, "evidence_authority.path")?;
            validate_non_empty_string(&evidence.locator, &entry.id, "evidence_authority.locator")?;
            match (&evidence.size_bytes, &evidence.sha256) {
                (Some(bytes), Some(sha256)) => {
                    if *bytes == 0 {
                        return Err(format!(
                            "{} evidence_authority.size_bytes must be positive",
                            entry.id
                        ));
                    }
                    if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                        return Err(format!(
                            "{} evidence_authority.sha256 must be 64 hexadecimal characters",
                            entry.id
                        ));
                    }
                }
                (None, None) => {}
                _ => {
                    return Err(format!(
                        "{} evidence authority must provide size_bytes and sha256 together",
                        entry.id
                    ));
                }
            }
        }
        if entry.acceptance_gates.is_empty() {
            return Err(format!("{} has no acceptance gates", entry.id));
        }

        if entry.native_status == UiNativeStatus::Implemented {
            if entry.native_tests.is_empty() {
                return Err(format!(
                    "{} may not be implemented without native tests",
                    entry.id
                ));
            }
            if entry.native_captures.is_empty() {
                return Err(format!(
                    "{} may not be implemented without native captures",
                    entry.id
                ));
            }
            if entry.evidence_authority.is_empty() {
                return Err(format!(
                    "{} may not be implemented without legacy evidence",
                    entry.id
                ));
            }
            if !entry.functional_gaps.is_empty() {
                return Err(format!(
                    "{} may not be implemented while functional gaps remain",
                    entry.id
                ));
            }
        } else if entry.functional_gaps.is_empty() {
            return Err(format!(
                "{} must describe functional gaps while status is {:?}",
                entry.id, entry.native_status
            ));
        }
    }

    if found_modes != required_modes {
        let missing = required_modes
            .difference(&found_modes)
            .copied()
            .collect::<Vec<_>>();
        return Err(format!("missing required eGameMode entries: {missing:?}"));
    }
    if found_families != required_families {
        let missing = required_families
            .difference(&found_families)
            .copied()
            .collect::<Vec<_>>();
        return Err(format!(
            "missing required cross-mode family entries: {missing:?}"
        ));
    }
    Ok(())
}

fn validate_ui_parity_metadata(metadata: &UiParityMetadata) -> Result<(), String> {
    if metadata.primary_build.source_alias != "primary" {
        return Err("UI parity primary_build.source_alias must be \"primary\"".into());
    }
    if metadata.primary_build.build_id != "retrobution-20260613" {
        return Err("UI parity primary_build.build_id must be \"retrobution-20260613\"".into());
    }
    for (field, value) in [
        (
            "primary_build.display_name",
            &metadata.primary_build.display_name,
        ),
        ("primary_build.root", &metadata.primary_build.root),
        (
            "primary_build.source_map",
            &metadata.primary_build.source_map,
        ),
    ] {
        validate_non_empty_string(value, "metadata", field)?;
    }

    let required_containers = REQUIRED_UI_RAW_CONTAINERS
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut found_containers = BTreeSet::new();
    for raw in &metadata.primary_build.raw_container_hashes {
        validate_non_empty_string(&raw.relative_path, "metadata", "raw relative_path")?;
        if !required_containers.contains(raw.relative_path.as_str()) {
            return Err(format!(
                "unexpected primary UI raw container {:?}",
                raw.relative_path
            ));
        }
        if !found_containers.insert(raw.relative_path.as_str()) {
            return Err(format!(
                "duplicate primary UI raw container {:?}",
                raw.relative_path
            ));
        }
        if raw.bytes == 0 {
            return Err(format!(
                "raw container {:?} may not have zero bytes",
                raw.relative_path
            ));
        }
        if raw.sha256.len() != 64 || !raw.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!(
                "raw container {:?} has invalid SHA-256 {:?}",
                raw.relative_path, raw.sha256
            ));
        }
    }
    if found_containers != required_containers {
        let missing = required_containers
            .difference(&found_containers)
            .copied()
            .collect::<Vec<_>>();
        return Err(format!(
            "missing primary UI raw container hashes: {missing:?}"
        ));
    }

    if metadata.design_reference_resolution
        != (UiViewport {
            width: 1280,
            height: 720,
        })
    {
        return Err("UI parity design_reference_resolution must be exactly 1280x720".into());
    }
    if metadata.authority_client_viewport
        != (UiViewport {
            width: 1264,
            height: 681,
        })
    {
        return Err("UI parity authority_client_viewport must be exactly 1264x681".into());
    }
    if metadata.outer_window_reference
        != (UiViewport {
            width: 1280,
            height: 720,
        })
    {
        return Err("UI parity outer_window_reference must be exactly 1280x720".into());
    }
    let harness = &metadata.capture_harness_viewport;
    if harness.width != 1264 || harness.height != 681 {
        return Err("capture_harness_viewport must document the 1264x681 harness".into());
    }
    if !harness.matches_authority {
        return Err(
            "the 1264x681 capture harness must match the measured Unity client authority".into(),
        );
    }
    validate_non_empty_string(
        &harness.parity_note,
        "metadata",
        "capture_harness_viewport.parity_note",
    )
}

fn validate_non_empty_string(value: &str, id: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{id} has an empty {field}"));
    }
    Ok(())
}

fn validate_string_array(values: &[String], id: &str, field: &str) -> Result<(), String> {
    for value in values {
        validate_non_empty_string(value, id, field)?;
    }
    Ok(())
}

pub fn scan(options: &AuditOptions) -> Result<ReferenceInventory, String> {
    let coverage = load_coverage(options.coverage.as_deref())?;
    let mut coverage_by_source = BTreeMap::new();
    for entry in coverage.entries {
        let source = normalize_logical_path(&entry.source)?;
        if coverage_by_source.insert(source.clone(), entry).is_some() {
            return Err(format!("duplicate coverage entry for {source}"));
        }
    }

    let mut scripts = Vec::new();
    scan_csharp_root(
        "assembly-csharp",
        &options.assembly_csharp,
        &coverage_by_source,
        &mut scripts,
    )?;
    scan_csharp_root(
        "assembly-firstpass",
        &options.assembly_firstpass,
        &coverage_by_source,
        &mut scripts,
    )?;
    scripts.sort_by(|left, right| left.source.cmp(&right.source));

    let known_sources = scripts
        .iter()
        .map(|script| script.source.as_str())
        .collect::<BTreeSet<_>>();
    for source in coverage_by_source.keys() {
        if !known_sources.contains(source.as_str()) {
            return Err(format!(
                "coverage source {source:?} does not exist in the selected decompile"
            ));
        }
    }

    let open_fusion = scan_openfusion(&options.openfusion_src)?;
    let mut summary = InventorySummary {
        script_files: scripts.len(),
        classes: scripts.iter().map(|script| script.classes.len()).sum(),
        mono_behaviours: scripts
            .iter()
            .flat_map(|script| &script.classes)
            .filter(|class| class.bases.iter().any(|base| base == "MonoBehaviour"))
            .count(),
        on_gui_scripts: scripts.iter().filter(|script| script.has_on_gui).count(),
        packet_layout_scripts: scripts
            .iter()
            .filter(|script| script.is_packet_layout)
            .count(),
        registered_shard_packets: open_fusion.shard_registrations.len(),
        login_packet_cases: open_fusion.login_cases.len(),
        packet_definitions: open_fusion.packet_definitions.len(),
        coverage: BTreeMap::new(),
    };
    for script in &scripts {
        *summary.coverage.entry(script.coverage).or_default() += 1;
    }

    Ok(ReferenceInventory {
        schema: INVENTORY_SCHEMA.to_owned(),
        protocol: 104,
        roots: ReferenceRoots {
            assembly_csharp: display_path(&options.assembly_csharp),
            assembly_firstpass: display_path(&options.assembly_firstpass),
            open_fusion: display_path(&options.openfusion_src),
        },
        summary,
        scripts,
        open_fusion,
    })
}

pub fn write_inventory(path: &Path, inventory: &ReferenceInventory) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    let mut bytes = serde_json::to_vec_pretty(inventory)
        .map_err(|error| format!("could not serialize inventory: {error}"))?;
    bytes.push(b'\n');
    fs::write(path, bytes).map_err(|error| format!("could not write {}: {error}", path.display()))
}

fn load_coverage(path: Option<&Path>) -> Result<CoverageMap, String> {
    let Some(path) = path else {
        return Ok(CoverageMap {
            schema: COVERAGE_SCHEMA.to_owned(),
            entries: Vec::new(),
        });
    };
    if !path.exists() {
        return Err(format!("coverage map does not exist: {}", path.display()));
    }
    let bytes =
        fs::read(path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let map: CoverageMap = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid coverage map {}: {error}", path.display()))?;
    if map.schema != COVERAGE_SCHEMA {
        return Err(format!(
            "unsupported coverage schema {:?}; expected {COVERAGE_SCHEMA}",
            map.schema
        ));
    }
    Ok(map)
}

fn scan_csharp_root(
    label: &str,
    root: &Path,
    coverage: &BTreeMap<String, CoverageEntry>,
    output: &mut Vec<ScriptInventory>,
) -> Result<(), String> {
    if !root.is_dir() {
        return Err(format!(
            "C# reference root is not a directory: {}",
            root.display()
        ));
    }
    for path in collect_files(root, &["cs"])? {
        let relative = portable_relative(root, &path)?;
        let source = format!("{label}/{relative}");
        let bytes = fs::read(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        let text = String::from_utf8_lossy(&bytes);
        let classes = parse_classes(&text);
        let filename = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        let mapped = coverage.get(&source);
        output.push(ScriptInventory {
            source,
            bytes: bytes.len() as u64,
            blake3: blake3::hash(&bytes).to_hex().to_string(),
            classes,
            has_on_gui: text.contains("OnGUI("),
            has_network_symbols: text.contains("P_FE2CL_")
                || text.contains("P_CL2FE_")
                || text.contains("P_CL2LS_")
                || text.contains("P_LS2CL_"),
            is_packet_layout: filename.starts_with("sP_") || filename.starts_with("sPacket"),
            coverage: mapped.map_or(CoverageStatus::Unclassified, |entry| entry.status),
            native_modules: mapped
                .map(|entry| entry.native_modules.clone())
                .unwrap_or_default(),
            evidence: mapped.and_then(|entry| entry.evidence.clone()),
        });
    }
    Ok(())
}

fn parse_classes(text: &str) -> Vec<ClassInventory> {
    let mut classes = Vec::new();
    for line in text.lines() {
        let declaration = line.trim().trim_start_matches('\u{feff}');
        let tokens = declaration.split_whitespace().collect::<Vec<_>>();
        let Some(class_index) = tokens.iter().position(|token| *token == "class") else {
            continue;
        };
        let Some(raw_name) = tokens.get(class_index + 1) else {
            continue;
        };
        let name = raw_name
            .trim_end_matches([':', '{'])
            .split('<')
            .next()
            .unwrap_or("")
            .to_owned();
        if name.is_empty() {
            continue;
        }
        let bases = declaration
            .split_once(':')
            .map(|(_, bases)| {
                bases
                    .split('{')
                    .next()
                    .unwrap_or("")
                    .split(',')
                    .map(str::trim)
                    .filter(|base| !base.is_empty())
                    .map(|base| base.split('<').next().unwrap_or(base).to_owned())
                    .collect()
            })
            .unwrap_or_default();
        classes.push(ClassInventory { name, bases });
    }
    classes
}

fn scan_openfusion(root: &Path) -> Result<OpenFusionInventory, String> {
    if !root.is_dir() {
        return Err(format!(
            "OpenFusion source root is not a directory: {}",
            root.display()
        ));
    }
    let mut inventory = OpenFusionInventory::default();
    for path in collect_files(root, &["cpp", "hpp", "h"])? {
        let relative = portable_relative(root, &path)?;
        let bytes = fs::read(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        let text = String::from_utf8_lossy(&bytes);
        for (index, line) in text.lines().enumerate() {
            let line_number = index + 1;
            if let Some(arguments) = macro_arguments(line, "REGISTER_SHARD_PACKET") {
                let mut arguments = arguments.split(',').map(str::trim);
                if let (Some(packet), Some(handler)) = (arguments.next(), arguments.next()) {
                    if packet.starts_with('P') {
                        inventory.shard_registrations.push(PacketRegistration {
                            packet: packet.to_owned(),
                            handler: handler.to_owned(),
                            source: relative.clone(),
                            line: line_number,
                        });
                    }
                }
            }
            if let Some(arguments) = macro_arguments(line, "PACKET") {
                let packet = arguments.split(',').next().unwrap_or("").trim();
                if packet.starts_with('P') {
                    inventory.packet_definitions.push(PacketDefinition {
                        packet: packet.to_owned(),
                        source: relative.clone(),
                        line: line_number,
                    });
                }
            }
            let trimmed = line.trim();
            if relative.ends_with("servers/CNLoginServer.cpp")
                && trimmed.starts_with("case P_CL2LS_")
            {
                if let Some((packet, _)) = trimmed.trim_start_matches("case ").split_once(':') {
                    inventory.login_cases.push(PacketCase {
                        packet: packet.to_owned(),
                        source: relative.clone(),
                        line: line_number,
                    });
                }
            }
        }
    }
    inventory.packet_definitions.sort();
    inventory.packet_definitions.dedup();
    inventory.shard_registrations.sort();
    inventory.shard_registrations.dedup();
    inventory.login_cases.sort();
    inventory.login_cases.dedup();
    Ok(inventory)
}

fn macro_arguments<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let start = line.find(name)? + name.len();
    let prefix = line.get(..start - name.len())?;
    if prefix
        .chars()
        .next_back()
        .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return None;
    }
    let rest = line.get(start..)?.trim_start();
    let rest = rest.strip_prefix('(')?;
    let end = rest.find(')')?;
    rest.get(..end)
}

fn collect_files(root: &Path, extensions: &[&str]) -> Result<Vec<PathBuf>, String> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|error| format!("could not read {}: {error}", directory.display()))?;
        for entry in entries {
            let entry = entry
                .map_err(|error| format!("could not enumerate {}: {error}", directory.display()))?;
            let file_type = entry
                .file_type()
                .map_err(|error| format!("could not stat {}: {error}", entry.path().display()))?;
            if file_type.is_symlink() {
                return Err(format!(
                    "reference roots may not contain symlinks: {}",
                    entry.path().display()
                ));
            }
            if file_type.is_dir() {
                pending.push(entry.path());
                continue;
            }
            let extension = entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if extensions.contains(&extension.as_str()) {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    Ok(files)
}

fn portable_relative(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| format!("{} escaped root {}", path.display(), root.display()))?;
    let components = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    normalize_logical_path(&components.join("/"))
}

fn normalize_logical_path(path: &str) -> Result<String, String> {
    let path = path.replace('\\', "/");
    if path.is_empty()
        || path.starts_with('/')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(format!("invalid logical source path {path:?}"));
    }
    Ok(path)
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests;

pub mod cli;
