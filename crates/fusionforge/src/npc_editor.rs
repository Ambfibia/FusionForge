use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NpcAuthoringMode {
    Edit,
    Blank,
    Clone,
    CustomModel,
    ImportBuild,
}

impl NpcAuthoringMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Edit => "Редактирование",
            Self::Blank => "Новый TableData NPC",
            Self::Clone => "Клон NPC",
            Self::CustomModel => "Новый NPC из GLB/GLTF",
            Self::ImportBuild => "Импорт из другой сборки",
        }
    }

    pub(crate) fn action_label(self) -> &'static str {
        match self {
            Self::Edit => "Сохранить изменения",
            Self::Blank => "Добавить NPC",
            Self::Clone => "Создать клон",
            Self::CustomModel => "Импортировать NPC",
            Self::ImportBuild => "Импортировать из сборки",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Edit => {
                "Изменяет staged TableData выбранного NPC. Исходная сборка остаётся нетронутой до Build."
            }
            Self::Blank => {
                "Создаёт только строки TableData. Модель, standalone bundle и серверное размещение этим режимом не создаются."
            }
            Self::Clone => {
                "Копирует строки выбранного NPC и его ссылки на существующие ресурсы. Новый standalone bundle не создаётся."
            }
            Self::CustomModel => {
                "Берёт скелет и схему TableData выбранного шаблона, импортирует GLB/GLTF и готовит проектные assets для сборки Character_*.resourceFile."
            }
            Self::ImportBuild => {
                "Копирует NPC из другой клиентской сборки и материализует найденные зависимости в папку NPC текущего проекта."
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NpcWorkspaceTab {
    Catalog,
    Overview,
    Editor,
    Preview,
    Assets,
    Usage,
    TableData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NpcWorkspaceLayout {
    Compact,
    Medium,
    Wide,
}

pub(crate) fn npc_workspace_layout(width: f32, height: f32) -> NpcWorkspaceLayout {
    if width >= 1240.0 && height >= 680.0 {
        NpcWorkspaceLayout::Wide
    } else if width >= 820.0 {
        NpcWorkspaceLayout::Medium
    } else {
        NpcWorkspaceLayout::Compact
    }
}

const NPC_WIDE_PRIMARY_TABS: &[NpcWorkspaceTab] = &[
    NpcWorkspaceTab::Overview,
    NpcWorkspaceTab::Editor,
    NpcWorkspaceTab::Usage,
];
const NPC_WIDE_SECONDARY_TABS: &[NpcWorkspaceTab] =
    &[NpcWorkspaceTab::Assets, NpcWorkspaceTab::TableData];
const NPC_MEDIUM_PRIMARY_TABS: &[NpcWorkspaceTab] = &[
    NpcWorkspaceTab::Overview,
    NpcWorkspaceTab::Editor,
    NpcWorkspaceTab::Usage,
];
const NPC_MEDIUM_SECONDARY_TABS: &[NpcWorkspaceTab] =
    &[NpcWorkspaceTab::Assets, NpcWorkspaceTab::TableData];
const NPC_COMPACT_TABS: &[NpcWorkspaceTab] = &[
    NpcWorkspaceTab::Catalog,
    NpcWorkspaceTab::Overview,
    NpcWorkspaceTab::Editor,
    NpcWorkspaceTab::Preview,
    NpcWorkspaceTab::Usage,
    NpcWorkspaceTab::Assets,
    NpcWorkspaceTab::TableData,
];

pub(crate) fn npc_primary_tabs(layout: NpcWorkspaceLayout) -> &'static [NpcWorkspaceTab] {
    match layout {
        NpcWorkspaceLayout::Wide => NPC_WIDE_PRIMARY_TABS,
        NpcWorkspaceLayout::Medium => NPC_MEDIUM_PRIMARY_TABS,
        NpcWorkspaceLayout::Compact => &[],
    }
}

pub(crate) fn npc_secondary_tabs(layout: NpcWorkspaceLayout) -> &'static [NpcWorkspaceTab] {
    match layout {
        NpcWorkspaceLayout::Wide => NPC_WIDE_SECONDARY_TABS,
        NpcWorkspaceLayout::Medium => NPC_MEDIUM_SECONDARY_TABS,
        NpcWorkspaceLayout::Compact => &[],
    }
}

pub(crate) fn npc_compact_tabs() -> &'static [NpcWorkspaceTab] {
    NPC_COMPACT_TABS
}

pub(crate) fn npc_wizard_size(available_width: f32, available_height: f32) -> [f32; 2] {
    [
        (available_width - 24.0).clamp(320.0, 720.0),
        (available_height - 24.0).clamp(400.0, 720.0),
    ]
}

impl NpcWorkspaceTab {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Catalog => "Каталог",
            Self::Overview => "Профиль",
            Self::Editor => "Правка",
            Self::Preview => "3D",
            Self::Assets => "Ресурсы",
            Self::Usage => "Использование",
            Self::TableData => "TableData",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NpcCreateWizardStep {
    Source,
    Identity,
    Role,
    Assets,
    Review,
}

impl NpcCreateWizardStep {
    pub(crate) const ALL: [Self; 5] = [
        Self::Source,
        Self::Identity,
        Self::Role,
        Self::Assets,
        Self::Review,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Source => "Источник",
            Self::Identity => "Имя и ID",
            Self::Role => "Роль",
            Self::Assets => "Ресурсы",
            Self::Review => "Проверка",
        }
    }

    pub(crate) fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|candidate| *candidate == self)
            .unwrap_or_default()
    }

    pub(crate) fn next(self) -> Self {
        Self::ALL
            .get(self.index() + 1)
            .copied()
            .unwrap_or(Self::Review)
    }

    pub(crate) fn previous(self) -> Self {
        self.index()
            .checked_sub(1)
            .and_then(|index| Self::ALL.get(index))
            .copied()
            .unwrap_or(Self::Source)
    }
}

impl Default for NpcCreateWizardStep {
    fn default() -> Self {
        Self::Source
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NpcRolePreset {
    Talking,
    Service,
    Ally,
    Enemy,
}

impl NpcRolePreset {
    pub(crate) const ALL: [Self; 4] = [Self::Talking, Self::Service, Self::Ally, Self::Enemy];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Talking => "Говорящий",
            Self::Service => "Сервисный",
            Self::Ally => "Союзник",
            Self::Enemy => "Противник",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Talking => "Статичный дружелюбный NPC для диалогов и barker-реплик.",
            Self::Service => "Дружелюбный NPC для магазина, банка, транспорта или другого сервиса.",
            Self::Ally => "Дружественный боевой NPC с активным AI-профилем.",
            Self::Enemy => "Враждебный боевой NPC с базовыми параметрами моба.",
        }
    }

    pub(crate) fn apply(self, draft: &mut NpcEditorDraft) {
        let values = match self {
            Self::Talking | Self::Service => [
                ("m_iTeam", "1"),
                ("m_iNpcType", "3"),
                ("m_iAiType", "0"),
                ("m_iMapIcon", "18"),
                ("m_iNpcLevel", "0"),
                ("m_iHP", "0"),
                ("m_iPower", "0"),
                ("m_iProtection", "0"),
                ("m_iHeight", "200"),
                ("m_iRadius", "120"),
                ("m_fScale", "1.0"),
                ("m_iWalkSpeed", "300"),
                ("m_iRunSpeed", "500"),
            ],
            Self::Ally => [
                ("m_iTeam", "1"),
                ("m_iNpcType", "0"),
                ("m_iAiType", "2"),
                ("m_iMapIcon", "0"),
                ("m_iNpcLevel", "1"),
                ("m_iHP", "500"),
                ("m_iPower", "100"),
                ("m_iProtection", "50"),
                ("m_iHeight", "200"),
                ("m_iRadius", "120"),
                ("m_fScale", "1.0"),
                ("m_iWalkSpeed", "300"),
                ("m_iRunSpeed", "500"),
            ],
            Self::Enemy => [
                ("m_iTeam", "2"),
                ("m_iNpcType", "0"),
                ("m_iAiType", "2"),
                ("m_iMapIcon", "0"),
                ("m_iNpcLevel", "1"),
                ("m_iHP", "500"),
                ("m_iPower", "100"),
                ("m_iProtection", "50"),
                ("m_iHeight", "200"),
                ("m_iRadius", "120"),
                ("m_fScale", "1.0"),
                ("m_iWalkSpeed", "300"),
                ("m_iRunSpeed", "500"),
            ],
        };
        for (key, value) in values {
            draft
                .profile_values
                .insert(key.to_string(), value.to_string());
        }
    }

    fn infer(draft: &NpcEditorDraft) -> Self {
        let value = |key: &str| {
            draft
                .profile_values
                .get(key)
                .map(String::as_str)
                .unwrap_or_default()
                .trim()
        };
        if value("m_iTeam") == "2" {
            Self::Enemy
        } else if value("m_iAiType") == "2" || value("m_iHP").parse::<i64>().unwrap_or_default() > 0
        {
            Self::Ally
        } else {
            Self::Talking
        }
    }
}

impl Default for NpcRolePreset {
    fn default() -> Self {
        Self::Talking
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NpcProfileValueKind {
    Integer,
    Float,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct NpcProfileField {
    pub key: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub kind: NpcProfileValueKind,
    pub default: &'static str,
}

pub(crate) const NPC_PROFILE_FIELDS: &[NpcProfileField] = &[
    NpcProfileField {
        key: "m_iTeam",
        label: "Команда",
        hint: "1 — дружелюбный, 2 — противник",
        kind: NpcProfileValueKind::Integer,
        default: "1",
    },
    NpcProfileField {
        key: "m_iNpcType",
        label: "Тип NPC",
        hint: "Клиентский тип поведения NPC",
        kind: NpcProfileValueKind::Integer,
        default: "3",
    },
    NpcProfileField {
        key: "m_iAiType",
        label: "AI-тип",
        hint: "Профиль искусственного интеллекта",
        kind: NpcProfileValueKind::Integer,
        default: "0",
    },
    NpcProfileField {
        key: "m_iMapIcon",
        label: "Иконка на карте",
        hint: "Числовой тип маркера на карте",
        kind: NpcProfileValueKind::Integer,
        default: "18",
    },
    NpcProfileField {
        key: "m_iNpcLevel",
        label: "Уровень",
        hint: "Боевой уровень; 0 для обычного говорящего NPC",
        kind: NpcProfileValueKind::Integer,
        default: "0",
    },
    NpcProfileField {
        key: "m_iHP",
        label: "HP",
        hint: "Запас здоровья",
        kind: NpcProfileValueKind::Integer,
        default: "0",
    },
    NpcProfileField {
        key: "m_iPower",
        label: "Сила атаки",
        hint: "Базовая сила атаки",
        kind: NpcProfileValueKind::Integer,
        default: "0",
    },
    NpcProfileField {
        key: "m_iProtection",
        label: "Защита",
        hint: "Базовая защита",
        kind: NpcProfileValueKind::Integer,
        default: "0",
    },
    NpcProfileField {
        key: "m_iHeight",
        label: "Высота",
        hint: "Высота персонажа в клиентских единицах",
        kind: NpcProfileValueKind::Integer,
        default: "200",
    },
    NpcProfileField {
        key: "m_iRadius",
        label: "Радиус",
        hint: "Радиус взаимодействия/коллизии",
        kind: NpcProfileValueKind::Integer,
        default: "120",
    },
    NpcProfileField {
        key: "m_fScale",
        label: "Масштаб",
        hint: "Множитель размера модели",
        kind: NpcProfileValueKind::Float,
        default: "1.0",
    },
    NpcProfileField {
        key: "m_iWalkSpeed",
        label: "Скорость ходьбы",
        hint: "Скорость обычного движения",
        kind: NpcProfileValueKind::Integer,
        default: "300",
    },
    NpcProfileField {
        key: "m_iRunSpeed",
        label: "Скорость бега",
        hint: "Скорость быстрого движения",
        kind: NpcProfileValueKind::Integer,
        default: "500",
    },
];

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NpcEditorDraft {
    pub npc_id: String,
    pub template_npc_id: String,
    pub source_build_dir: String,
    pub source_npc_id: String,
    pub name: String,
    pub internal_name: String,
    pub comment: String,
    pub comment1: String,
    pub greeting_name: String,
    pub greeting_comment: String,
    pub greeting_comment1: String,
    pub greeting_key: String,
    pub authoring_model_path: String,
    pub model_bundle: String,
    pub model_asset: String,
    pub texture_bundle: String,
    pub texture_asset: String,
    pub icon_bundle: String,
    pub icon_asset: String,
    pub profile_values: BTreeMap<String, String>,
    pub advanced_profile_json: String,
    pub notes: String,
}

impl Default for NpcEditorDraft {
    fn default() -> Self {
        let profile_values = NPC_PROFILE_FIELDS
            .iter()
            .map(|field| (field.key.to_string(), field.default.to_string()))
            .collect();
        Self {
            npc_id: "-1".to_string(),
            template_npc_id: String::new(),
            source_build_dir: String::new(),
            source_npc_id: String::new(),
            name: "Новый NPC".to_string(),
            internal_name: "new_npc".to_string(),
            comment: String::new(),
            comment1: String::new(),
            greeting_name: String::new(),
            greeting_comment: String::new(),
            greeting_comment1: String::new(),
            greeting_key: String::new(),
            authoring_model_path: String::new(),
            model_bundle: String::new(),
            model_asset: String::new(),
            texture_bundle: String::new(),
            texture_asset: String::new(),
            icon_bundle: String::new(),
            icon_asset: String::new(),
            profile_values,
            advanced_profile_json: "{}".to_string(),
            notes: String::new(),
        }
    }
}

impl NpcEditorDraft {
    pub(crate) fn from_record(record: &NpcCatalogRecord) -> Self {
        let npc_data = record.related_rows.get("m_pNpcData");
        let mut profile_values = BTreeMap::new();
        for field in NPC_PROFILE_FIELDS {
            let value = npc_data
                .and_then(|row| row.get(field.key))
                .map(json_scalar_text)
                .unwrap_or_default();
            profile_values.insert(field.key.to_string(), value);
        }

        let common = NPC_PROFILE_FIELDS
            .iter()
            .map(|field| field.key)
            .collect::<BTreeSet<_>>();
        let advanced = npc_data
            .and_then(JsonValue::as_object)
            .map(|row| {
                row.iter()
                    .filter(|(key, _)| !common.contains(key.as_str()))
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect::<serde_json::Map<_, _>>()
            })
            .unwrap_or_default();
        let mesh_row = related_row(record, "m_pNpcMeshData");

        Self {
            npc_id: record.id.to_string(),
            template_npc_id: record.id.to_string(),
            source_build_dir: String::new(),
            source_npc_id: record.id.to_string(),
            name: record.name.clone(),
            internal_name: record.internal_name.clone().unwrap_or_default(),
            comment: record.comment.clone().unwrap_or_default(),
            comment1: record.comment1.clone().unwrap_or_default(),
            greeting_name: record
                .barker
                .as_ref()
                .and_then(|row| row.name.clone())
                .unwrap_or_default(),
            greeting_comment: record
                .barker
                .as_ref()
                .and_then(|row| row.comment.clone())
                .unwrap_or_default(),
            greeting_comment1: record
                .barker
                .as_ref()
                .and_then(|row| row.comment1.clone())
                .unwrap_or_default(),
            greeting_key: record
                .barker
                .as_ref()
                .and_then(|row| row.comment2.clone())
                .unwrap_or_default(),
            authoring_model_path: String::new(),
            model_bundle: String::new(),
            model_asset: mesh_row
                .and_then(|row| row.get("m_pstrMMeshModelString"))
                .and_then(JsonValue::as_str)
                .map(model_asset_path)
                .unwrap_or_default(),
            texture_bundle: String::new(),
            texture_asset: mesh_row
                .and_then(|row| row.get("m_pstrMTextureString"))
                .and_then(JsonValue::as_str)
                .map(texture_asset_path)
                .unwrap_or_default(),
            icon_bundle: String::new(),
            icon_asset: record
                .icon
                .as_ref()
                .and_then(|icon| icon.asset_paths.first())
                .cloned()
                .unwrap_or_default(),
            profile_values,
            advanced_profile_json: serde_json::to_string_pretty(&JsonValue::Object(advanced))
                .unwrap_or_else(|_| "{}".to_string()),
            notes: String::new(),
        }
    }

    pub(crate) fn for_mode(mode: NpcAuthoringMode, record: Option<&NpcCatalogRecord>) -> Self {
        let mut draft = record.map(Self::from_record).unwrap_or_default();
        match mode {
            NpcAuthoringMode::Edit => {}
            NpcAuthoringMode::Blank => {
                draft = Self::default();
            }
            NpcAuthoringMode::Clone => {
                draft.npc_id = "-1".to_string();
                draft.name = format!("{} — копия", draft.name);
                draft.internal_name = if draft.internal_name.trim().is_empty() {
                    "npc_copy".to_string()
                } else {
                    format!("{}_copy", draft.internal_name.trim())
                };
            }
            NpcAuthoringMode::CustomModel => {
                draft.npc_id = "-1".to_string();
                draft.name = "Новый NPC".to_string();
                draft.internal_name = "custom_npc".to_string();
                draft.comment.clear();
                draft.comment1.clear();
                draft.greeting_name.clear();
                draft.greeting_comment.clear();
                draft.greeting_comment1.clear();
                draft.greeting_key.clear();
                draft.authoring_model_path.clear();
                draft.model_bundle.clear();
                draft.model_asset = "mob/custom_npc.kfm".to_string();
                draft.texture_bundle.clear();
                draft.texture_asset.clear();
                draft.icon_bundle.clear();
                draft.icon_asset.clear();
            }
            NpcAuthoringMode::ImportBuild => {
                draft = Self::default();
                draft.name.clear();
                draft.internal_name.clear();
                draft.template_npc_id.clear();
                draft.source_npc_id.clear();
                for value in draft.profile_values.values_mut() {
                    value.clear();
                }
                draft.advanced_profile_json = "{}".to_string();
            }
        }
        draft
    }

    pub(crate) fn npc_id_value(&self) -> Result<i64, String> {
        parse_i64_field("ID NPC", &self.npc_id)
    }

    pub(crate) fn template_id_value(&self) -> Result<usize, String> {
        parse_usize_field("ID NPC-шаблона", &self.template_npc_id)
    }

    pub(crate) fn source_id_value(&self) -> Result<usize, String> {
        parse_usize_field("ID исходного NPC", &self.source_npc_id)
    }

    pub(crate) fn to_blueprint(&self) -> Result<NpcBlueprint, String> {
        if self.name.trim().is_empty() {
            return Err("Укажите отображаемое имя NPC.".to_string());
        }
        if self.internal_name.trim().is_empty() {
            return Err("Укажите внутреннее имя NPC.".to_string());
        }
        let advanced = serde_json::from_str::<JsonValue>(&self.advanced_profile_json)
            .map_err(|err| format!("Дополнительные поля профиля: {err}"))?;
        let mut profile = advanced
            .as_object()
            .ok_or_else(|| "Дополнительные поля профиля должны быть JSON-объектом.".to_string())?
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<BTreeMap<_, _>>();

        for field in NPC_PROFILE_FIELDS {
            let raw = self
                .profile_values
                .get(field.key)
                .map(String::as_str)
                .unwrap_or_default()
                .trim();
            if raw.is_empty() {
                continue;
            }
            let value = match field.kind {
                NpcProfileValueKind::Integer => raw
                    .parse::<i64>()
                    .map(JsonValue::from)
                    .map_err(|err| format!("{}: {err}", field.label))?,
                NpcProfileValueKind::Float => raw
                    .replace(',', ".")
                    .parse::<f64>()
                    .map(JsonValue::from)
                    .map_err(|err| format!("{}: {err}", field.label))?,
            };
            profile.insert(field.key.to_string(), value);
        }

        let model_source =
            optional_text(&self.authoring_model_path).or_else(|| optional_text(&self.model_bundle));
        Ok(NpcBlueprint {
            npc_id: self.npc_id_value()?,
            template_npc_id: optional_i64(&self.template_npc_id)?,
            name: self.name.trim().to_string(),
            internal_name: self.internal_name.trim().to_string(),
            comment: Some(self.comment.clone()),
            comment1: Some(self.comment1.clone()),
            greeting_name: Some(self.greeting_name.clone()),
            greeting_comment: Some(self.greeting_comment.clone()),
            greeting_comment1: Some(self.greeting_comment1.clone()),
            greeting_key: Some(self.greeting_key.clone()),
            authoring_model_path: optional_text(&self.authoring_model_path),
            model_bundle: model_source,
            model_asset: optional_text(&self.model_asset),
            texture_bundle: optional_text(&self.texture_bundle),
            texture_asset: optional_text(&self.texture_asset),
            icon_bundle: optional_text(&self.icon_bundle),
            icon_asset: optional_text(&self.icon_asset),
            audio_source: None,
            audio_prefix: None,
            animation_set: None,
            generated_icon: None,
            profile,
            spawn_map: None,
            spawn_position: None,
            spawn_angle: None,
            spawn_json_id: None,
            notes: optional_text(&self.notes),
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct NpcCreateWizardState {
    pub open: bool,
    pub step: NpcCreateWizardStep,
    pub role_preset: NpcRolePreset,
    pub alias_edited: bool,
}

impl Default for NpcCreateWizardState {
    fn default() -> Self {
        Self {
            open: false,
            step: NpcCreateWizardStep::Source,
            role_preset: NpcRolePreset::Talking,
            alias_edited: false,
        }
    }
}

impl NpcCreateWizardState {
    pub(crate) fn begin(&mut self, draft: &mut NpcEditorDraft) {
        self.open = true;
        self.step = NpcCreateWizardStep::Source;
        self.role_preset = NpcRolePreset::infer(draft);
        self.alias_edited = false;
        self.sync_auto_alias(draft);
    }

    pub(crate) fn close(&mut self) {
        self.open = false;
        self.step = NpcCreateWizardStep::Source;
        self.alias_edited = false;
    }

    pub(crate) fn next(&mut self) {
        self.step = self.step.next();
    }

    pub(crate) fn previous(&mut self) {
        self.step = self.step.previous();
    }

    pub(crate) fn set_role_preset(&mut self, draft: &mut NpcEditorDraft, preset: NpcRolePreset) {
        self.role_preset = preset;
        preset.apply(draft);
    }

    pub(crate) fn set_name(&mut self, draft: &mut NpcEditorDraft, name: impl Into<String>) {
        draft.name = name.into();
        self.sync_auto_alias(draft);
    }

    pub(crate) fn set_alias(&mut self, draft: &mut NpcEditorDraft, alias: impl Into<String>) {
        draft.internal_name = alias.into();
        self.alias_edited = true;
    }

    pub(crate) fn resume_auto_alias(&mut self, draft: &mut NpcEditorDraft) {
        self.alias_edited = false;
        self.sync_auto_alias(draft);
    }

    pub(crate) fn sync_auto_alias(&self, draft: &mut NpcEditorDraft) {
        if self.alias_edited {
            return;
        }
        draft.internal_name = if draft.name.trim().is_empty() {
            String::new()
        } else {
            npc_alias_from_name(&draft.name)
        };
    }
}

#[derive(Debug, Clone)]
pub(crate) struct NpcAuthoringState {
    pub mode: NpcAuthoringMode,
    pub workspace_tab: NpcWorkspaceTab,
    pub draft: NpcEditorDraft,
    baseline: NpcEditorDraft,
    pub source_npc_id: Option<usize>,
    pub loaded_npc_id: Option<usize>,
    pub validation_error: Option<String>,
    pub wizard: NpcCreateWizardState,
}

impl Default for NpcAuthoringState {
    fn default() -> Self {
        let draft = NpcEditorDraft::default();
        Self {
            mode: NpcAuthoringMode::Edit,
            workspace_tab: NpcWorkspaceTab::Overview,
            baseline: draft.clone(),
            draft,
            source_npc_id: None,
            loaded_npc_id: None,
            validation_error: None,
            wizard: NpcCreateWizardState::default(),
        }
    }
}

impl NpcAuthoringState {
    pub(crate) fn dirty(&self) -> bool {
        self.draft != self.baseline
    }

    pub(crate) fn load_mode(&mut self, mode: NpcAuthoringMode, record: Option<&NpcCatalogRecord>) {
        self.mode = mode;
        self.source_npc_id = record.map(|record| record.id);
        self.loaded_npc_id = (mode == NpcAuthoringMode::Edit)
            .then(|| record.map(|record| record.id))
            .flatten();
        let mut draft = NpcEditorDraft::for_mode(mode, record);
        if mode == NpcAuthoringMode::Edit {
            self.wizard.close();
        } else {
            self.wizard.begin(&mut draft);
        }
        self.draft = draft;
        self.baseline = self.draft.clone();
        self.validation_error = None;
        self.workspace_tab = NpcWorkspaceTab::Editor;
    }

    pub(crate) fn load_selected_for_edit(&mut self, record: &NpcCatalogRecord) {
        if self.mode != NpcAuthoringMode::Edit {
            return;
        }
        if self.loaded_npc_id == Some(record.id) {
            return;
        }
        self.mode = NpcAuthoringMode::Edit;
        self.source_npc_id = Some(record.id);
        self.loaded_npc_id = Some(record.id);
        self.draft = NpcEditorDraft::from_record(record);
        self.baseline = self.draft.clone();
        self.validation_error = None;
        self.wizard.close();
    }

    pub(crate) fn mark_saved(&mut self) {
        self.baseline = self.draft.clone();
        self.validation_error = None;
        self.wizard.close();
    }

    pub(crate) fn reset(&mut self) {
        self.draft = self.baseline.clone();
        self.validation_error = None;
        if self.wizard.open {
            self.wizard.alias_edited = false;
            self.wizard.sync_auto_alias(&mut self.draft);
        }
    }

    pub(crate) fn apply_wizard_role_preset(&mut self, preset: NpcRolePreset) {
        self.wizard.set_role_preset(&mut self.draft, preset);
    }

    pub(crate) fn set_wizard_name(&mut self, name: impl Into<String>) {
        self.wizard.set_name(&mut self.draft, name);
    }

    pub(crate) fn set_wizard_alias(&mut self, alias: impl Into<String>) {
        self.wizard.set_alias(&mut self.draft, alias);
    }
}

fn related_row<'a>(record: &'a NpcCatalogRecord, prefix: &str) -> Option<&'a JsonValue> {
    record
        .related_rows
        .iter()
        .find(|(key, _)| key.starts_with(prefix))
        .map(|(_, value)| value)
}

fn json_scalar_text(value: &JsonValue) -> String {
    match value {
        JsonValue::String(value) => value.clone(),
        JsonValue::Number(value) => value.to_string(),
        JsonValue::Bool(value) => value.to_string(),
        JsonValue::Null => String::new(),
        other => other.to_string(),
    }
}

fn model_asset_path(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("null") {
        String::new()
    } else if value.contains('/') || value.contains('\\') || value.contains('.') {
        value.replace('\\', "/")
    } else {
        format!("mob/{value}.kfm")
    }
}

fn texture_asset_path(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("null") {
        String::new()
    } else if value.contains('/') || value.contains('\\') || value.contains('.') {
        value.replace('\\', "/")
    } else {
        format!("texture/{value}.dds")
    }
}

fn optional_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn npc_alias_from_name(value: &str) -> String {
    let mut alias = String::new();
    for ch in value.trim().to_lowercase().chars() {
        let part = match ch {
            'а' => "a",
            'б' => "b",
            'в' => "v",
            'г' => "g",
            'д' => "d",
            'е' | 'ё' => "e",
            'ж' => "zh",
            'з' => "z",
            'и' | 'й' => "i",
            'к' => "k",
            'л' => "l",
            'м' => "m",
            'н' => "n",
            'о' => "o",
            'п' => "p",
            'р' => "r",
            'с' => "s",
            'т' => "t",
            'у' => "u",
            'ф' => "f",
            'х' => "h",
            'ц' => "c",
            'ч' => "ch",
            'ш' => "sh",
            'щ' => "sch",
            'ы' => "y",
            'э' => "e",
            'ю' => "yu",
            'я' => "ya",
            'ь' | 'ъ' => "",
            _ if ch.is_ascii_alphanumeric() => {
                alias.push(ch);
                continue;
            }
            _ => "_",
        };
        alias.push_str(part);
    }
    let collapsed = alias
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if collapsed.is_empty() {
        "new_npc".to_string()
    } else {
        collapsed
    }
}

fn optional_i64(value: &str) -> Result<Option<i64>, String> {
    let value = value.trim();
    if value.is_empty() {
        Ok(None)
    } else {
        value
            .parse::<i64>()
            .map(Some)
            .map_err(|err| format!("ID NPC-шаблона: {err}"))
    }
}

fn parse_i64_field(label: &str, value: &str) -> Result<i64, String> {
    value
        .trim()
        .parse::<i64>()
        .map_err(|err| format!("{label}: {err}"))
}

fn parse_usize_field(label: &str, value: &str) -> Result<usize, String> {
    value
        .trim()
        .parse::<usize>()
        .map_err(|err| format!("{label}: {err}"))
}

#[cfg(test)]
mod tests;
