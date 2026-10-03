use super::*;

pub(super) const CONTENT_INDEX_SCHEMA: &str = "ffone.asset-index.v1";

/// Files and evidence roots required by the current offline ownership report.
#[derive(Clone, Debug)]
pub struct AvatarCatalogOptions {
    pub asset_root: PathBuf,
    pub evidence_root: PathBuf,
    pub semantic_plan_path: PathBuf,
    /// A trusted build-time pin for the semantic report itself.
    ///
    /// The report contains hashes of its inputs, but that is not enough to
    /// authenticate the report. If this pin is absent or wrong, resolution
    /// remains report-only and no asset can receive `verified` status.
    pub expected_semantic_plan_blake3: Option<String>,
}

impl AvatarCatalogOptions {
    pub fn new(
        asset_root: impl Into<PathBuf>,
        evidence_root: impl Into<PathBuf>,
        semantic_plan_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            asset_root: asset_root.into(),
            evidence_root: evidence_root.into(),
            semantic_plan_path: semantic_plan_path.into(),
            expected_semantic_plan_blake3: None,
        }
    }

    pub fn with_expected_semantic_plan_blake3(mut self, hash: impl Into<String>) -> Self {
        self.expected_semantic_plan_blake3 = Some(hash.into());
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeAvatarAssetKind {
    ModelGlb,
    TexturePng,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeAvatarAssetResolution {
    pub kind: NativeAvatarAssetKind,
    pub source_route: String,
    /// Readable destination required from the future ownership-preserving cooker.
    pub semantic_path: String,
    pub status: NativeResolutionStatus,
    pub candidates: Vec<VerifiedNativeCandidate>,
    pub blockers: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvatarCatalogProvenance {
    pub asset_manifest_path: String,
    pub asset_manifest_blake3: String,
    pub table_set_path: String,
    pub table_set_blake3: String,
    pub semantic_plan_path: String,
    pub semantic_plan_blake3: String,
    pub trusted: bool,
    pub issues: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct AvatarAssetCatalog {
    pub(super) asset_root: PathBuf,
    pub(super) provenance: AvatarCatalogProvenance,
    pub(super) manifest: BTreeMap<String, ProjectAssetFile>,
    pub(super) tables: Value,
    pub(super) entities: BTreeMap<(String, usize), PlanEntity>,
    pub(super) player_entities: BTreeMap<String, PlanEntity>,
    pub(super) models: BTreeMap<String, PlanModel>,
    pub(super) dependencies_by_route: BTreeMap<String, Vec<PlanDependency>>,
    pub(super) content_by_kind_name: BTreeMap<(String, String), Vec<ContentAsset>>,
}

impl AvatarAssetCatalog {
    /// Opens legacy evidence for offline validation and content resolution.
    ///
    /// This deliberately lives in `ffone-asset-pipeline`; the game runtime does
    /// not compile this filesystem- and provenance-heavy path.
    pub fn open(options: &AvatarCatalogOptions) -> Result<Self, AvatarCatalogError> {
        let asset_root = canonical_directory(&options.asset_root)?;
        let evidence_root = canonical_directory(&options.evidence_root)?;
        let workspace_root = evidence_root
            .parent()
            .ok_or_else(|| {
                AvatarCatalogError::Invalid(format!(
                    "evidence root {} has no workspace parent",
                    evidence_root.display()
                ))
            })?
            .to_path_buf();

        let manifest_path = asset_root.join(ASSET_MANIFEST_FILE);
        let manifest_bytes = read_file(&manifest_path)?;
        let manifest_hash = blake3::hash(&manifest_bytes).to_hex().to_string();
        let manifest_document: ProjectAssetManifest = parse_json(&manifest_path, &manifest_bytes)?;
        if manifest_document.schema != PROJECT_ASSET_SCHEMA {
            return Err(AvatarCatalogError::Invalid(format!(
                "unsupported asset manifest schema {:?}; expected {:?}",
                manifest_document.schema, PROJECT_ASSET_SCHEMA
            )));
        }
        if manifest_document.protocol != PROTOCOL_0104 {
            return Err(AvatarCatalogError::Invalid(format!(
                "asset manifest protocol {} is not 0104",
                manifest_document.protocol
            )));
        }
        let manifest = index_manifest(manifest_document.files)?;

        let plan_path = canonical_file(&options.semantic_plan_path)?;
        let plan_bytes = read_file(&plan_path)?;
        let plan_hash = blake3::hash(&plan_bytes).to_hex().to_string();
        let plan: SemanticPlan = parse_json(&plan_path, &plan_bytes)?;
        if plan.schema != SEMANTIC_PLAN_SCHEMA {
            return Err(AvatarCatalogError::Invalid(format!(
                "unsupported semantic plan schema {:?}; expected {:?}",
                plan.schema, SEMANTIC_PLAN_SCHEMA
            )));
        }

        let mut provenance_issues = Vec::new();
        if plan.mode != "plan-only" {
            provenance_issues.push(format!(
                "semantic evidence mode {:?} is not plan-only",
                plan.mode
            ));
        }
        if plan.status != "ready" {
            provenance_issues.push(format!(
                "semantic ownership plan status is {:?}, not ready",
                plan.status
            ));
        }
        if plan.production_assets_mutated {
            provenance_issues
                .push("semantic ownership report claims it mutated production assets".to_owned());
        }
        match options.expected_semantic_plan_blake3.as_deref() {
            Some(expected) if expected == plan_hash => {}
            Some(expected) => provenance_issues.push(format!(
                "semantic plan BLAKE3 mismatch: expected {expected}, actual {plan_hash}"
            )),
            None => provenance_issues.push(
                "semantic plan has no trusted expected BLAKE3 pin; report-only resolution"
                    .to_owned(),
            ),
        }

        let table_evidence = unique_input(&plan.inputs, "tableSet")?;
        let content_evidence = unique_input(&plan.inputs, "contentIndex")?;
        let manifest_evidence = unique_input(&plan.inputs, "assetManifest")?;
        require_input_schema(table_evidence, TABLE_SET_SCHEMA)?;
        require_input_schema(content_evidence, CONTENT_INDEX_SCHEMA)?;
        require_input_schema(manifest_evidence, PROJECT_ASSET_SCHEMA)?;

        let resolved_manifest_evidence =
            resolve_evidence_path(&evidence_root, &workspace_root, &manifest_evidence.path)?;
        if resolved_manifest_evidence != manifest_path {
            provenance_issues.push(format!(
                "semantic plan assetManifest resolves to {}, configured manifest is {}",
                resolved_manifest_evidence.display(),
                manifest_path.display()
            ));
        }
        verify_evidence(
            manifest_evidence,
            &resolved_manifest_evidence,
            &mut provenance_issues,
        );

        let table_path =
            resolve_evidence_path(&evidence_root, &workspace_root, &table_evidence.path)?;
        if !table_path.starts_with(&asset_root) {
            return Err(AvatarCatalogError::Invalid(format!(
                "TableData evidence {} is outside native asset root {}",
                table_path.display(),
                asset_root.display()
            )));
        }
        let table_relative = project_relative_path(&asset_root, &table_path)?;
        let table_manifest = manifest.get(&table_relative).ok_or_else(|| {
            AvatarCatalogError::Invalid(format!(
                "TableData {table_relative:?} is not listed by the native asset manifest"
            ))
        })?;
        if table_manifest.kind != ProjectAssetKind::Data {
            return Err(AvatarCatalogError::Invalid(format!(
                "TableData {table_relative:?} is listed as {:?}, not data",
                table_manifest.kind
            )));
        }
        let table_bytes = read_verified_manifest_file(&asset_root, table_manifest)?;
        let table_hash = blake3::hash(&table_bytes).to_hex().to_string();
        if table_evidence.bytes != table_bytes.len() as u64 || table_evidence.blake3 != table_hash {
            return Err(AvatarCatalogError::Invalid(format!(
                "TableData evidence mismatch for {}: plan bytes/hash={}/{}, actual={}/{}",
                table_path.display(),
                table_evidence.bytes,
                table_evidence.blake3,
                table_bytes.len(),
                table_hash
            )));
        }
        let table_set: TableSet = parse_json(&table_path, &table_bytes)?;
        if table_set.schema != TABLE_SET_SCHEMA {
            return Err(AvatarCatalogError::Invalid(format!(
                "unsupported table-set schema {:?}; expected {:?}",
                table_set.schema, TABLE_SET_SCHEMA
            )));
        }
        let [table] = table_set.tables.as_slice() else {
            return Err(AvatarCatalogError::Invalid(format!(
                "avatar resolver requires exactly one TableData document, found {}",
                table_set.tables.len()
            )));
        };
        require_avatar_tables(&table.value)?;

        let content_path =
            resolve_evidence_path(&evidence_root, &workspace_root, &content_evidence.path)?;
        if !content_path.starts_with(&asset_root) {
            return Err(AvatarCatalogError::Invalid(format!(
                "content index {} is outside native asset root {}",
                content_path.display(),
                asset_root.display()
            )));
        }
        let content_relative = project_relative_path(&asset_root, &content_path)?;
        let content_manifest = manifest.get(&content_relative).ok_or_else(|| {
            AvatarCatalogError::Invalid(format!(
                "content index {content_relative:?} is not listed by the native asset manifest"
            ))
        })?;
        let content_bytes = read_verified_manifest_file(&asset_root, content_manifest)?;
        let content_hash = blake3::hash(&content_bytes).to_hex().to_string();
        if content_evidence.bytes != content_bytes.len() as u64
            || content_evidence.blake3 != content_hash
        {
            return Err(AvatarCatalogError::Invalid(format!(
                "content-index evidence mismatch for {}",
                content_path.display()
            )));
        }
        let content: ContentIndex = parse_json(&content_path, &content_bytes)?;
        if content.schema != CONTENT_INDEX_SCHEMA || !content.native_only {
            return Err(AvatarCatalogError::Invalid(format!(
                "content index must be schema {CONTENT_INDEX_SCHEMA:?} with nativeOnly=true"
            )));
        }

        for input in &plan.inputs {
            let path = match resolve_evidence_path(&evidence_root, &workspace_root, &input.path) {
                Ok(path) => path,
                Err(error) => {
                    provenance_issues.push(format!(
                        "{} evidence path {:?} is invalid: {error}",
                        input.role, input.path
                    ));
                    continue;
                }
            };
            verify_evidence(input, &path, &mut provenance_issues);
        }

        provenance_issues.sort();
        provenance_issues.dedup();
        let trusted = provenance_issues.is_empty();
        let provenance = AvatarCatalogProvenance {
            asset_manifest_path: manifest_path.display().to_string(),
            asset_manifest_blake3: manifest_hash,
            table_set_path: table_path.display().to_string(),
            table_set_blake3: table_hash,
            semantic_plan_path: plan_path.display().to_string(),
            semantic_plan_blake3: plan_hash,
            trusted,
            issues: provenance_issues,
        };

        let mut entities = BTreeMap::new();
        let mut player_entities = BTreeMap::new();
        let mut dependencies_by_route: BTreeMap<String, Vec<PlanDependency>> = BTreeMap::new();
        for entity in plan.entities {
            for dependency in &entity.dependencies {
                dependencies_by_route
                    .entry(normalize_route(&dependency.legacy_route))
                    .or_default()
                    .push(dependency.clone());
            }
            if let Some(owner) = &entity.table_owner {
                let table_name = owner
                    .table
                    .strip_suffix(".m_pItemData")
                    .unwrap_or(&owner.table)
                    .to_owned();
                entities.insert((table_name, owner.row_index), entity.clone());
            }
            if entity.id.starts_with("player:") {
                player_entities.insert(entity.id.clone(), entity);
            }
        }
        let models = plan
            .models
            .into_iter()
            .map(|model| (model.id.clone(), model))
            .collect();
        let mut content_by_kind_name: BTreeMap<(String, String), Vec<ContentAsset>> =
            BTreeMap::new();
        for asset in content.assets {
            content_by_kind_name
                .entry((asset.kind.clone(), asset.name.clone()))
                .or_default()
                .push(asset);
        }

        Ok(Self {
            asset_root,
            provenance,
            manifest,
            tables: table.value.clone(),
            entities,
            player_entities,
            models,
            dependencies_by_route,
            content_by_kind_name,
        })
    }

    pub fn provenance(&self) -> &AvatarCatalogProvenance {
        &self.provenance
    }

    pub fn resolve_character_style(
        &self,
        style: &CharacterStyle0104,
        equipment: &[EquippedItem0104; CHARACTER_EQUIP_SLOT_COUNT_0104],
    ) -> AvatarResolutionReport {
        match AvatarStyleSelection::try_from(style) {
            Ok(style) => self.resolve(style, equipment.map(AvatarItemSelection::from)),
            Err(error) => self.invalid_style_report(style, error),
        }
    }

    pub fn resolve_pc_style(
        &self,
        style: &PcStyle0104,
        equipment: &[ItemBase0104; CHARACTER_EQUIP_SLOT_COUNT_0104],
    ) -> AvatarResolutionReport {
        match AvatarStyleSelection::try_from(style) {
            Ok(style) => self.resolve(style, equipment.map(AvatarItemSelection::from)),
            Err(error) => self.invalid_pc_style_report(style, error),
        }
    }

    pub fn resolve(
        &self,
        style: AvatarStyleSelection,
        equipment: [AvatarItemSelection; CHARACTER_EQUIP_SLOT_COUNT_0104],
    ) -> AvatarResolutionReport {
        let hat = equipment[4];
        let hat_policy = self.hat_policy(hat);
        let mut parts = Vec::with_capacity(12);

        parts.push(self.resolve_body(&style));
        parts.push(self.resolve_face(&style, &hat_policy));
        parts.push(self.resolve_hair(&style, &hat_policy));
        parts.push(self.resolve_clothing(
            AvatarPartKind::UpperBody,
            AvatarPartParticipation::CombinedSkinnedMesh,
            "m_pShirtsItemTable",
            "shirt",
            1,
            equipment[1],
            style.gender,
        ));
        parts.push(self.resolve_clothing(
            AvatarPartKind::LowerBody,
            AvatarPartParticipation::CombinedSkinnedMesh,
            "m_pPantsItemTable",
            "pants",
            2,
            equipment[2],
            style.gender,
        ));
        parts.push(self.resolve_clothing(
            AvatarPartKind::Foot,
            AvatarPartParticipation::CombinedSkinnedMesh,
            "m_pShoesItemTable",
            "shoes",
            3,
            equipment[3],
            style.gender,
        ));
        parts.push(self.resolve_item(
            AvatarPartKind::Hat,
            AvatarPartParticipation::SocketAttachment,
            "m_pHatItemTable",
            "hat",
            4,
            equipment[4],
            style.gender,
            false,
        ));

        let mut glasses = self.resolve_item(
            AvatarPartKind::Glasses,
            AvatarPartParticipation::SocketAttachment,
            "m_pGlassItemTable",
            "glasses",
            5,
            equipment[5],
            style.gender,
            false,
        );
        if matches!(hat_policy, Ok(policy) if !policy.glasses_visible) {
            suppress_part(&mut glasses, "hat equip type suppresses the glasses socket");
        } else if let Err(detail) = &hat_policy {
            block_part(&mut glasses, detail.clone());
        }
        parts.push(glasses);

        parts.push(self.resolve_item(
            AvatarPartKind::Back,
            AvatarPartParticipation::BackTablePolicy,
            "m_pBackItemTable",
            "back",
            6,
            equipment[6],
            style.gender,
            false,
        ));
        parts.push(self.resolve_item(
            AvatarPartKind::Hand,
            AvatarPartParticipation::SocketAttachment,
            "m_pWeaponItemTable",
            "weapon",
            0,
            equipment[0],
            style.gender,
            false,
        ));
        let mut extended_hand = self.resolve_item(
            AvatarPartKind::ExtendedHand,
            AvatarPartParticipation::CombatSecondary,
            "m_pWeaponItemTable",
            "weapon",
            0,
            equipment[7],
            style.gender,
            false,
        );
        if equipment[7].item_id > 0 {
            block_part(
                &mut extended_hand,
                "protocol slot 7 is used by combat logic but the standard avatar look path does not attach a second visual; duplicating slot 0 would be incorrect",
            );
        }
        parts.push(extended_hand);
        parts.push(self.resolve_item(
            AvatarPartKind::Vehicle,
            AvatarPartParticipation::SeparateVehicle,
            "m_pVehicleItemTable",
            "vehicle",
            10,
            equipment[8],
            style.gender,
            false,
        ));

        let complete = self.provenance.trusted
            && parts.iter().all(|part| {
                matches!(
                    part.disposition,
                    AvatarPartDisposition::Ready
                        | AvatarPartDisposition::Empty
                        | AvatarPartDisposition::SuppressedByHat
                )
            });
        let missing_source_assets = missing_sources(&parts);
        AvatarResolutionReport {
            schema: AVATAR_RESOLUTION_SCHEMA.to_owned(),
            complete,
            provenance: self.provenance.clone(),
            height_factor: 1.0 - f32::from(style.height) / 4.0,
            body_shape: f32::from(style.body) / 2.0,
            style,
            parts,
            missing_source_assets,
        }
    }

    pub(super) fn invalid_style_report(
        &self,
        style: &CharacterStyle0104,
        error: String,
    ) -> AvatarResolutionReport {
        self.invalid_report(
            AvatarStyleSelection {
                gender: AvatarGender::Male,
                face_style: style.face_style,
                hair_style: style.hair_style,
                hair_color: style.hair_color,
                skin_color: style.skin_color,
                eye_color: style.eye_color,
                height: style.height,
                body: style.body,
            },
            error,
        )
    }

    pub(super) fn invalid_pc_style_report(
        &self,
        style: &PcStyle0104,
        error: String,
    ) -> AvatarResolutionReport {
        self.invalid_report(
            AvatarStyleSelection {
                gender: AvatarGender::Male,
                face_style: style.face_style,
                hair_style: style.hair_style,
                hair_color: style.hair_color,
                skin_color: style.skin_color,
                eye_color: style.eye_color,
                height: style.height,
                body: style.body,
            },
            error,
        )
    }

    pub(super) fn invalid_report(&self, style: AvatarStyleSelection, error: String) -> AvatarResolutionReport {
        AvatarResolutionReport {
            schema: AVATAR_RESOLUTION_SCHEMA.to_owned(),
            complete: false,
            provenance: self.provenance.clone(),
            height_factor: 1.0 - f32::from(style.height) / 4.0,
            body_shape: f32::from(style.body) / 2.0,
            style,
            parts: vec![AvatarPartResolution {
                part: AvatarPartKind::Body,
                semantic_name: "player_invalid_style".to_owned(),
                participation: AvatarPartParticipation::SharedSkeletonBase,
                disposition: AvatarPartDisposition::Missing,
                table: None,
                model: None,
                textures: Vec::new(),
                blockers: vec![error.clone()],
            }],
            missing_source_assets: vec![error],
        }
    }

    pub(super) fn resolve_body(&self, style: &AvatarStyleSelection) -> AvatarPartResolution {
        let token = style.gender.token();
        let entity = self.player_entities.get(&format!("player:{token}:base"));
        let model_route = style.gender.actor_route();
        let semantic_name = format!("player_{token}");
        let model = self.resolve_model(
            entity,
            model_route,
            &format!("characters/player/body/{semantic_name}/{semantic_name}.glb"),
        );
        let skin = self.resolve_texture(
            entity,
            style.gender.skin_route(),
            &format!(
                "characters/player/body/{semantic_name}/textures/{}.png",
                source_stem(style.gender.skin_route()).unwrap_or("skin")
            ),
        );
        let disposition = disposition_for_assets(Some(&model), std::slice::from_ref(&skin));
        let mut blockers = Vec::new();
        if !(0..=4).contains(&style.height) {
            blockers.push(format!(
                "height style {} is outside the authored 0..=4 deformation domain",
                style.height
            ));
        }
        if !(0..=2).contains(&style.body) {
            blockers.push(format!(
                "body style {} is outside the authored 0..=2 deformation domain",
                style.body
            ));
        }
        AvatarPartResolution {
            part: AvatarPartKind::Body,
            semantic_name,
            participation: AvatarPartParticipation::SharedSkeletonBase,
            disposition: if blockers.is_empty() {
                disposition
            } else {
                AvatarPartDisposition::Missing
            },
            table: None,
            model: Some(model),
            textures: vec![skin],
            blockers,
        }
    }

    pub(super) fn resolve_face(
        &self,
        style: &AvatarStyleSelection,
        hat_policy: &Result<HatPolicy, String>,
    ) -> AvatarPartResolution {
        let variant = match hat_policy {
            Ok(policy) => policy.face_variant,
            Err(detail) => {
                return unresolved_style_part(AvatarPartKind::Face, "player_face", detail.clone());
            }
        };
        if style.face_style <= 0 {
            return empty_style_part(AvatarPartKind::Face, "player_face_none");
        }
        let eye_suffix = match style.eye_color {
            0 | 1 => 'a',
            2 => 'b',
            3 => 'c',
            4 => 'd',
            5 => 'e',
            value => {
                return unresolved_style_part(
                    AvatarPartKind::Face,
                    "player_face",
                    format!("eye color {value} is outside the authored 0..=5 domain"),
                );
            }
        };
        let row = match self.item_row("m_pFaceItemTable", i64::from(style.face_style)) {
            Ok(row) => row,
            Err(detail) => {
                return unresolved_style_part(AvatarPartKind::Face, "player_face", detail);
            }
        };
        let mesh = match self.mesh_row(&row) {
            Ok(mesh) => mesh,
            Err(detail) => {
                return unresolved_style_part(AvatarPartKind::Face, "player_face", detail);
            }
        };
        let model_base = match gender_string(&mesh, style.gender, "MeshModelString") {
            Some(value) => value,
            None => {
                return unresolved_style_part(
                    AvatarPartKind::Face,
                    "player_face",
                    "face mesh row has no gender-specific model name".to_owned(),
                );
            }
        };
        let texture_base = match gender_string(&mesh, style.gender, "TextureString") {
            Some(value) => value,
            None => {
                return unresolved_style_part(
                    AvatarPartKind::Face,
                    "player_face",
                    "face mesh row has no gender-specific texture name".to_owned(),
                );
            }
        };
        let model_route =
            make_route("wear", &format!("{model_base}_type0{variant}"), "nif").unwrap();
        let texture_route =
            make_route("texture", &format!("{texture_base}_{eye_suffix}"), "dds").unwrap();
        self.part_from_row(
            AvatarPartKind::Face,
            AvatarPartParticipation::CombinedSkinnedMesh,
            row,
            model_route,
            vec![texture_route, style.gender.skin_route().to_owned()],
        )
    }

    pub(super) fn resolve_hair(
        &self,
        style: &AvatarStyleSelection,
        hat_policy: &Result<HatPolicy, String>,
    ) -> AvatarPartResolution {
        let variant = match hat_policy {
            Ok(policy) => match policy.hair_variant {
                Some(variant) => variant,
                None => {
                    let mut part = empty_style_part(AvatarPartKind::Hair, "player_hair_suppressed");
                    part.disposition = AvatarPartDisposition::SuppressedByHat;
                    return part;
                }
            },
            Err(detail) => {
                return unresolved_style_part(AvatarPartKind::Hair, "player_hair", detail.clone());
            }
        };
        if style.hair_style <= 0 {
            return empty_style_part(AvatarPartKind::Hair, "player_hair_none");
        }
        let row = match self.item_row("m_pHeadItemTable", i64::from(style.hair_style)) {
            Ok(row) => row,
            Err(detail) => {
                return unresolved_style_part(AvatarPartKind::Hair, "player_hair", detail);
            }
        };
        let mesh = match self.mesh_row(&row) {
            Ok(mesh) => mesh,
            Err(detail) => {
                return unresolved_style_part(AvatarPartKind::Hair, "player_hair", detail);
            }
        };
        let model_base = match gender_string(&mesh, style.gender, "MeshModelString") {
            Some(value) => value,
            None => {
                return unresolved_style_part(
                    AvatarPartKind::Hair,
                    "player_hair",
                    "hair mesh row has no gender-specific model name".to_owned(),
                );
            }
        };
        let texture_base = match gender_string(&mesh, style.gender, "TextureString") {
            Some(value) => value,
            None => {
                return unresolved_style_part(
                    AvatarPartKind::Hair,
                    "player_hair",
                    "hair mesh row has no gender-specific texture name".to_owned(),
                );
            }
        };
        let model_route =
            make_route("wear", &format!("{model_base}_type0{variant}"), "nif").unwrap();
        let texture_route = make_route("texture", &format!("{texture_base}_a"), "dds").unwrap();
        self.part_from_row(
            AvatarPartKind::Hair,
            AvatarPartParticipation::CombinedSkinnedMesh,
            row,
            model_route,
            vec![texture_route],
        )
    }
}
