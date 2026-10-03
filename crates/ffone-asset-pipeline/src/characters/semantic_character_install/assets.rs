use super::*;

pub const SEMANTIC_CHARACTER_REGISTRY_SCHEMA: &str = "ffone.semantic-character-registry.v2";

pub const SEMANTIC_CHARACTER_REGISTRY_PATH: &str = "_runtime/characters.json";

// These named characters are authored and maintained as NPC content even though
// the legacy tables also reuse their mesh routes for hostile/HNPC encounters.
// Keeping the override on the exact KFM route makes the runtime taxonomy stable
// without hiding the complete set of observed legacy roles from the report.
pub(super) const NPC_AUTHORING_ROUTE_OVERRIDES: &[&str] = &["mob/npc_deedee.kfm", "mob/npc_dexter.kfm"];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterRegistryProofs {
    pub batch_report_sha256: String,
    pub gpu_audit_sha256: String,
    pub table_set_sha256: String,
    pub structural_audit_schema: String,
    pub gpu_audit_schema: String,
    pub exact_route_policy: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticCharacterRegistry {
    pub schema: String,
    pub models: Vec<RuntimeCharacterModel>,
}

pub(super) fn exact_character_route_evidence(
    table_set: &Value,
) -> Result<BTreeMap<String, Vec<TableReference>>> {
    if string(table_set, "schema") != Some(TABLE_SET_SCHEMA) {
        return invalid("character classification table-set has the wrong schema");
    }
    let mut matches = value_array(table_set.get("tables"))
        .iter()
        .filter(|table| string(table, "name") == Some(CONSOLIDATED_TABLE));
    let table = matches
        .next()
        .ok_or_else(|| invalid_error("table-set has no npc_imports_consolidated table"))?;
    if matches.next().is_some() {
        return invalid("table-set has duplicate npc_imports_consolidated tables");
    }
    let root = table
        .get("value")
        .ok_or_else(|| invalid_error("consolidated table has no value"))?;
    let mut routes = BTreeMap::<String, Vec<TableReference>>::new();

    let npc_table = root.get("m_pNpcTable").unwrap_or(&Value::Null);
    let npc_rows = value_array(npc_table.get("m_pNpcData"));
    let npc_meshes = value_array(npc_table.get("m_pNpcMeshData"));
    for (row_index, row) in npc_rows.iter().enumerate() {
        let Some(mesh_index) = positive_index(row, "m_iMesh") else {
            continue;
        };
        let Some(mesh) = npc_meshes.get(mesh_index) else {
            continue;
        };
        let Some(route) = route_from_model_field(mesh, "m_pstrMMeshModelString", "mob") else {
            continue;
        };
        let role = if int_field(row, "m_iHNpc").unwrap_or_default() != 0 {
            TableRole::Hnpc
        } else if int_field(row, "m_iTeam") == Some(2) {
            TableRole::Mob
        } else {
            TableRole::Npc
        };
        routes.entry(route).or_default().push(TableReference {
            role,
            row_index,
            entity_number: int_field(row, "m_iNpcNumber"),
            mesh_index,
        });
    }

    let nano_table = root.get("m_pNanoTable").unwrap_or(&Value::Null);
    let nano_rows = value_array(nano_table.get("m_pNanoData"));
    let nano_meshes = value_array(nano_table.get("m_pNanoMeshData"));
    for (row_index, row) in nano_rows.iter().enumerate() {
        let Some(mesh_index) = positive_index(row, "m_iMesh") else {
            continue;
        };
        let Some(mesh) = nano_meshes.get(mesh_index) else {
            continue;
        };
        let Some(route) = route_from_model_field(mesh, "m_pstrMMeshModelString", "nano") else {
            continue;
        };
        routes.entry(route).or_default().push(TableReference {
            role: TableRole::Nano,
            row_index,
            entity_number: int_field(row, "m_iNanoNumber"),
            mesh_index,
        });
    }
    for references in routes.values_mut() {
        references.sort_by_key(|reference| {
            (
                reference.role,
                reference.row_index,
                reference.entity_number,
                reference.mesh_index,
            )
        });
    }
    Ok(routes)
}

pub(super) fn classify_route(
    legacy_route: &str,
    family: &str,
    references: Vec<TableReference>,
) -> Classification {
    if family == "nano" {
        return Classification {
            category: Some(RuntimeCharacterCategory::Nano),
            method: if references.is_empty() {
                "exact-nano-kfm-route-namespace"
            } else {
                "exact-nano-table-route"
            },
            references,
        };
    }
    if legacy_route.starts_with("mob/fusion_") && legacy_route.ends_with(".kfm") {
        return Classification {
            category: Some(RuntimeCharacterCategory::Fusion),
            method: "exact-fusion-kfm-route-namespace",
            references,
        };
    }
    let roles = references
        .iter()
        .map(|reference| reference.role)
        .collect::<BTreeSet<_>>();
    let authoring_npc_override = NPC_AUTHORING_ROUTE_OVERRIDES.contains(&legacy_route);
    let authoring_npc_namespace = legacy_route.ends_with(".kfm")
        && (legacy_route.starts_with("mob/npc_") || legacy_route.starts_with("mob/t_"));
    let category = if authoring_npc_override {
        Some(RuntimeCharacterCategory::Npc)
    } else if roles.len() == 1 && roles.contains(&TableRole::Npc) {
        Some(RuntimeCharacterCategory::Npc)
    } else if roles.len() == 1 && roles.contains(&TableRole::Mob) {
        Some(RuntimeCharacterCategory::Mob)
    } else if authoring_npc_namespace
        && (roles.is_empty() || (roles.len() == 1 && roles.contains(&TableRole::Hnpc)))
    {
        Some(RuntimeCharacterCategory::Npc)
    } else if roles.is_empty()
        || (roles.len() == 1
            && (roles.contains(&TableRole::Hnpc) || roles.contains(&TableRole::Nano)))
    {
        None
    } else if !roles.contains(&TableRole::Nano) {
        Some(RuntimeCharacterCategory::Shared)
    } else {
        None
    };
    Classification {
        category,
        method: if authoring_npc_override {
            "exact-authoring-npc-route-override"
        } else if category == Some(RuntimeCharacterCategory::Npc)
            && authoring_npc_namespace
            && (roles.is_empty() || (roles.len() == 1 && roles.contains(&TableRole::Hnpc)))
        {
            "exact-npc-kfm-route-namespace"
        } else {
            match category {
                Some(RuntimeCharacterCategory::Npc) => "exact-npc-table-route-team-not-2",
                Some(RuntimeCharacterCategory::Mob) => "exact-npc-table-route-team-2",
                Some(RuntimeCharacterCategory::Fusion) => "exact-fusion-kfm-route-namespace",
                Some(RuntimeCharacterCategory::Shared) => "exact-npc-table-route-multiple-roles",
                Some(RuntimeCharacterCategory::Nano) => "exact-nano-table-route",
                None => "unresolved-exact-route",
            }
        },
        references,
    }
}

pub(super) fn exact_route_from_mapping(mapping: &Value) -> Result<(String, String)> {
    let family = required_string(mapping, "family")?;
    if !matches!(family, "mob" | "nano") {
        return invalid(format!("unsupported character candidate family {family:?}"));
    }
    let directories = string_array(mapping, "semanticDirectories")?;
    let semantic_path = validate_semantic_directories(&directories)?;
    let logical_name = required_string(mapping, "logicalName")?;
    validate_file_name(logical_name)?;
    let expected_source = format!("{family}/{semantic_path}/{logical_name}.source.json");
    if required_string(mapping, "source")? != expected_source {
        return invalid(format!(
            "candidate source path does not prove its exact route: expected {expected_source:?}"
        ));
    }
    let expected_glb = format!("models/{family}/{semantic_path}/{logical_name}.glb");
    if required_string(mapping, "outputGlb")? != expected_glb {
        return invalid(format!(
            "candidate GLB path does not prove its exact route: expected {expected_glb:?}"
        ));
    }
    Ok((
        normalize_route(&format!("{family}/{semantic_path}.kfm")),
        semantic_path,
    ))
}

pub(super) fn exact_route_from_source_blocker(blocker: &Value) -> Result<(String, String)> {
    let family = required_string(blocker, "family")?;
    let directories = string_array(blocker, "semanticDirectories")?;
    let semantic_path = validate_semantic_directories(&directories)?;
    let logical_name = required_string(blocker, "logicalName")?;
    validate_file_name(logical_name)?;
    let expected_source = format!("{family}/{semantic_path}/{logical_name}.source.json");
    if required_string(blocker, "source")? != expected_source {
        return invalid("blocked source path does not prove its exact route");
    }
    Ok((
        normalize_route(&format!("{family}/{semantic_path}.kfm")),
        semantic_path,
    ))
}

pub(super) fn ensure_unique_registry_models(models: &[RuntimeCharacterModel]) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut glbs = BTreeSet::new();
    let mut names = BTreeMap::<String, String>::new();
    for model in models {
        validate_relative(&model.id)?;
        validate_relative(&model.glb)?;
        if !model
            .glb
            .starts_with(&format!("{}/", model.category.directory()))
        {
            return invalid(format!(
                "registry model {:?} is outside its category directory",
                model.id
            ));
        }
        if !ids.insert(model.id.to_ascii_lowercase()) {
            return invalid(format!(
                "case-insensitive semantic character registry ID collision: {:?}",
                model.id
            ));
        }
        if !glbs.insert(model.glb.to_ascii_lowercase()) {
            return invalid(format!(
                "case-insensitive semantic character registry GLB collision: {:?}",
                model.glb
            ));
        }
        let (_, id_stem) = model.id.split_once('/').ok_or_else(|| {
            invalid_error("semantic character registry ID has no category prefix")
        })?;
        for name in std::iter::once(id_stem).chain(model.legacy_aliases.iter().map(String::as_str))
        {
            validate_file_name(name)?;
            let key = name.to_ascii_lowercase();
            if let Some(previous) = names.insert(key, model.glb.clone())
                && previous != model.glb
            {
                return invalid(format!(
                    "case-insensitive semantic character name/alias collision at {name:?}: {previous:?} versus {:?}",
                    model.glb
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn is_managed_path(path: &str) -> bool {
    MANAGED_TARGETS
        .iter()
        .any(|target| path == *target || path.starts_with(&format!("{target}/")))
}

pub(super) fn positive_index(value: &Value, field: &str) -> Option<usize> {
    int_field(value, field)
        .filter(|value| *value > 0)
        .and_then(|value| usize::try_from(value).ok())
}

pub(super) fn sibling_transaction_path(path: &Path, token: &str, suffix: &str) -> Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("transaction output has no parent"))?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid_error("transaction output filename is not UTF-8"))?;
    Ok(parent.join(format!(".{name}.semantic-characters-{token}.{suffix}")))
}

pub(super) fn relative_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| invalid_error("path escaped its declared root"))?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) => parts.push(
                value
                    .to_str()
                    .ok_or_else(|| invalid_error("relative path is not UTF-8"))?,
            ),
            _ => return invalid("relative path contains a non-normal component"),
        }
    }
    Ok(parts.join("/"))
}

pub(super) fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
