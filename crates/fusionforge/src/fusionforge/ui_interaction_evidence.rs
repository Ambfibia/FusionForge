// Fail-closed, metadata-token based evidence for legacy Unity IMGUI interactions.
//
// This module deliberately does not inspect decompiled C# text.  A site is emitted only when
// its CLR method body and metadata token can be read from a payload whose bytes are tied to an
// `fftools.managed-assembly-evidence.v1` envelope.  `staticProven` means only that the IL/CFG
// fact exists in those exact bytes; it never means that Unity executed the path at runtime.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    fs,
    path::PathBuf,
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use clrmeta::{ElementType, Metadata, MethodDefRow, MethodSig, TableId, TypeSig};
use portex::{data_dir::DataDirectoryType, PE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use std::{
    env,
    io::{self, Write},
    path::{Component, Path},
};


#[cfg(test)]
mod tests;

mod commands;
mod constants;
mod types;
mod output;
mod containers;
mod state;
mod codec;
mod operations_analyze_method;
mod operations_known_imgui_overload;
mod operations_portable_relative_parts;
mod input;
mod validation;

pub use commands::UiInteractionAnalysisRequest;
pub(crate) use commands::analyze_ui_interactions_with_request_bytes;
pub use constants::{EVIDENCE_SCHEMA, CANDIDATE_SCHEMA};
use constants::{MANAGED_EVIDENCE_SCHEMA, CLI_USAGE};
pub use types::{
    ManagedAssemblyInput, ManagedMethodSelector, TraversalLimits, UiInteractionAnalysis,
    UiInteractionEvidence, ManagedAuthority, ExactManagedMethod, MethodEvidence,
    EvidenceLevel, CallCategory, InteractionKind, ManagedMemberIdentity, CallDispatchKind,
    CallSiteEvidence, ManagedFieldIdentity, EvidenceValue, ResultBranchEvidence,
    CallOrderEvidence, ControlFlowGraphEvidence, BasicBlockEvidence, ControlFlowEdgeEvidence,
    ControlFlowEdgeKind, DirectCallEdge, EvidenceLimitation, UiInteractionCandidate,
    EditorUiControlCandidate, EditorUiSideEffectCandidate
};
use types::{
    ManagedEvidenceDocument,
    ManagedMaterializationDocument, AssemblyContext, IlOpcode, IlInstruction,
    DecodedMethodBody, InternalCfg, ResolvedMethod, ClassifiedCall, CliHeader
};
pub use output::FieldWriteEvidence;
pub(crate) use output::export_ui_interaction_evidence_cli;
use containers::ManagedRawContainerDocument;
use state::ManagedSelectionDocument;
use codec::{ManagedPayloadDocument, decode_il};
use operations_analyze_method::{
    analyze_ui_interactions_bound, single_operand_len, extended_operand_len
};
use operations_known_imgui_overload::{
    exact_method_for_row, method_signature, member_ref_owner_full_name, classify_api, build_type_names, build_method_owners,
    build_field_owners, format_type_sig, normalize_sha256,
    sha256_hex_lower, sha256_hex, metadata_token, site_id, field_site_id
};
use operations_portable_relative_parts::{
    portable_relative_parts, preflight_output
};
use input::{
    load_managed_authority, read_method_body, read_cli_header, resolve_exact_method,
    resolve_method_token, resolve_field_token, parse_metadata_token_row, read_u32_le,
    resolve_portable_input
};
use validation::{require_bytes_and_hash, validate_sha256};
#[cfg(test)]
pub use commands::REQUEST_SCHEMA;
#[cfg(test)]
use operations_analyze_method::{is_candidate_control_kind, global_limitations, build_cfg, direct_result_branches, previous_i32_constant, should_traverse_call, call_dispatch_kind};
