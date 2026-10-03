use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagedAssemblyInput {
    pub evidence_report: PathBuf,
    /// Required for a portable materialization.  Embedded-base64 evidence can omit this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagedMethodSelector {
    pub declaring_type: String,
    pub name: String,
    pub parameter_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct TraversalLimits {
    pub max_direct_call_depth: usize,
    pub max_methods: usize,
}

impl Default for TraversalLimits {
    fn default() -> Self {
        Self {
            max_direct_call_depth: 1,
            max_methods: 512,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiInteractionAnalysis {
    pub evidence: UiInteractionEvidence,
    pub candidate: UiInteractionCandidate,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiInteractionEvidence {
    pub schema: &'static str,
    pub schema_version: u32,
    pub request_sha256: String,
    pub request_bytes: u64,
    pub request_hash_mode: String,
    pub traversal: TraversalLimits,
    pub authority: ManagedAuthority,
    pub roots: Vec<ExactManagedMethod>,
    pub methods: Vec<MethodEvidence>,
    pub direct_calls: Vec<DirectCallEdge>,
    pub limitations: Vec<EvidenceLimitation>,
    pub runtime_behavior_proven: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedAuthority {
    pub evidence_report_bytes: u64,
    pub evidence_report_sha256: String,
    pub source_alias: String,
    pub raw_container_relative_path: String,
    pub raw_container_sha256: String,
    pub selected_level: usize,
    pub selected_exact_entry: String,
    pub selected_route: String,
    pub payload_bytes: u64,
    pub payload_sha256: String,
    pub clr_metadata_bytes: u64,
    pub clr_metadata_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExactManagedMethod {
    pub declaring_type: String,
    pub type_token: String,
    pub name: String,
    pub parameter_types: Vec<String>,
    pub return_type: String,
    pub method_token: String,
    pub rva: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MethodEvidence {
    pub method: ExactManagedMethod,
    pub body_sha256: String,
    pub body_bytes: u64,
    pub certainty: EvidenceLevel,
    pub cfg: Option<ControlFlowGraphEvidence>,
    pub calls: Vec<CallSiteEvidence>,
    pub field_writes: Vec<FieldWriteEvidence>,
    pub result_branches: Vec<ResultBranchEvidence>,
    pub call_order: Vec<CallOrderEvidence>,
    pub unresolved: Vec<EvidenceLimitation>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceLevel {
    StaticProven,
    Candidate,
    Unresolved,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CallCategory {
    Control,
    ScrollScope,
    LayoutScope,
    Draw,
    GuiState,
    EventState,
    Focus,
    Input,
    Time,
    Packet,
    LocalEvent,
    Audio,
    Popup,
    DirectManaged,
    VirtualManagedReference,
    Other,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum InteractionKind {
    Button,
    Toggle,
    Toolbar,
    SelectionGrid,
    Slider,
    Scrollbar,
    TextField,
    TextArea,
    ScrollViewBegin,
    ScrollViewEnd,
    GroupBegin,
    GroupEnd,
    HorizontalScopeBegin,
    HorizontalScopeEnd,
    VerticalScopeBegin,
    VerticalScopeEnd,
    AreaBegin,
    AreaEnd,
    Window,
    DragWindow,
    ControlId,
    ExitGui,
    EventTypeForControl,
    FocusControl,
    NextControlName,
    EventCurrent,
    EventUse,
    EventRead,
    HotControl,
    KeyboardControl,
    KeyPoll,
    MousePoll,
    AxisPoll,
    GuiEnabledWrite,
    GuiChangedWrite,
    GuiVisualStateWrite,
    TimeRead,
    Draw,
    PacketSend,
    LocalEventSend,
    AudioSideEffect,
    PopupSideEffect,
    Other,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedMemberIdentity {
    pub declaring_type: String,
    pub name: String,
    pub parameter_types: Vec<String>,
    pub return_type: String,
    pub metadata_token: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CallDispatchKind {
    Direct,
    VirtualReference,
    Constructor,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallSiteEvidence {
    pub site_id: String,
    pub il_offset: u32,
    pub basic_block: u32,
    pub opcode: String,
    pub referenced_member: ManagedMemberIdentity,
    pub dispatch: CallDispatchKind,
    pub runtime_target_proven: bool,
    pub category: CallCategory,
    pub interaction: InteractionKind,
    pub certainty: EvidenceLevel,
    pub runtime_execution_proven: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedFieldIdentity {
    pub declaring_type: String,
    pub name: String,
    pub metadata_token: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", content = "value", rename_all = "camelCase")]
pub enum EvidenceValue {
    StaticProvenI32(i32),
    CandidateI32(i32),
    Unresolved,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultBranchEvidence {
    pub predicate_site_id: String,
    pub branch_il_offset: u32,
    pub true_successor: u32,
    pub false_successor: u32,
    pub certainty: EvidenceLevel,
    pub proof: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallOrderEvidence {
    pub before_site_id: String,
    pub after_site_id: String,
    pub basic_block: u32,
    pub certainty: EvidenceLevel,
    pub proof: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlFlowGraphEvidence {
    pub entry_block: u32,
    pub blocks: Vec<BasicBlockEvidence>,
    pub edges: Vec<ControlFlowEdgeEvidence>,
    pub certainty: EvidenceLevel,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BasicBlockEvidence {
    pub id: u32,
    pub start_il_offset: u32,
    pub end_il_offset_exclusive: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlFlowEdgeEvidence {
    pub from: u32,
    pub to: u32,
    pub kind: ControlFlowEdgeKind,
    pub certainty: EvidenceLevel,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ControlFlowEdgeKind {
    Fallthrough,
    BranchTaken,
    SwitchCase,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectCallEdge {
    pub caller_method_token: String,
    pub call_il_offset: u32,
    pub callee_method_token: String,
    pub depth: usize,
    pub certainty: EvidenceLevel,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceLimitation {
    pub code: String,
    pub detail: String,
    pub level: EvidenceLevel,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiInteractionCandidate {
    pub schema: &'static str,
    pub schema_version: u32,
    pub evidence_payload_sha256: String,
    pub controls: Vec<EditorUiControlCandidate>,
    pub side_effects: Vec<EditorUiSideEffectCandidate>,
    pub required_runtime_checks: Vec<&'static str>,
    pub publication_allowed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorUiControlCandidate {
    pub evidence_site_id: String,
    pub semantic_id: Option<String>,
    pub kind: InteractionKind,
    pub visibility_gate: EvidenceLevel,
    pub enabled_gate: EvidenceLevel,
    pub trigger_effects: EvidenceLevel,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorUiSideEffectCandidate {
    pub evidence_site_id: String,
    pub kind: InteractionKind,
    pub trigger_control: EvidenceLevel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ManagedEvidenceDocument {
    pub(super) schema: String,
    pub(super) schema_version: u32,
    pub(super) source: ManagedSourceDocument,
    pub(super) selection: ManagedSelectionDocument,
    pub(super) managed_format: ManagedFormatDocument,
    pub(super) payload: ManagedPayloadDocument,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ManagedSourceDocument {
    pub(super) alias: String,
    pub(super) raw_container: ManagedRawContainerDocument,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ManagedFormatDocument {
    pub(super) clr_metadata_bytes: u64,
    pub(super) clr_metadata_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ManagedMaterializationDocument {
    pub(super) mode: String,
    pub(super) base64: Option<String>,
}

pub(super) struct AssemblyContext {
    pub(super) pe: PE,
    pub(super) metadata: Metadata,
    pub(super) type_names: Vec<String>,
    pub(super) method_owners: Vec<u32>,
    pub(super) field_owners: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IlOpcode {
    Single(u8),
    Extended(u8),
}

#[derive(Debug, Clone)]
pub(super) struct IlInstruction {
    pub(super) offset: usize,
    pub(super) opcode: IlOpcode,
    pub(super) operand_offset: usize,
    pub(super) operand_len: usize,
    pub(super) size: usize,
}

#[derive(Debug, Clone)]
pub(super) struct DecodedMethodBody {
    pub(super) code: Vec<u8>,
    pub(super) has_extra_sections: bool,
}

#[derive(Debug, Clone)]
pub(super) struct InternalCfg {
    pub(super) blocks: Vec<BasicBlockEvidence>,
    pub(super) edges: Vec<ControlFlowEdgeEvidence>,
    pub(super) instruction_to_block: BTreeMap<usize, u32>,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedMethod {
    pub(super) identity: ManagedMemberIdentity,
    pub(super) method_def_row: Option<u32>,
}

#[derive(Debug, Clone)]
pub(super) struct ClassifiedCall {
    pub(super) category: CallCategory,
    pub(super) interaction: InteractionKind,
    pub(super) boolean_result: bool,
}

impl ManagedEvidenceDocument {
    pub(super) fn materialization_is_embedded(&self) -> Result<bool, String> {
        match self.payload.materialization.mode.as_str() {
            "embedded-base64" => Ok(true),
            "portable-relative-output" => Ok(false),
            mode => Err(format!(
                "unsupported managed payload materialization mode '{mode}'"
            )),
        }
    }
}

impl AssemblyContext {
    pub(super) fn from_payload(payload: &[u8], authority: &ManagedAuthority) -> Result<Self, String> {
        let pe =
            PE::parse(payload).map_err(|error| format!("managed payload is not PE: {error}"))?;
        if !pe.is_dll() {
            return Err("managed payload is not a PE DLL".to_string());
        }
        let cli = read_cli_header(&pe)?;
        let metadata_bytes = pe
            .read_at_rva(cli.metadata_rva, cli.metadata_size as usize)
            .ok_or_else(|| {
                format!(
                    "CLR metadata RVA 0x{:x} size {} is out of range",
                    cli.metadata_rva, cli.metadata_size
                )
            })?;
        require_bytes_and_hash(
            "CLR metadata",
            metadata_bytes,
            authority.clr_metadata_bytes,
            &authority.clr_metadata_sha256,
        )?;
        let metadata = Metadata::parse(metadata_bytes)
            .map_err(|error| format!("could not parse CLR metadata: {error}"))?;
        let type_names = build_type_names(&metadata);
        let method_owners = build_method_owners(&metadata);
        let field_owners = build_field_owners(&metadata);
        Ok(Self {
            pe,
            metadata,
            type_names,
            method_owners,
            field_owners,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CliHeader {
    pub(super) metadata_rva: u32,
    pub(super) metadata_size: u32,
}
