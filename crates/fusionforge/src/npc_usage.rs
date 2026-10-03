use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

const MAX_REFERENCE_SAMPLES: usize = 10;
const NPC_RUNTIME_ID_OFFSET: usize = 1;
const MOB_RUNTIME_ID_OFFSET: usize = 10_000;
const MOB_GROUP_RUNTIME_ID_OFFSET: usize = 30_000;

const NPC_ID_CONTRACT: &str = "NpcCatalogRecord.id is the zero-based index in \
m_pNpcTable.m_pNpcData. In stock OpenFusion xdt.json that index is also stored in \
the row's m_iNpcNumber field.";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NpcUsageInspection {
    pub selected_path: String,
    pub tdata_dir: String,
    pub xdt_path: String,
    pub id_contract: String,
    pub server_catalog_count: usize,
    pub catalog_count: usize,
    pub used_count: usize,
    pub unused_count: usize,
    pub on_map_count: usize,
    pub quest_referenced_count: usize,
    pub other_referenced_count: usize,
    pub missing_files: Vec<String>,
    pub warnings: Vec<String>,
    pub unknown_reference_ids: Vec<usize>,
    pub records: Vec<NpcUsageRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NpcUsageRecord {
    pub npc_id: usize,
    pub server_catalog_present: bool,
    pub used: bool,
    pub unused: bool,
    pub on_map: bool,
    pub quest_referenced: bool,
    pub other_referenced: bool,
    pub on_map_count: usize,
    pub quest_reference_count: usize,
    pub other_reference_count: usize,
    pub map_samples: Vec<NpcUsageReference>,
    pub quest_samples: Vec<NpcUsageReference>,
    pub other_samples: Vec<NpcUsageReference>,
    pub sample_descriptions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NpcUsageReference {
    pub source_file: String,
    pub section: String,
    pub field: String,
    pub row: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy)]
enum UsageKind {
    Map,
    Quest,
    Other,
}

#[derive(Debug, Default)]
struct UsageAccumulator {
    map_count: usize,
    quest_count: usize,
    other_count: usize,
    map_samples: Vec<NpcUsageReference>,
    quest_samples: Vec<NpcUsageReference>,
    other_samples: Vec<NpcUsageReference>,
}

impl UsageAccumulator {
    fn push(&mut self, kind: UsageKind, reference: NpcUsageReference) {
        let (count, samples) = match kind {
            UsageKind::Map => (&mut self.map_count, &mut self.map_samples),
            UsageKind::Quest => (&mut self.quest_count, &mut self.quest_samples),
            UsageKind::Other => (&mut self.other_count, &mut self.other_samples),
        };
        *count += 1;
        if samples.len() < MAX_REFERENCE_SAMPLES {
            samples.push(reference);
        }
    }
}

pub(crate) fn inspect_npc_usage(server_tdata_path: String) -> Result<NpcUsageInspection, String> {
    inspect_npc_usage_path(Path::new(&server_tdata_path), None)
}

/// Uses the client catalog IDs as the result domain while still validating them against the
/// selected server xdt.json. This is the preferred entry point for the NPC editor because a
/// staged client NPC may not have been copied into server tabledata yet.
pub(crate) fn inspect_npc_usage_for_catalog(
    server_tdata_path: String,
    catalog_ids: Vec<usize>,
) -> Result<NpcUsageInspection, String> {
    inspect_npc_usage_path(Path::new(&server_tdata_path), Some(catalog_ids))
}

fn inspect_npc_usage_path(
    selected_path: &Path,
    requested_catalog_ids: Option<Vec<usize>>,
) -> Result<NpcUsageInspection, String> {
    let (selected_path, tdata_dir, xdt_path) = resolve_tdata_path(selected_path)?;
    let xdt = read_required_json(&xdt_path)?;
    let mut warnings = Vec::new();
    let server_catalog_ids = server_catalog_ids(&xdt, &mut warnings)?;

    let catalog_ids = requested_catalog_ids
        .filter(|ids| !ids.is_empty())
        .map(|ids| {
            ids.into_iter()
                .filter(|id| *id != 0)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_else(|| server_catalog_ids.clone());

    let mut missing_files = Vec::new();
    let npcs = read_optional_json(&tdata_dir, "NPCs.json", &mut missing_files, &mut warnings);
    let mobs = read_optional_json(&tdata_dir, "mobs.json", &mut missing_files, &mut warnings);
    let gruntwork = read_optional_json(
        &tdata_dir,
        "gruntwork.json",
        &mut missing_files,
        &mut warnings,
    );
    let paths = read_optional_json(&tdata_dir, "paths.json", &mut missing_files, &mut warnings);
    let drops = read_optional_json(&tdata_dir, "drops.json", &mut missing_files, &mut warnings);

    let mut usage = BTreeMap::<usize, UsageAccumulator>::new();
    let mut runtime_npc_types = BTreeMap::<usize, usize>::new();

    if let Some(npcs) = npcs.as_ref() {
        scan_static_npcs(npcs, &mut usage, &mut runtime_npc_types);
    }
    if let Some(mobs) = mobs.as_ref() {
        scan_static_mobs(mobs, &mut usage, &mut runtime_npc_types);
    }
    if let Some(gruntwork) = gruntwork.as_ref() {
        scan_gruntwork_spawns(gruntwork, &mut usage);
    }

    scan_mission_references(&xdt, &mut usage);
    scan_xdt_other_references(&xdt, &mut usage);

    let mut unresolved_runtime_path_ids = 0usize;
    if let Some(paths) = paths.as_ref() {
        scan_npc_paths(
            "paths.json",
            paths.get("npc"),
            &runtime_npc_types,
            &mut usage,
            &mut unresolved_runtime_path_ids,
        );
    }
    if let Some(gruntwork) = gruntwork.as_ref() {
        scan_npc_paths(
            "gruntwork.json",
            gruntwork.get("paths"),
            &runtime_npc_types,
            &mut usage,
            &mut unresolved_runtime_path_ids,
        );
    }
    if unresolved_runtime_path_ids != 0 {
        warnings.push(format!(
            "{unresolved_runtime_path_ids} path aNPCIDs reference(s) could not be mapped to an \
             NPC type. aNPCIDs stores runtime spawn IDs, not NpcCatalogRecord IDs."
        ));
    }
    if let Some(drops) = drops.as_ref() {
        scan_drop_references(drops, &mut usage);
    }

    let unknown_reference_ids = usage
        .keys()
        .copied()
        .filter(|id| !server_catalog_ids.contains(id))
        .collect::<Vec<_>>();
    if !unknown_reference_ids.is_empty() {
        warnings.push(format!(
            "Server JSON references {} NPC type ID(s) that are absent from the selected \
             xdt.json catalog.",
            unknown_reference_ids.len()
        ));
    }

    let records = catalog_ids
        .iter()
        .copied()
        .map(|npc_id| {
            usage_record(
                npc_id,
                server_catalog_ids.contains(&npc_id),
                usage.remove(&npc_id),
            )
        })
        .collect::<Vec<_>>();
    let used_count = records.iter().filter(|record| record.used).count();
    let unused_count = records.iter().filter(|record| record.unused).count();
    let on_map_count = records.iter().filter(|record| record.on_map).count();
    let quest_referenced_count = records
        .iter()
        .filter(|record| record.quest_referenced)
        .count();
    let other_referenced_count = records
        .iter()
        .filter(|record| record.other_referenced)
        .count();

    Ok(NpcUsageInspection {
        selected_path: display_path(&selected_path),
        tdata_dir: display_path(&tdata_dir),
        xdt_path: display_path(&xdt_path),
        id_contract: NPC_ID_CONTRACT.to_string(),
        server_catalog_count: server_catalog_ids.len(),
        catalog_count: records.len(),
        used_count,
        unused_count,
        on_map_count,
        quest_referenced_count,
        other_referenced_count,
        missing_files,
        warnings,
        unknown_reference_ids,
        records,
    })
}

fn resolve_tdata_path(selected_path: &Path) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    if !selected_path.exists() {
        return Err(format!(
            "Server tabledata path does not exist: {}",
            selected_path.display()
        ));
    }

    let (tdata_dir, xdt_path) = if selected_path.is_dir() {
        let xdt = find_file_case_insensitive(selected_path, "xdt.json").ok_or_else(|| {
            format!(
                "Selected tabledata folder has no xdt.json: {}",
                selected_path.display()
            )
        })?;
        (selected_path.to_path_buf(), xdt)
    } else {
        let file_name = selected_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if !file_name.eq_ignore_ascii_case("xdt.json") {
            return Err(format!(
                "Select the server tdata folder or its xdt.json, not {}",
                selected_path.display()
            ));
        }
        let parent = selected_path
            .parent()
            .ok_or_else(|| format!("xdt.json has no parent folder: {}", selected_path.display()))?;
        (parent.to_path_buf(), selected_path.to_path_buf())
    };

    let selected_path =
        fs::canonicalize(selected_path).unwrap_or_else(|_| selected_path.to_path_buf());
    let tdata_dir = fs::canonicalize(&tdata_dir).unwrap_or(tdata_dir);
    let xdt_path = fs::canonicalize(&xdt_path).unwrap_or(xdt_path);
    Ok((selected_path, tdata_dir, xdt_path))
}

fn find_file_case_insensitive(dir: &Path, file_name: &str) -> Option<PathBuf> {
    let direct = dir.join(file_name);
    if direct.is_file() {
        return Some(direct);
    }
    fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .find_map(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.eq_ignore_ascii_case(file_name))
                .then(|| entry.path())
        })
}

fn read_required_json(path: &Path) -> Result<JsonValue, String> {
    let data = fs::read_to_string(path)
        .map_err(|err| format!("Failed to read {}: {err}", path.display()))?;
    serde_json::from_str(&data).map_err(|err| format!("Failed to parse {}: {err}", path.display()))
}

fn read_optional_json(
    dir: &Path,
    file_name: &str,
    missing_files: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Option<JsonValue> {
    let Some(path) = find_file_case_insensitive(dir, file_name) else {
        missing_files.push(file_name.to_string());
        return None;
    };
    match read_required_json(&path) {
        Ok(value) => Some(value),
        Err(error) => {
            warnings.push(error);
            None
        }
    }
}

fn server_catalog_ids(
    xdt: &JsonValue,
    warnings: &mut Vec<String>,
) -> Result<BTreeSet<usize>, String> {
    let rows = xdt
        .pointer("/m_pNpcTable/m_pNpcData")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| "xdt.json has no m_pNpcTable.m_pNpcData array".to_string())?;
    let mut ids = BTreeSet::new();
    let mut mismatches = Vec::new();
    let mut mismatch_count = 0usize;
    for (index, row) in rows.iter().enumerate() {
        // Zero is the XDT sentinel/empty row and is not emitted by inspect_npc_catalog_impl.
        if index != 0 {
            ids.insert(index);
        }
        if let Some(stored_id) = row.get("m_iNpcNumber").and_then(json_usize) {
            if stored_id != index {
                mismatch_count += 1;
                if mismatches.len() < 8 {
                    mismatches.push(format!("row {index} stores {stored_id}"));
                }
            }
        }
    }
    if mismatch_count != 0 {
        warnings.push(format!(
            "{mismatch_count} m_pNpcData row(s) have m_iNpcNumber different from their array \
             index ({}). NpcCatalogRecord.id follows the array index.",
            mismatches.join(", ")
        ));
    }
    Ok(ids)
}

fn scan_static_npcs(
    root: &JsonValue,
    usage: &mut BTreeMap<usize, UsageAccumulator>,
    runtime_npc_types: &mut BTreeMap<usize, usize>,
) {
    for (spawn_id, row) in json_rows(root.get("NPCs")) {
        let Some(npc_id) = row.get("iNPCType").and_then(json_usize) else {
            continue;
        };
        add_usage(
            usage,
            npc_id,
            UsageKind::Map,
            usage_reference(
                "NPCs.json",
                "NPCs",
                "iNPCType",
                &spawn_id,
                format!(
                    "Статический NPC #{spawn_id}: {}",
                    spawn_location_description(row)
                ),
            ),
        );
        if let Ok(raw_id) = spawn_id.parse::<usize>() {
            runtime_npc_types.insert(raw_id + NPC_RUNTIME_ID_OFFSET, npc_id);
        }
    }
}

fn scan_static_mobs(
    root: &JsonValue,
    usage: &mut BTreeMap<usize, UsageAccumulator>,
    runtime_npc_types: &mut BTreeMap<usize, usize>,
) {
    for (spawn_id, row) in json_rows(root.get("mobs")) {
        let Some(npc_id) = row.get("iNPCType").and_then(json_usize) else {
            continue;
        };
        add_usage(
            usage,
            npc_id,
            UsageKind::Map,
            usage_reference(
                "mobs.json",
                "mobs",
                "iNPCType",
                &spawn_id,
                format!(
                    "Одиночный моб #{spawn_id}: {}",
                    spawn_location_description(row)
                ),
            ),
        );
        if let Ok(raw_id) = spawn_id.parse::<usize>() {
            runtime_npc_types.insert(raw_id + MOB_RUNTIME_ID_OFFSET, npc_id);
        }
    }

    for (group_id, leader) in json_rows(root.get("groups")) {
        if let Some(npc_id) = leader.get("iNPCType").and_then(json_usize) {
            add_usage(
                usage,
                npc_id,
                UsageKind::Map,
                usage_reference(
                    "mobs.json",
                    "groups",
                    "iNPCType",
                    &group_id,
                    format!(
                        "Лидер группы мобов #{group_id}: {}",
                        spawn_location_description(leader)
                    ),
                ),
            );
            if let Ok(raw_id) = group_id.parse::<usize>() {
                runtime_npc_types.insert(raw_id + MOB_GROUP_RUNTIME_ID_OFFSET, npc_id);
            }
        }
        for (follower_index, follower) in json_rows(leader.get("aFollowers")) {
            let Some(npc_id) = follower.get("iNPCType").and_then(json_usize) else {
                continue;
            };
            add_usage(
                usage,
                npc_id,
                UsageKind::Map,
                usage_reference(
                    "mobs.json",
                    "groups.aFollowers",
                    "iNPCType",
                    &format!("{group_id}:{follower_index}"),
                    format!("Участник {follower_index} группы мобов #{group_id}"),
                ),
            );
        }
    }
}

fn scan_gruntwork_spawns(root: &JsonValue, usage: &mut BTreeMap<usize, UsageAccumulator>) {
    for (spawn_id, row) in json_rows(root.get("mobs")) {
        let Some(npc_id) = row.get("iNPCType").and_then(json_usize) else {
            continue;
        };
        add_usage(
            usage,
            npc_id,
            UsageKind::Map,
            usage_reference(
                "gruntwork.json",
                "mobs",
                "iNPCType",
                &spawn_id,
                format!(
                    "Gruntwork-моб #{spawn_id}: {}",
                    spawn_location_description(row)
                ),
            ),
        );
    }
    for (group_id, leader) in json_rows(root.get("groups")) {
        if let Some(npc_id) = leader.get("iNPCType").and_then(json_usize) {
            add_usage(
                usage,
                npc_id,
                UsageKind::Map,
                usage_reference(
                    "gruntwork.json",
                    "groups",
                    "iNPCType",
                    &group_id,
                    format!("Лидер gruntwork-группы #{group_id}"),
                ),
            );
        }
        for (follower_index, follower) in json_rows(leader.get("aFollowers")) {
            let Some(npc_id) = follower.get("iNPCType").and_then(json_usize) else {
                continue;
            };
            add_usage(
                usage,
                npc_id,
                UsageKind::Map,
                usage_reference(
                    "gruntwork.json",
                    "groups.aFollowers",
                    "iNPCType",
                    &format!("{group_id}:{follower_index}"),
                    format!("Участник {follower_index} gruntwork-группы #{group_id}"),
                ),
            );
        }
    }
}

fn scan_mission_references(xdt: &JsonValue, usage: &mut BTreeMap<usize, UsageAccumulator>) {
    const SCALAR_FIELDS: &[(&str, &str)] = &[
        ("m_iHNPCID", "основной NPC задания"),
        ("m_iHJournalNPCID", "NPC журнала"),
        ("m_iHTerminatorNPCID", "NPC завершения задания"),
        ("m_iCSUDEFNPCID", "защищаемый/сопровождаемый NPC"),
        ("m_iSTGrantWayPoint", "NPC путевой точки при старте"),
        ("m_iSTSpawnMonsterID", "создаваемый заданием монстр"),
        ("m_iSTDialogBubbleNPCID", "NPC реплики при старте"),
        ("m_iSUDialogBubbleNPCID", "NPC реплики при успехе"),
        ("m_iFDialogBubbleNPCID", "NPC реплики при провале"),
        ("m_iSTMessageSendNPC", "отправитель сообщения при старте"),
        ("m_iSUMessageSendNPC", "отправитель сообщения при успехе"),
        ("m_iFMessageSendNPC", "отправитель сообщения при провале"),
    ];

    for (row_index, task) in json_rows(xdt.pointer("/m_pMissionTable/m_pMissionData")) {
        let task_id = task
            .get("m_iHTaskID")
            .and_then(json_usize)
            .map(|value| value.to_string())
            .unwrap_or_else(|| row_index.clone());
        let mission_id = task
            .get("m_iHMissionID")
            .and_then(json_usize)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "?".to_string());
        let row_label = format!("task {task_id}");

        for (field, label) in SCALAR_FIELDS {
            let Some(npc_id) = task.get(*field).and_then(json_usize) else {
                continue;
            };
            add_usage(
                usage,
                npc_id,
                UsageKind::Quest,
                usage_reference(
                    "xdt.json",
                    "m_pMissionTable.m_pMissionData",
                    field,
                    &row_label,
                    format!("Задание {task_id} (миссия {mission_id}): {label}"),
                ),
            );
        }

        if let Some(enemy_ids) = task.get("m_iCSUEnemyID").and_then(JsonValue::as_array) {
            for (index, value) in enemy_ids.iter().enumerate() {
                let Some(npc_id) = json_usize(value) else {
                    continue;
                };
                add_usage(
                    usage,
                    npc_id,
                    UsageKind::Quest,
                    usage_reference(
                        "xdt.json",
                        "m_pMissionTable.m_pMissionData",
                        &format!("m_iCSUEnemyID[{index}]"),
                        &row_label,
                        format!("Задание {task_id} (миссия {mission_id}): цель/противник #{index}"),
                    ),
                );
            }
        }
    }
}

fn scan_xdt_other_references(xdt: &JsonValue, usage: &mut BTreeMap<usize, UsageAccumulator>) {
    scan_xdt_reference_rows(
        xdt.pointer("/m_pVendorTable/m_pItemData"),
        "m_pVendorTable.m_pItemData",
        "m_iNpcNumber",
        Some("m_iSortNumber"),
        "Товар продавца",
        usage,
    );
    scan_xdt_reference_rows(
        xdt.pointer("/m_pInstanceTable/m_pWarpData"),
        "m_pInstanceTable.m_pWarpData",
        "m_iNpcNumber",
        Some("m_iWarpNumber"),
        "Точка телепортации",
        usage,
    );
    scan_xdt_reference_rows(
        xdt.pointer("/m_pTransportationTable/m_pBroomstickLocation"),
        "m_pTransportationTable.m_pBroomstickLocation",
        "m_iNPCID",
        Some("m_iLocationID"),
        "Остановка Broomstick",
        usage,
    );
    scan_xdt_reference_rows(
        xdt.pointer("/m_pTransportationTable/m_pTransportationWarpLocation"),
        "m_pTransportationTable.m_pTransportationWarpLocation",
        "m_iNPCID",
        Some("m_iLocationID"),
        "Транспортная точка",
        usage,
    );
    scan_xdt_reference_rows(
        xdt.pointer("/m_pTransportationTable/m_pTransportationData"),
        "m_pTransportationTable.m_pTransportationData",
        "m_iNPCID",
        Some("m_iVehicleID"),
        "Транспортный маршрут",
        usage,
    );

    for (group_index, group) in json_rows(xdt.pointer("/m_pNpcTable/m_pNpcGroupData")) {
        for field in ["iMember1", "iMember2", "iMember3", "iMember4", "iMember5"] {
            let Some(npc_id) = group.get(field).and_then(json_usize) else {
                continue;
            };
            add_usage(
                usage,
                npc_id,
                UsageKind::Other,
                usage_reference(
                    "xdt.json",
                    "m_pNpcTable.m_pNpcGroupData",
                    field,
                    &group_index,
                    format!("Участник NPC-группы #{group_index} ({field})"),
                ),
            );
        }
    }
}

fn scan_xdt_reference_rows(
    rows: Option<&JsonValue>,
    section: &str,
    field: &str,
    row_id_field: Option<&str>,
    label: &str,
    usage: &mut BTreeMap<usize, UsageAccumulator>,
) {
    for (row_index, row) in json_rows(rows) {
        let Some(npc_id) = row.get(field).and_then(json_usize) else {
            continue;
        };
        let row_id = row_id_field
            .and_then(|key| row.get(key))
            .and_then(json_usize)
            .map(|value| value.to_string())
            .unwrap_or(row_index);
        add_usage(
            usage,
            npc_id,
            UsageKind::Other,
            usage_reference(
                "xdt.json",
                section,
                field,
                &row_id,
                format!("{label} #{row_id} ({field}={npc_id})"),
            ),
        );
    }
}

fn scan_npc_paths(
    source_file: &str,
    paths: Option<&JsonValue>,
    runtime_npc_types: &BTreeMap<usize, usize>,
    usage: &mut BTreeMap<usize, UsageAccumulator>,
    unresolved_runtime_ids: &mut usize,
) {
    for (path_id, path) in json_rows(paths) {
        if let Some(types) = path.get("aNPCTypes").and_then(JsonValue::as_array) {
            for (index, value) in types.iter().enumerate() {
                let Some(npc_id) = json_usize(value) else {
                    continue;
                };
                add_usage(
                    usage,
                    npc_id,
                    UsageKind::Other,
                    usage_reference(
                        source_file,
                        "npc",
                        &format!("aNPCTypes[{index}]"),
                        &path_id,
                        format!("Маршрут NPC #{path_id} назначен типу NPC {npc_id}"),
                    ),
                );
            }
        }
        if let Some(runtime_ids) = path.get("aNPCIDs").and_then(JsonValue::as_array) {
            for (index, value) in runtime_ids.iter().enumerate() {
                let Some(runtime_id) = json_usize_allow_zero(value) else {
                    continue;
                };
                let Some(npc_id) = runtime_npc_types.get(&runtime_id).copied() else {
                    *unresolved_runtime_ids += 1;
                    continue;
                };
                add_usage(
                    usage,
                    npc_id,
                    UsageKind::Other,
                    usage_reference(
                        source_file,
                        "npc",
                        &format!("aNPCIDs[{index}]"),
                        &path_id,
                        format!(
                            "Маршрут NPC #{path_id}: runtime ID {runtime_id} соответствует типу {npc_id}"
                        ),
                    ),
                );
            }
        }
    }
}

fn scan_drop_references(root: &JsonValue, usage: &mut BTreeMap<usize, UsageAccumulator>) {
    for (row_index, row) in json_rows(root.get("Mobs")) {
        let Some(npc_id) = row.get("MobID").and_then(json_usize) else {
            continue;
        };
        let drop_id = row
            .get("MobDropID")
            .and_then(json_usize_allow_zero)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "?".to_string());
        add_usage(
            usage,
            npc_id,
            UsageKind::Other,
            usage_reference(
                "drops.json",
                "Mobs",
                "MobID",
                &row_index,
                format!("Таблица добычи MobDropID={drop_id} для типа NPC {npc_id}"),
            ),
        );
    }
}

fn add_usage(
    usage: &mut BTreeMap<usize, UsageAccumulator>,
    npc_id: usize,
    kind: UsageKind,
    reference: NpcUsageReference,
) {
    // Zero is used as an absent-reference sentinel throughout XDT and server JSON.
    if npc_id == 0 {
        return;
    }
    usage.entry(npc_id).or_default().push(kind, reference);
}

fn usage_reference(
    source_file: &str,
    section: &str,
    field: &str,
    row: &str,
    description: String,
) -> NpcUsageReference {
    NpcUsageReference {
        source_file: source_file.to_string(),
        section: section.to_string(),
        field: field.to_string(),
        row: row.to_string(),
        description,
    }
}

fn usage_record(
    npc_id: usize,
    server_catalog_present: bool,
    accumulator: Option<UsageAccumulator>,
) -> NpcUsageRecord {
    let accumulator = accumulator.unwrap_or_default();
    let on_map = accumulator.map_count != 0;
    let quest_referenced = accumulator.quest_count != 0;
    let other_referenced = accumulator.other_count != 0;
    let used = on_map || quest_referenced || other_referenced;
    let sample_descriptions = accumulator
        .map_samples
        .iter()
        .take(2)
        .chain(accumulator.quest_samples.iter().take(2))
        .chain(accumulator.other_samples.iter().take(2))
        .map(|reference| reference.description.clone())
        .collect();
    NpcUsageRecord {
        npc_id,
        server_catalog_present,
        used,
        unused: !used,
        on_map,
        quest_referenced,
        other_referenced,
        on_map_count: accumulator.map_count,
        quest_reference_count: accumulator.quest_count,
        other_reference_count: accumulator.other_count,
        map_samples: accumulator.map_samples,
        quest_samples: accumulator.quest_samples,
        other_samples: accumulator.other_samples,
        sample_descriptions,
    }
}

fn json_rows(value: Option<&JsonValue>) -> Vec<(String, &JsonValue)> {
    match value {
        Some(JsonValue::Array(rows)) => rows
            .iter()
            .enumerate()
            .map(|(index, row)| (index.to_string(), row))
            .collect(),
        Some(JsonValue::Object(rows)) => rows.iter().map(|(key, row)| (key.clone(), row)).collect(),
        _ => Vec::new(),
    }
}

fn json_usize(value: &JsonValue) -> Option<usize> {
    let value = json_usize_allow_zero(value)?;
    (value != 0).then_some(value)
}

fn json_usize_allow_zero(value: &JsonValue) -> Option<usize> {
    if let Some(value) = value.as_u64() {
        return usize::try_from(value).ok();
    }
    if let Some(value) = value.as_i64() {
        return usize::try_from(value).ok();
    }
    if let Some(value) = value.as_f64() {
        if value.is_finite() && value >= 0.0 && value.fract() == 0.0 {
            return usize::try_from(value as u64).ok();
        }
    }
    value
        .as_str()
        .map(str::trim)
        .and_then(|value| value.parse::<usize>().ok())
}

fn spawn_location_description(row: &JsonValue) -> String {
    let x = display_json_value(row.get("iX"));
    let y = display_json_value(row.get("iY"));
    let z = display_json_value(row.get("iZ"));
    let map = row
        .get("iMapNum")
        .map(|value| display_json_value(Some(value)))
        .unwrap_or_else(|| "0".to_string());
    format!("карта {map}, координаты ({x}, {y}, {z})")
}

fn display_json_value(value: Option<&JsonValue>) -> String {
    match value {
        Some(JsonValue::Number(value)) => value.to_string(),
        Some(JsonValue::String(value)) => value.clone(),
        Some(value) => value.to_string(),
        None => "?".to_string(),
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests;
