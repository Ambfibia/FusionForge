use super::*;

impl AvatarAssetCatalog {
    /// Opens legacy evidence for offline validation and content resolution.
    ///
    /// This deliberately lives in `ffone-asset-pipeline`; the game runtime does
    /// not compile this filesystem- and provenance-heavy path.


    #[allow(clippy::too_many_arguments)]
    pub(super) fn resolve_clothing(
        &self,
        part: AvatarPartKind,
        participation: AvatarPartParticipation,
        table_name: &str,
        semantic_category: &str,
        expected_type: i16,
        item: AvatarItemSelection,
        gender: AvatarGender,
    ) -> AvatarPartResolution {
        if item.item_id != 0 {
            return self.resolve_item(
                part,
                participation,
                table_name,
                semantic_category,
                expected_type,
                item,
                gender,
                true,
            );
        }
        let (model_name, texture_name) = match (part, gender) {
            (AvatarPartKind::UpperBody, AvatarGender::Male) => ("m_shirt_naked", "m_naked"),
            (AvatarPartKind::UpperBody, AvatarGender::Female) => ("f_shirt_naked", "f_naked"),
            (AvatarPartKind::LowerBody, AvatarGender::Male) => ("m_pants_naked", "m_naked"),
            (AvatarPartKind::LowerBody, AvatarGender::Female) => ("f_pants_naked", "f_naked"),
            (AvatarPartKind::Foot, AvatarGender::Male) => ("m_shoes_naked", "m_naked"),
            (AvatarPartKind::Foot, AvatarGender::Female) => ("f_shoes_naked", "f_naked"),
            _ => {
                return unresolved_style_part(
                    part,
                    "player_clothing",
                    "internal naked-clothing policy mismatch".to_owned(),
                );
            }
        };
        let model_route = make_route("wear", model_name, "nif").unwrap();
        let texture_route = make_route("texture", texture_name, "dds").unwrap();
        let semantic_name = model_name.to_owned();
        let model = self.resolve_model(
            None,
            &model_route,
            &semantic_model_path(semantic_category, &semantic_name),
        );
        let textures = vec![
            self.resolve_texture(
                None,
                &texture_route,
                &semantic_texture_path(semantic_category, &semantic_name, texture_name),
            ),
            self.resolve_texture(
                self.player_entities
                    .get(&format!("player:{}:base", gender.token())),
                gender.skin_route(),
                &semantic_texture_path(
                    semantic_category,
                    &semantic_name,
                    source_stem(gender.skin_route()).unwrap_or("skin"),
                ),
            ),
        ];
        AvatarPartResolution {
            part,
            semantic_name,
            participation,
            disposition: disposition_for_assets(Some(&model), &textures),
            table: None,
            model: Some(model),
            textures,
            blockers: vec![
                "item id 0 uses the authored naked-clothing route; it is not an empty visual"
                    .to_owned(),
            ],
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn resolve_item(
        &self,
        part: AvatarPartKind,
        participation: AvatarPartParticipation,
        table_name: &str,
        semantic_category: &str,
        expected_type: i16,
        item: AvatarItemSelection,
        gender: AvatarGender,
        required_when_zero: bool,
    ) -> AvatarPartResolution {
        if item.item_id == 0 && !required_when_zero {
            return AvatarPartResolution {
                part,
                semantic_name: format!("{semantic_category}_none"),
                participation,
                disposition: AvatarPartDisposition::Empty,
                table: None,
                model: None,
                textures: Vec::new(),
                blockers: Vec::new(),
            };
        }
        if item.item_id < 0 {
            return unresolved_style_part(
                part,
                semantic_category,
                format!("negative item id {} is invalid", item.item_id),
            );
        }
        if item.item_id > 0 && item.item_type != expected_type {
            return unresolved_style_part(
                part,
                semantic_category,
                format!(
                    "protocol item type {} does not match slot/table type {expected_type}",
                    item.item_type
                ),
            );
        }
        let row = match self.item_row(table_name, i64::from(item.item_id)) {
            Ok(row) => row,
            Err(detail) => return unresolved_style_part(part, semantic_category, detail),
        };
        let mesh = match self.mesh_row(&row) {
            Ok(mesh) => mesh,
            Err(detail) => return unresolved_style_part(part, semantic_category, detail),
        };
        let model_name = match gender_string(&mesh, gender, "MeshModelString") {
            Some(value) => value,
            None => {
                return unresolved_style_part(
                    part,
                    semantic_category,
                    "item mesh row has no gender-specific model name".to_owned(),
                );
            }
        };
        let model_route = make_route("wear", model_name, "nif").unwrap();
        let mut textures = Vec::new();
        for suffix in ["TextureString", "TextureString2"] {
            if let Some(texture) = gender_string(&mesh, gender, suffix)
                && let Some(route) = make_route("texture", texture, "dds")
            {
                textures.push(route);
            }
        }
        self.part_from_row(part, participation, row, model_route, textures)
    }

    pub(super) fn part_from_row(
        &self,
        part: AvatarPartKind,
        participation: AvatarPartParticipation,
        row: ResolvedItemRow,
        model_route: String,
        texture_routes: Vec<String>,
    ) -> AvatarPartResolution {
        let semantic_name = source_stem(&model_route).unwrap_or("unresolved").to_owned();
        let category = semantic_category(part);
        let entity = self
            .entities
            .get(&(row.table_name.to_owned(), row.row_index));
        let model = self.resolve_model(
            entity,
            &model_route,
            &semantic_model_path(category, &semantic_name),
        );
        let textures = texture_routes
            .iter()
            .map(|route| {
                self.resolve_texture(
                    entity,
                    route,
                    &semantic_texture_path(
                        category,
                        &semantic_name,
                        source_stem(route).unwrap_or("texture"),
                    ),
                )
            })
            .collect::<Vec<_>>();
        AvatarPartResolution {
            part,
            semantic_name,
            participation,
            disposition: disposition_for_assets(Some(&model), &textures),
            table: Some(AvatarTableSelection {
                table: format!("{}.m_pItemData", row.table_name),
                row_index: row.row_index,
                item_number: row.item_number,
                mesh_index: row.mesh_index,
                equip_type: row.equip_type,
            }),
            model: Some(model),
            textures,
            blockers: Vec::new(),
        }
    }

    pub(super) fn resolve_model(
        &self,
        entity: Option<&PlanEntity>,
        source_route: &str,
        semantic_path: &str,
    ) -> NativeAvatarAssetResolution {
        let normalized = normalize_route(source_route);
        let matching = entity
            .into_iter()
            .flat_map(|entity| &entity.model_proposal_ids)
            .filter_map(|id| self.models.get(id))
            .filter(|model| normalize_route(&model.legacy_route) == normalized)
            .collect::<Vec<_>>();
        if let Some(entity) = entity
            && !entity.blocker_ids.is_empty()
            && !matching.is_empty()
        {
            return NativeAvatarAssetResolution {
                kind: NativeAvatarAssetKind::ModelGlb,
                source_route: normalized,
                semantic_path: semantic_path.to_owned(),
                status: NativeResolutionStatus::BlockedProvenance,
                candidates: Vec::new(),
                blockers: vec![format!(
                    "semantic entity {} still has {} ownership blockers",
                    entity.id,
                    entity.blocker_ids.len()
                )],
            };
        }
        if matching.len() != 1 {
            let detail = if matching.is_empty() {
                format!(
                    "no ownership-proven logical-root GLB for {normalized}; flat mesh GLBs are deliberately rejected because they do not prove bones, skinning, hierarchy, or animations"
                )
            } else {
                format!(
                    "{} ownership proposals match {normalized}; refusing an ambiguous model",
                    matching.len()
                )
            };
            return NativeAvatarAssetResolution {
                kind: NativeAvatarAssetKind::ModelGlb,
                source_route: normalized,
                semantic_path: semantic_path.to_owned(),
                status: if matching.is_empty() {
                    NativeResolutionStatus::Missing
                } else {
                    NativeResolutionStatus::Ambiguous
                },
                candidates: Vec::new(),
                blockers: vec![detail],
            };
        }
        let model = matching[0];
        if !model.eligible || !model.blocker_ids.is_empty() {
            return NativeAvatarAssetResolution {
                kind: NativeAvatarAssetKind::ModelGlb,
                source_route: normalized,
                semantic_path: semantic_path.to_owned(),
                status: NativeResolutionStatus::Missing,
                candidates: Vec::new(),
                blockers: vec![format!(
                    "semantic model proposal {} is not publishable (eligible={}, blockers={})",
                    model.id,
                    model.eligible,
                    model.blocker_ids.len()
                )],
            };
        }
        let candidate =
            match self.verify_candidate(&model.output_glb, ProjectAssetKind::Model, None) {
                Ok(candidate) => candidate,
                Err(detail) => {
                    return NativeAvatarAssetResolution {
                        kind: NativeAvatarAssetKind::ModelGlb,
                        source_route: normalized,
                        semantic_path: semantic_path.to_owned(),
                        status: NativeResolutionStatus::Missing,
                        candidates: Vec::new(),
                        blockers: vec![detail],
                    };
                }
            };
        NativeAvatarAssetResolution {
            kind: NativeAvatarAssetKind::ModelGlb,
            source_route: normalized,
            semantic_path: semantic_path.to_owned(),
            status: if self.provenance.trusted {
                NativeResolutionStatus::Verified
            } else {
                NativeResolutionStatus::BlockedProvenance
            },
            candidates: vec![candidate],
            blockers: if self.provenance.trusted {
                Vec::new()
            } else {
                self.provenance.issues.clone()
            },
        }
    }

    pub(super) fn resolve_texture(
        &self,
        entity: Option<&PlanEntity>,
        source_route: &str,
        semantic_path: &str,
    ) -> NativeAvatarAssetResolution {
        let normalized = normalize_route(source_route);
        let mut dependencies = entity
            .into_iter()
            .flat_map(|entity| &entity.dependencies)
            .filter(|dependency| {
                dependency.kind == "texture"
                    && normalize_route(&dependency.legacy_route) == normalized
            })
            .cloned()
            .collect::<Vec<_>>();
        if dependencies.is_empty()
            && let Some(global) = self.dependencies_by_route.get(&normalized)
        {
            dependencies.extend(
                global
                    .iter()
                    .filter(|dependency| dependency.kind == "texture")
                    .cloned(),
            );
        }
        dependencies.sort_by(|left, right| {
            left.resolution
                .cmp(&right.resolution)
                .then_with(|| left.ownership.cmp(&right.ownership))
        });
        dependencies.dedup();

        let mut plan_candidates = dependencies
            .iter()
            .flat_map(|dependency| dependency.candidates.iter().cloned())
            .collect::<Vec<_>>();
        plan_candidates.sort();
        plan_candidates.dedup();

        if plan_candidates.is_empty()
            && let Some(name) = source_stem(&normalized)
            && let Some(content) = self
                .content_by_kind_name
                .get(&("texture".to_owned(), name.to_owned()))
        {
            plan_candidates.extend(content.iter().map(|asset| PlanCandidate {
                native_key: Some(asset.key.clone()),
                project_path: Some(asset.path.clone()),
            }));
        }

        let mut verified = Vec::new();
        let mut blockers = Vec::new();
        for candidate in plan_candidates {
            let Some(project_path) = candidate.project_path else {
                blockers.push("native texture candidate has no project path".to_owned());
                continue;
            };
            match self.verify_candidate(
                &project_path,
                ProjectAssetKind::Texture,
                candidate.native_key,
            ) {
                Ok(candidate) => verified.push(candidate),
                Err(detail) => blockers.push(detail),
            }
        }
        verified.sort_by(|left, right| left.project_path.cmp(&right.project_path));
        verified.dedup_by(|left, right| left.project_path == right.project_path);

        let exact = !dependencies.is_empty()
            && dependencies.iter().all(|dependency| {
                dependency.resolution == "exact"
                    && matches!(dependency.ownership.as_str(), "entity" | "shared")
            })
            && entity.is_none_or(|entity| entity.blocker_ids.is_empty())
            && verified.len() == 1;
        let status = if exact && self.provenance.trusted {
            NativeResolutionStatus::Verified
        } else if exact {
            blockers.extend(self.provenance.issues.clone());
            NativeResolutionStatus::BlockedProvenance
        } else if verified.len() == 1 {
            blockers.push(format!(
                "{normalized} has a hash-verified PNG candidate, but route ownership is not proven"
            ));
            NativeResolutionStatus::HashVerifiedCandidateUnproven
        } else if verified.len() > 1 {
            blockers.push(format!(
                "{normalized} maps to {} native PNG candidates",
                verified.len()
            ));
            NativeResolutionStatus::Ambiguous
        } else {
            blockers.push(format!("no native PNG candidate for {normalized}"));
            NativeResolutionStatus::Missing
        };
        NativeAvatarAssetResolution {
            kind: NativeAvatarAssetKind::TexturePng,
            source_route: normalized,
            semantic_path: semantic_path.to_owned(),
            status,
            candidates: verified,
            blockers,
        }
    }

    pub(super) fn verify_candidate(
        &self,
        project_path: &str,
        expected_kind: ProjectAssetKind,
        native_key: Option<String>,
    ) -> Result<VerifiedNativeCandidate, String> {
        let normalized = normalize_project_path(project_path)?;
        let entry = self
            .manifest
            .get(&normalized)
            .ok_or_else(|| format!("native candidate {normalized:?} is not manifest-listed"))?;
        if entry.kind != expected_kind {
            return Err(format!(
                "native candidate {normalized:?} has manifest kind {:?}, expected {:?}",
                entry.kind, expected_kind
            ));
        }
        read_verified_manifest_file(&self.asset_root, entry).map_err(|error| error.to_string())?;
        Ok(VerifiedNativeCandidate {
            project_path: entry.path.clone(),
            source_path: entry.source_path.clone(),
            bytes: entry.bytes,
            blake3: entry.blake3.clone(),
            native_key,
        })
    }

    pub(super) fn item_row(&self, table_name: &str, item_number: i64) -> Result<ResolvedItemRow, String> {
        let table = self
            .tables
            .get(table_name)
            .and_then(Value::as_object)
            .ok_or_else(|| format!("missing TableData object {table_name}"))?;
        let rows = table
            .get("m_pItemData")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{table_name}.m_pItemData is not an array"))?;
        let matches = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| int_field(row, "m_iItemNumber") == Some(item_number))
            .collect::<Vec<_>>();
        let [(row_index, row)] = matches.as_slice() else {
            return Err(if matches.is_empty() {
                format!("{table_name} has no item number {item_number}")
            } else {
                format!(
                    "{table_name} has {} rows for item number {item_number}",
                    matches.len()
                )
            });
        };
        let mesh_index = int_field(row, "m_iMesh")
            .and_then(|value| usize::try_from(value).ok())
            .filter(|value| *value > 0)
            .ok_or_else(|| {
                format!("{table_name}.m_pItemData[{row_index}] has no positive m_iMesh")
            })?;
        Ok(ResolvedItemRow {
            table_name: table_name.to_owned(),
            row_index: *row_index,
            item_number,
            mesh_index,
            equip_type: int_field(row, "m_iEquipType").unwrap_or_default(),
        })
    }

    pub(super) fn mesh_row(&self, row: &ResolvedItemRow) -> Result<Value, String> {
        self.tables
            .get(&row.table_name)
            .and_then(|table| table.get("m_pItemMeshData"))
            .and_then(Value::as_array)
            .and_then(|meshes| meshes.get(row.mesh_index))
            .cloned()
            .ok_or_else(|| {
                format!(
                    "{}.m_pItemMeshData has no row {}",
                    row.table_name, row.mesh_index
                )
            })
    }

    pub(super) fn hat_policy(&self, item: AvatarItemSelection) -> Result<HatPolicy, String> {
        if item.item_id == 0 {
            return Ok(HatPolicy::default());
        }
        if item.item_type != 4 {
            return Err(format!(
                "hat slot contains item type {}, expected 4; face/hair compatibility is unknown",
                item.item_type
            ));
        }
        let row = self.item_row("m_pHatItemTable", i64::from(item.item_id))?;
        match row.equip_type {
            0 => Ok(HatPolicy::default()),
            1 => Ok(HatPolicy {
                face_variant: 1,
                hair_variant: Some(2),
                glasses_visible: true,
            }),
            2 => Ok(HatPolicy {
                face_variant: 2,
                hair_variant: None,
                glasses_visible: true,
            }),
            3 => Ok(HatPolicy {
                face_variant: 2,
                hair_variant: None,
                glasses_visible: false,
            }),
            4 => Ok(HatPolicy {
                face_variant: 1,
                hair_variant: Some(1),
                glasses_visible: false,
            }),
            5 => Ok(HatPolicy {
                face_variant: 1,
                hair_variant: Some(2),
                glasses_visible: false,
            }),
            value => Err(format!(
                "hat item {} has unsupported m_iEquipType {value}; face/hair compatibility is unknown",
                item.item_id
            )),
        }
    }

}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ContentIndex {
    pub(super) schema: String,
    pub(super) native_only: bool,
    pub(super) assets: Vec<ContentAsset>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct ContentAsset {
    pub(super) key: String,
    pub(super) kind: String,
    pub(super) name: String,
    pub(super) path: String,
}

pub(super) fn index_manifest(
    files: Vec<ProjectAssetFile>,
) -> Result<BTreeMap<String, ProjectAssetFile>, AvatarCatalogError> {
    let mut output = BTreeMap::new();
    let mut folded = BTreeSet::new();
    for file in files {
        let path = normalize_project_path(&file.path).map_err(AvatarCatalogError::Invalid)?;
        if path != file.path {
            return Err(AvatarCatalogError::Invalid(format!(
                "manifest path {:?} is not canonical",
                file.path
            )));
        }
        if !folded.insert(path.to_ascii_lowercase()) {
            return Err(AvatarCatalogError::Invalid(format!(
                "case-insensitive duplicate manifest path {path:?}"
            )));
        }
        output.insert(path, file);
    }
    Ok(output)
}

pub(super) fn normalize_project_path(path: &str) -> Result<String, String> {
    if path.is_empty() || path.contains('\\') {
        return Err(format!("unsafe native project path {path:?}"));
    }
    let parsed = Path::new(path);
    if parsed.is_absolute()
        || parsed
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("unsafe native project path {path:?}"));
    }
    Ok(path.to_owned())
}

pub(super) fn project_relative_path(root: &Path, path: &Path) -> Result<String, AvatarCatalogError> {
    let relative = path.strip_prefix(root).map_err(|_| {
        AvatarCatalogError::Invalid(format!(
            "{} is outside project root {}",
            path.display(),
            root.display()
        ))
    })?;
    let components = relative
        .components()
        .map(|component| match component {
            Component::Normal(value) => value.to_str().map(str::to_owned).ok_or_else(|| {
                AvatarCatalogError::Invalid(format!("non-UTF8 project path {}", path.display()))
            }),
            _ => Err(AvatarCatalogError::Invalid(format!(
                "unsafe project path {}",
                path.display()
            ))),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(components.join("/"))
}

pub(super) fn read_verified_manifest_file(
    root: &Path,
    entry: &ProjectAssetFile,
) -> Result<Vec<u8>, AvatarCatalogError> {
    let relative = normalize_project_path(&entry.path).map_err(AvatarCatalogError::Invalid)?;
    let path = relative
        .split('/')
        .fold(root.to_path_buf(), |path, component| path.join(component));
    let bytes = read_file(&path)?;
    if bytes.len() as u64 != entry.bytes {
        return Err(AvatarCatalogError::Invalid(format!(
            "native asset length mismatch for {}: manifest={}, disk={}",
            path.display(),
            entry.bytes,
            bytes.len()
        )));
    }
    let actual = blake3::hash(&bytes).to_hex().to_string();
    if actual != entry.blake3 {
        return Err(AvatarCatalogError::Invalid(format!(
            "native asset BLAKE3 mismatch for {}: manifest={}, disk={actual}",
            path.display(),
            entry.blake3
        )));
    }
    Ok(bytes)
}

pub(super) fn resolve_evidence_path(
    evidence_root: &Path,
    workspace_root: &Path,
    relative: &str,
) -> Result<PathBuf, AvatarCatalogError> {
    if relative.is_empty() || relative.contains('\\') || Path::new(relative).is_absolute() {
        return Err(AvatarCatalogError::Invalid(format!(
            "unsafe evidence path {relative:?}"
        )));
    }
    let joined = relative
        .split('/')
        .fold(evidence_root.to_path_buf(), |path, component| {
            path.join(component)
        });
    let canonical = canonical_file(&joined)?;
    if !canonical.starts_with(workspace_root) {
        return Err(AvatarCatalogError::Invalid(format!(
            "evidence path {} escapes workspace {}",
            canonical.display(),
            workspace_root.display()
        )));
    }
    Ok(canonical)
}

pub(super) fn make_route(prefix: &str, value: &str, extension: &str) -> Option<String> {
    let clean = value.trim().trim_matches('"').trim();
    if clean.is_empty() || clean.eq_ignore_ascii_case("null") || clean.eq_ignore_ascii_case("none")
    {
        return None;
    }
    let prefix = normalize_route(prefix).trim_matches('/').to_owned();
    let mut route = normalize_route(clean);
    let expected = extension.trim_start_matches('.').to_ascii_lowercase();
    let current = route
        .rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, extension)| extension.to_ascii_lowercase());
    match current.as_deref() {
        Some(current) if current == expected => {}
        Some(current) if matches!(current, "nif" | "kfm" | "dds" | "wav") => {
            route.truncate(route.len().saturating_sub(current.len()));
            route.push_str(&expected);
        }
        Some(_) => {}
        None => {
            route.push('.');
            route.push_str(&expected);
        }
    }
    if route != prefix && !route.starts_with(&format!("{prefix}/")) {
        route = format!("{prefix}/{}", route.trim_start_matches('/'));
    }
    Some(route)
}
