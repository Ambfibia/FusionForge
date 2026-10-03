use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputEvidence {
    pub role: String,
    pub path: String,
    pub schema: String,
    pub bytes: u64,
    pub blake3: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrganizerCounts {
    pub manifest_files: u64,
    pub content_index_assets: u64,
    pub cook_mappings: u64,
    pub logical_plan_ready_roots: u64,
    pub entities: u64,
    pub npc_entities: u64,
    pub mob_entities: u64,
    pub hnpc_entities: u64,
    pub nano_entities: u64,
    pub player_entities: u64,
    pub equipment_entities: BTreeMap<String, u64>,
    pub model_proposals: u64,
    pub eligible_model_proposals: u64,
    pub blocked_model_proposals: u64,
    pub dependencies: u64,
    pub resolved_dependencies: u64,
    pub ambiguous_dependencies: u64,
    pub missing_dependencies: u64,
    pub name_only_candidates: u64,
    pub route_proof_pending: u64,
    pub shared_native_assets: u64,
    pub unresolved_routes: u64,
    pub npc_routes_with_multiple_scales: u64,
    pub blockers: u64,
    pub blockers_by_code: BTreeMap<String, u64>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticCategory {
    Npc,
    Mob,
    Shared,
    Hnpc,
    Nano,
    Player,
    EquipmentHat,
    EquipmentMask,
    EquipmentGlasses,
    EquipmentBack,
    EquipmentHead,
    EquipmentShirt,
    EquipmentPants,
    EquipmentShoes,
    EquipmentWeapon,
    EquipmentVehicle,
    Unresolved,
}

impl SemanticCategory {
    pub(super) fn directory(self) -> &'static str {
        match self {
            Self::Npc => "characters/npc",
            Self::Mob => "characters/mob",
            Self::Shared => "characters/shared",
            Self::Hnpc => "characters/hnpc",
            Self::Nano => "characters/nano",
            Self::Player => "characters/player",
            Self::EquipmentHat => "characters/player/equipment/hat",
            Self::EquipmentMask => "characters/player/equipment/mask",
            Self::EquipmentGlasses => "characters/player/equipment/glasses",
            Self::EquipmentBack => "characters/player/equipment/back",
            Self::EquipmentHead => "characters/player/equipment/head",
            Self::EquipmentShirt => "characters/player/equipment/shirt",
            Self::EquipmentPants => "characters/player/equipment/pants",
            Self::EquipmentShoes => "characters/player/equipment/shoes",
            Self::EquipmentWeapon => "characters/player/equipment/weapon",
            Self::EquipmentVehicle => "characters/player/equipment/vehicle",
            Self::Unresolved => "characters/unresolved",
        }
    }

    pub(super) fn is_equipment(self) -> bool {
        self.directory().starts_with("characters/player/equipment/")
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityProposal {
    pub id: String,
    pub category: SemanticCategory,
    pub semantic_directory: String,
    pub table_owner: TableOwner,
    pub spawn_root_policy: Option<SpawnRootPolicy>,
    pub model_routes: Vec<String>,
    pub model_proposal_ids: Vec<String>,
    pub dependencies: Vec<DependencyProposal>,
    pub blocker_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TableOwner {
    pub table: String,
    pub row_index: usize,
    pub entity_number: Option<i64>,
    pub mesh_index: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpawnRootPolicy {
    pub kind: SpawnRootPolicyKind,
    pub authority: String,
    pub position: RootTrsAction,
    pub rotation: RootTrsAction,
    pub scale: RootTrsAction,
    pub row_m_f_scale: Option<f64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpawnRootPolicyKind {
    NpcSetupNpcReplaceRootTrs,
    NanoPreserveAuthoredRootScale,
    PlayerReplaceRootTrsWithIdentityScale,
    EquipmentRuntimeAttachmentNotIndependent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DependencyProposal {
    pub kind: DependencyKind,
    pub source_field: String,
    pub source_m_name: String,
    pub legacy_route: String,
    pub resolution: DependencyResolution,
    pub ownership: NativeOwnership,
    pub candidates: Vec<NativeAssetReference>,
    pub blocker_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    Texture,
    Audio,
}

impl DependencyKind {
    pub(super) fn cook_kind(self) -> &'static str {
        match self {
            Self::Texture => "texture",
            Self::Audio => "audio",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyResolution {
    Exact,
    UniqueNameCandidateUnproven,
    Missing,
    Ambiguous,
    EvidenceMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeOwnership {
    Entity,
    Shared,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockerCode {
    AmbiguousLogicalOwner,
    LogicalModelNotReady,
    LogicalRouteMissingFromPlan,
    LogicalPlanGlobalBlocker,
    HnpcPhysicalMappingUnproven,
    StandaloneNifProofIncomplete,
    TableReferenceInvalid,
    TableRowHasNoModel,
    NpcScaleMissing,
    NanoSoundTableUnproven,
    NativeMappingMissing,
    NativeMappingAmbiguous,
    NativeEvidenceMismatch,
    RouteTargetIdentityUnproven,
    TrueNameNotPortable,
    NormalizedPathCollision,
    NoTableOwner,
}

impl BlockerCode {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::AmbiguousLogicalOwner => "ambiguous_logical_owner",
            Self::LogicalModelNotReady => "logical_model_not_ready",
            Self::LogicalRouteMissingFromPlan => "logical_route_missing_from_plan",
            Self::LogicalPlanGlobalBlocker => "logical_plan_global_blocker",
            Self::HnpcPhysicalMappingUnproven => "hnpc_physical_mapping_unproven",
            Self::StandaloneNifProofIncomplete => "standalone_nif_proof_incomplete",
            Self::TableReferenceInvalid => "table_reference_invalid",
            Self::TableRowHasNoModel => "table_row_has_no_model",
            Self::NpcScaleMissing => "npc_scale_missing",
            Self::NanoSoundTableUnproven => "nano_sound_table_unproven",
            Self::NativeMappingMissing => "native_mapping_missing",
            Self::NativeMappingAmbiguous => "native_mapping_ambiguous",
            Self::NativeEvidenceMismatch => "native_evidence_mismatch",
            Self::RouteTargetIdentityUnproven => "route_target_identity_unproven",
            Self::TrueNameNotPortable => "true_name_not_portable",
            Self::NormalizedPathCollision => "normalized_path_collision",
            Self::NoTableOwner => "no_table_owner",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrganizationBlocker {
    pub id: String,
    pub code: BlockerCode,
    pub subject: String,
    pub legacy_route: Option<String>,
    pub detail: String,
}

#[derive(Clone)]
pub(super) struct EvidenceDocument {
    pub(super) evidence: InputEvidence,
    pub(super) value: Value,
}

#[derive(Clone, Debug)]
pub(super) struct ReadyRoot {
    pub(super) route: String,
    pub(super) true_name: String,
    pub(super) proof: String,
    pub(super) source_root: SerializedObjectIdentity,
    pub(super) feature_closure: ModelFeatureClosure,
    pub(super) material_count: u64,
}

#[derive(Default)]
pub(super) struct Builder {
    pub(super) entities: Vec<EntityProposal>,
    pub(super) models: Vec<ModelProposal>,
    pub(super) blockers: Vec<OrganizationBlocker>,
    pub(super) unresolved: BTreeMap<String, UnresolvedRoute>,
    pub(super) route_scales: BTreeMap<String, Vec<f64>>,
    pub(super) ready_roots: BTreeMap<String, Vec<ReadyRoot>>,
    pub(super) plan_route_blockers: BTreeMap<String, PlanRouteBlockers>,
    pub(super) native_index: NativeIndex,
}

impl Builder {
    pub(super) fn add_blocker(
        &mut self,
        code: BlockerCode,
        subject: impl Into<String>,
        legacy_route: Option<String>,
        detail: impl Into<String>,
    ) -> String {
        let id = format!("blocker:{:06}", self.blockers.len() + 1);
        self.blockers.push(OrganizationBlocker {
            id: id.clone(),
            code,
            subject: subject.into(),
            legacy_route,
            detail: detail.into(),
        });
        id
    }

    pub(super) fn add_dependency(
        &mut self,
        entity: &mut EntityProposal,
        row: &Value,
        field: &str,
        prefix: &str,
        extension: &str,
        kind: DependencyKind,
    ) {
        if let Some(route) = route_from_field(row, field, prefix, extension) {
            self.add_dependency_route(entity, field.to_owned(), route, kind);
        }
    }

    pub(super) fn add_dependency_route(
        &mut self,
        entity: &mut EntityProposal,
        source_field: String,
        route: FieldRoute,
        kind: DependencyKind,
    ) {
        if entity.dependencies.iter().any(|dependency| {
            dependency.kind == kind && dependency.legacy_route == route.legacy_route
        }) {
            return;
        }
        let candidates = self
            .native_index
            .by_kind_name
            .get(&(kind.cook_kind().to_owned(), route.source_name.clone()))
            .cloned()
            .unwrap_or_default();
        let (resolution, blocker_code, detail) = match candidates.as_slice() {
            [] => (
                DependencyResolution::Missing,
                Some(BlockerCode::NativeMappingMissing),
                format!(
                    "no exact cook mapping for {} m_Name {:?}",
                    kind.cook_kind(),
                    route.source_name
                ),
            ),
            [candidate] if candidate.project_path.is_none() => (
                DependencyResolution::EvidenceMismatch,
                Some(BlockerCode::NativeEvidenceMismatch),
                format!(
                    "cook/content candidate for {:?} has no asset-manifest project path",
                    route.source_name
                ),
            ),
            [_] => (
                DependencyResolution::UniqueNameCandidateUnproven,
                Some(BlockerCode::RouteTargetIdentityUnproven),
                format!(
                    "unique (kind, m_Name) candidate {:?} is not an exact AssetLoader route target identity join",
                    route.source_name
                ),
            ),
            _ => (
                DependencyResolution::Ambiguous,
                Some(BlockerCode::NativeMappingAmbiguous),
                format!(
                    "exact m_Name {:?} resolves to {} distinct native assets",
                    route.source_name,
                    candidates.len()
                ),
            ),
        };
        let mut blocker_ids = Vec::new();
        if let Some(code) = blocker_code {
            let blocker =
                self.add_blocker(code, &entity.id, Some(route.legacy_route.clone()), detail);
            entity.blocker_ids.push(blocker.clone());
            blocker_ids.push(blocker);
        }
        entity.dependencies.push(DependencyProposal {
            kind,
            source_field,
            source_m_name: route.source_name,
            legacy_route: route.legacy_route,
            resolution,
            ownership: NativeOwnership::Unresolved,
            candidates,
            blocker_ids,
        });
    }

    pub(super) fn resolve_models(&mut self) {
        let mut route_entities = BTreeMap::<String, BTreeSet<String>>::new();
        let mut route_categories = BTreeMap::<String, BTreeSet<SemanticCategory>>::new();
        for entity in &self.entities {
            for route in &entity.model_routes {
                route_entities
                    .entry(route.clone())
                    .or_default()
                    .insert(entity.id.clone());
                route_categories
                    .entry(route.clone())
                    .or_default()
                    .insert(entity.category);
            }
        }
        let every_route = self
            .ready_roots
            .keys()
            .chain(route_entities.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut proposal_by_route = BTreeMap::<String, String>::new();
        for route in every_route {
            let entity_ids = route_entities
                .get(&route)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>();
            let Some(roots) = self.ready_roots.get(&route).cloned() else {
                self.add_unresolved_route(&route, &entity_ids);
                continue;
            };
            if roots.len() != 1 {
                let blocker = self.add_blocker(
                    BlockerCode::AmbiguousLogicalOwner,
                    format!("model:{route}"),
                    Some(route.clone()),
                    format!(
                        "logical plan contains {} ready roots for one normalized route",
                        roots.len()
                    ),
                );
                self.unresolved.insert(
                    route.clone(),
                    UnresolvedRoute {
                        legacy_route: route,
                        referenced_entity_ids: entity_ids,
                        logical_plan_codes: vec!["duplicateReadyRoot".to_owned()],
                        logical_plan_details: vec![blocker],
                    },
                );
                continue;
            }
            let root = roots[0].clone();
            debug_assert_eq!(root.route, route);
            let categories = route_categories.get(&route).cloned().unwrap_or_default();
            let category = if categories.len() > 1 {
                SemanticCategory::Shared
            } else {
                categories
                    .iter()
                    .next()
                    .copied()
                    .unwrap_or(SemanticCategory::Unresolved)
            };
            let route_directory = route_directory(&route);
            let semantic_directory = if route_directory.is_empty() {
                category.directory().to_owned()
            } else {
                format!("{}/{}", category.directory(), route_directory)
            };
            let output_glb = format!("{semantic_directory}/{}.glb", root.true_name);
            let id = format!("model:{route}");
            let mut blocker_ids = Vec::new();
            let mut eligible = true;
            if category == SemanticCategory::Unresolved {
                blocker_ids.push(self.add_blocker(
                    BlockerCode::NoTableOwner,
                    &id,
                    Some(route.clone()),
                    "ready logical root has no table/runtime entity owner; filename or route-name guessing is forbidden",
                ));
                eligible = false;
            }
            if !portable_true_name(&root.true_name) {
                blocker_ids.push(self.add_blocker(
                    BlockerCode::TrueNameNotPortable,
                    &id,
                    Some(route.clone()),
                    format!(
                        "exact root m_Name {:?} cannot be used unchanged as a Windows GLB filename",
                        root.true_name
                    ),
                ));
                eligible = false;
            }
            proposal_by_route.insert(route.clone(), id.clone());
            self.models.push(ModelProposal {
                id,
                legacy_route: route,
                true_root_m_name: root.true_name,
                category,
                semantic_directory,
                output_glb,
                proof: root.proof,
                source_root: root.source_root,
                feature_closure: root.feature_closure,
                materials: ModelMaterialPlan {
                    status: ModelMaterialStatus::EmittedWithLogicalModelSidecarPending,
                    source_material_count: root.material_count,
                    detail: "exact logical-model material/texture closure stays with the GLB publisher; semantic organizer does not scatter raw material objects".to_owned(),
                },
                entity_ids,
                eligible,
                blocker_ids,
            });
        }
        self.models
            .sort_by(|left, right| left.legacy_route.cmp(&right.legacy_route));
        self.apply_model_path_collision_gate();
        for entity in &mut self.entities {
            entity.model_proposal_ids = entity
                .model_routes
                .iter()
                .filter_map(|route| proposal_by_route.get(route).cloned())
                .collect();
        }
        let unresolved_by_route = self.unresolved.clone();
        let mut additions = Vec::<(usize, BlockerCode, String, String)>::new();
        for (index, entity) in self.entities.iter().enumerate() {
            for route in &entity.model_routes {
                if let Some(unresolved) = unresolved_by_route.get(route) {
                    let ambiguous = unresolved
                        .logical_plan_codes
                        .iter()
                        .any(|code| code == "kfmPhysicalTargetConflict");
                    additions.push((
                        index,
                        if ambiguous {
                            BlockerCode::AmbiguousLogicalOwner
                        } else if route.ends_with(".nif") {
                            BlockerCode::StandaloneNifProofIncomplete
                        } else if unresolved.logical_plan_codes.is_empty() {
                            BlockerCode::LogicalRouteMissingFromPlan
                        } else {
                            BlockerCode::LogicalModelNotReady
                        },
                        route.clone(),
                        if unresolved.logical_plan_codes.is_empty() {
                            "route has no ready root or typed blocker in logical-model plan"
                                .to_owned()
                        } else {
                            format!(
                                "logical-model plan blockers: {}",
                                unresolved.logical_plan_codes.join(", ")
                            )
                        },
                    ));
                }
            }
        }
        for (index, code, route, detail) in additions {
            let entity_id = self.entities[index].id.clone();
            let blocker = self.add_blocker(code, entity_id, Some(route), detail);
            self.entities[index].blocker_ids.push(blocker);
        }
    }

    pub(super) fn add_unresolved_route(&mut self, route: &str, entity_ids: &[String]) {
        let plan = self
            .plan_route_blockers
            .get(route)
            .cloned()
            .unwrap_or_default();
        self.unresolved.insert(
            route.to_owned(),
            UnresolvedRoute {
                legacy_route: route.to_owned(),
                referenced_entity_ids: entity_ids.to_vec(),
                logical_plan_codes: plan.codes.into_iter().collect(),
                logical_plan_details: plan.details.into_iter().collect(),
            },
        );
    }

    pub(super) fn apply_model_path_collision_gate(&mut self) {
        let mut groups = BTreeMap::<String, Vec<usize>>::new();
        for (index, model) in self.models.iter().enumerate() {
            let folded = model.output_glb.nfkc().collect::<String>().to_lowercase();
            groups.entry(folded).or_default().push(index);
        }
        let collisions = groups
            .into_values()
            .filter(|indices| indices.len() > 1)
            .collect::<Vec<_>>();
        for indices in collisions {
            let paths = indices
                .iter()
                .map(|index| self.models[*index].output_glb.clone())
                .collect::<Vec<_>>();
            for index in indices {
                let subject = self.models[index].id.clone();
                let route = self.models[index].legacy_route.clone();
                let blocker = self.add_blocker(
                    BlockerCode::NormalizedPathCollision,
                    subject,
                    Some(route),
                    format!(
                        "NFKC/case-insensitive output collision among {}",
                        paths.join(", ")
                    ),
                );
                self.models[index].eligible = false;
                self.models[index].blocker_ids.push(blocker);
            }
        }
    }

    pub(super) fn resolve_shared_assets(&mut self) -> Vec<SharedNativeAsset> {
        #[derive(Default)]
        struct Usage {
            native: Option<NativeAssetReference>,
            entities: BTreeSet<String>,
            categories: BTreeSet<SemanticCategory>,
            routes: BTreeSet<String>,
        }
        let mut usages = BTreeMap::<String, Usage>::new();
        for entity in &self.entities {
            for dependency in &entity.dependencies {
                if dependency.resolution != DependencyResolution::Exact {
                    continue;
                }
                let candidate = dependency.candidates[0].clone();
                let usage = usages.entry(candidate.native_key.clone()).or_default();
                usage.native.get_or_insert(candidate);
                usage.entities.insert(entity.id.clone());
                usage.categories.insert(entity.category);
                usage.routes.insert(dependency.legacy_route.clone());
            }
        }
        let shared_keys = usages
            .iter()
            .filter_map(|(key, usage)| (usage.entities.len() > 1).then_some(key.clone()))
            .collect::<BTreeSet<_>>();
        for entity in &mut self.entities {
            for dependency in &mut entity.dependencies {
                if dependency
                    .candidates
                    .first()
                    .is_some_and(|candidate| shared_keys.contains(&candidate.native_key))
                {
                    dependency.ownership = NativeOwnership::Shared;
                }
            }
        }
        usages
            .into_iter()
            .filter_map(|(key, usage)| {
                shared_keys.contains(&key).then(|| SharedNativeAsset {
                    native: usage.native.expect("exact usage has a native candidate"),
                    entity_count: usage.entities.len() as u64,
                    categories: usage.categories.into_iter().collect(),
                    legacy_routes: usage.routes.into_iter().collect(),
                })
            })
            .collect()
    }

    pub(super) fn route_scale_report(&self) -> Vec<RouteScaleUsage> {
        self.route_scales
            .iter()
            .map(|(route, values)| {
                let mut unique = values.clone();
                unique.sort_by(f64::total_cmp);
                unique.dedup_by(|left, right| left.to_bits() == right.to_bits());
                RouteScaleUsage {
                    legacy_route: route.clone(),
                    entity_count: values.len() as u64,
                    multiple_values: unique.len() > 1,
                    values: unique,
                }
            })
            .collect()
    }

    pub(super) fn finish_unresolved(&self) -> Vec<UnresolvedRoute> {
        self.unresolved.values().cloned().collect()
    }
}
