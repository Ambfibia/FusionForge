use super::*;

pub fn analyze_ui_interactions(
    request: &UiInteractionAnalysisRequest,
) -> Result<UiInteractionAnalysis, String> {
    let canonical = serde_json::to_vec(request).map_err(|error| error.to_string())?;
    analyze_ui_interactions_bound(request, &canonical, "canonical-request-json-v1")
}

pub(super) fn analyze_ui_interactions_bound(
    request: &UiInteractionAnalysisRequest,
    request_bytes: &[u8],
    request_hash_mode: &str,
) -> Result<UiInteractionAnalysis, String> {
    request.validate()?;
    let (authority, payload) = load_managed_authority(&request.managed)?;
    let context = AssemblyContext::from_payload(&payload, &authority)?;

    let mut root_rows = Vec::new();
    let mut roots = Vec::new();
    for selector in &request.roots {
        let (row, exact) = resolve_exact_method(&context, selector)?;
        root_rows.push(row);
        roots.push(exact);
    }

    let mut queue = VecDeque::new();
    let mut queued_depth = HashMap::<u32, usize>::new();
    for row in root_rows {
        queue.push_back((row, 0usize));
        queued_depth.entry(row).or_insert(0);
    }

    let mut methods = Vec::new();
    let mut direct_calls = Vec::new();
    let mut limitations = global_limitations();
    let mut visited = BTreeSet::new();
    while let Some((method_row, depth)) = queue.pop_front() {
        if !visited.insert(method_row) {
            continue;
        }
        if methods.len() >= request.traversal.max_methods {
            limitations.push(EvidenceLimitation {
                code: "methodLimitReached".to_string(),
                detail: format!(
                    "analysis stopped at configured maxMethods={}",
                    request.traversal.max_methods
                ),
                level: EvidenceLevel::Unresolved,
            });
            break;
        }
        let (evidence, callees) = analyze_method(&context, method_row)?;
        for (il_offset, callee) in callees {
            direct_calls.push(DirectCallEdge {
                caller_method_token: metadata_token(0x06, method_row),
                call_il_offset: il_offset,
                callee_method_token: metadata_token(0x06, callee),
                depth: depth + 1,
                certainty: EvidenceLevel::StaticProven,
            });
            if depth < request.traversal.max_direct_call_depth {
                let next_depth = depth + 1;
                let should_queue = match queued_depth.get(&callee) {
                    Some(known) => next_depth < *known,
                    None => true,
                };
                if should_queue {
                    queued_depth.insert(callee, next_depth);
                    queue.push_back((callee, next_depth));
                }
            }
        }
        methods.push(evidence);
    }

    methods.sort_by_key(|method| parse_metadata_token_row(&method.method.method_token));
    direct_calls.sort_by_key(|edge| {
        (
            parse_metadata_token_row(&edge.caller_method_token),
            edge.call_il_offset,
            parse_metadata_token_row(&edge.callee_method_token),
        )
    });
    let candidate = build_editor_candidate(&authority, &methods);
    Ok(UiInteractionAnalysis {
        evidence: UiInteractionEvidence {
            schema: EVIDENCE_SCHEMA,
            schema_version: 1,
            request_sha256: sha256_hex_lower(request_bytes),
            request_bytes: request_bytes.len() as u64,
            request_hash_mode: request_hash_mode.to_string(),
            traversal: request.traversal.clone(),
            authority,
            roots,
            methods,
            direct_calls,
            limitations,
            runtime_behavior_proven: false,
        },
        candidate,
    })
}

pub(super) fn analyze_method(
    context: &AssemblyContext,
    method_row: u32,
) -> Result<(MethodEvidence, Vec<(u32, u32)>), String> {
    let method = exact_method_for_row(context, method_row)?;
    if method.rva == 0 {
        return Ok((
            MethodEvidence {
                method,
                body_sha256: sha256_hex(&[]),
                body_bytes: 0,
                certainty: EvidenceLevel::Unresolved,
                cfg: None,
                calls: Vec::new(),
                field_writes: Vec::new(),
                result_branches: Vec::new(),
                call_order: Vec::new(),
                unresolved: vec![EvidenceLimitation {
                    code: "methodHasNoBody".to_string(),
                    detail: "MethodDef RVA is zero (abstract, runtime, or external body)"
                        .to_string(),
                    level: EvidenceLevel::Unresolved,
                }],
            },
            Vec::new(),
        ));
    }
    let body = read_method_body(&context.pe, method.rva)?;
    let instructions = decode_il(&body.code)?;
    let mut unresolved = Vec::new();
    let cfg_internal = if body.has_extra_sections {
        unresolved.push(EvidenceLimitation {
            code: "exceptionRegionsNotModeled".to_string(),
            detail: "method has extra sections; v1 does not claim a complete CFG across EH regions"
                .to_string(),
            level: EvidenceLevel::Unresolved,
        });
        None
    } else {
        Some(build_cfg(&instructions, &body.code)?)
    };
    let mut calls = Vec::new();
    let mut direct_callees = Vec::new();
    let mut field_writes = Vec::new();
    for (index, instruction) in instructions.iter().enumerate() {
        let block = cfg_internal
            .as_ref()
            .and_then(|cfg| cfg.instruction_to_block.get(&instruction.offset).copied())
            .unwrap_or(0);
        if is_call_opcode(instruction.opcode) {
            let token = instruction_u32_operand(instruction, &body.code)?;
            match resolve_method_token(context, token) {
                Ok(resolved) => {
                    let virtual_reference = instruction.opcode == IlOpcode::Single(0x6f);
                    let method_def_row = resolved.method_def_row;
                    let mut classified = classify_api(&resolved.identity);
                    if classified.category == CallCategory::Other && method_def_row.is_some() {
                        classified.category = if virtual_reference {
                            CallCategory::VirtualManagedReference
                        } else {
                            CallCategory::DirectManaged
                        };
                    }
                    calls.push(CallSiteEvidence {
                        site_id: site_id(method_row, instruction.offset),
                        il_offset: instruction.offset as u32,
                        basic_block: block,
                        opcode: opcode_label(instruction.opcode),
                        referenced_member: resolved.identity,
                        dispatch: call_dispatch_kind(instruction.opcode),
                        runtime_target_proven: !virtual_reference,
                        category: classified.category,
                        interaction: classified.interaction,
                        certainty: EvidenceLevel::StaticProven,
                        runtime_execution_proven: false,
                    });
                    if virtual_reference {
                        unresolved.push(EvidenceLimitation {
                            code: "virtualDispatchTargetUnresolved".to_string(),
                            detail: format!(
                                "IL_{:04X} callvirt token 0x{token:08X} proves only a metadata reference; v1 does not prove or traverse the runtime target",
                                instruction.offset
                            ),
                            level: EvidenceLevel::Unresolved,
                        });
                    }
                    if should_traverse_call(instruction.opcode) {
                        if let Some(callee) = method_def_row {
                            direct_callees.push((instruction.offset as u32, callee));
                        }
                    }
                }
                Err(detail) => unresolved.push(EvidenceLimitation {
                    code: "callTargetUnresolved".to_string(),
                    detail: format!(
                        "IL_{:04X} token 0x{token:08X}: {detail}",
                        instruction.offset
                    ),
                    level: EvidenceLevel::Unresolved,
                }),
            }
        } else if matches!(instruction.opcode, IlOpcode::Single(0x7d | 0x80)) {
            let token = instruction_u32_operand(instruction, &body.code)?;
            match resolve_field_token(context, token) {
                Ok(field) => field_writes.push(FieldWriteEvidence {
                    site_id: field_site_id(method_row, instruction.offset),
                    il_offset: instruction.offset as u32,
                    basic_block: block,
                    field,
                    certainty: EvidenceLevel::StaticProven,
                    assigned_value: previous_i32_constant(&instructions, &body.code, index)
                        .map(EvidenceValue::StaticProvenI32)
                        .unwrap_or(EvidenceValue::Unresolved),
                    runtime_execution_proven: false,
                }),
                Err(detail) => unresolved.push(EvidenceLimitation {
                    code: "fieldTargetUnresolved".to_string(),
                    detail: format!(
                        "IL_{:04X} token 0x{token:08X}: {detail}",
                        instruction.offset
                    ),
                    level: EvidenceLevel::Unresolved,
                }),
            }
        }
    }
    let result_branches = cfg_internal
        .as_ref()
        .map(|cfg| direct_result_branches(&calls, &instructions, &body.code, cfg))
        .transpose()?
        .unwrap_or_default();
    let call_order = cfg_internal
        .as_ref()
        .map(|_| call_order_within_blocks(&calls))
        .unwrap_or_default();
    let cfg = cfg_internal.map(|cfg| ControlFlowGraphEvidence {
        entry_block: cfg.blocks.first().map(|block| block.id).unwrap_or(0),
        blocks: cfg.blocks,
        edges: cfg.edges,
        certainty: EvidenceLevel::StaticProven,
    });
    let certainty = if unresolved.is_empty() {
        EvidenceLevel::StaticProven
    } else {
        EvidenceLevel::Unresolved
    };
    Ok((
        MethodEvidence {
            method,
            body_sha256: sha256_hex(&body.code),
            body_bytes: body.code.len() as u64,
            certainty,
            cfg,
            calls,
            field_writes,
            result_branches,
            call_order,
            unresolved,
        },
        direct_callees,
    ))
}

pub(super) fn build_editor_candidate(
    authority: &ManagedAuthority,
    methods: &[MethodEvidence],
) -> UiInteractionCandidate {
    let mut controls = Vec::new();
    let mut side_effects = Vec::new();
    for method in methods {
        for call in &method.calls {
            if is_candidate_control_kind(&call.interaction) {
                controls.push(EditorUiControlCandidate {
                    evidence_site_id: call.site_id.clone(),
                    semantic_id: None,
                    kind: call.interaction.clone(),
                    visibility_gate: EvidenceLevel::Unresolved,
                    enabled_gate: EvidenceLevel::Unresolved,
                    trigger_effects: EvidenceLevel::Candidate,
                });
            }
            if matches!(
                call.interaction,
                InteractionKind::PacketSend
                    | InteractionKind::LocalEventSend
                    | InteractionKind::AudioSideEffect
                    | InteractionKind::PopupSideEffect
            ) {
                side_effects.push(EditorUiSideEffectCandidate {
                    evidence_site_id: call.site_id.clone(),
                    kind: call.interaction.clone(),
                    trigger_control: EvidenceLevel::Unresolved,
                });
            }
        }
    }
    UiInteractionCandidate {
        schema: CANDIDATE_SCHEMA,
        schema_version: 1,
        evidence_payload_sha256: authority.payload_sha256.clone(),
        controls,
        side_effects,
        required_runtime_checks: vec![
            "actual visibility and GUI.enabled predicate values for each Unity event pass",
            "cross-MonoBehaviour OnGUI invocation and draw order",
            "hit testing, overlap priority, clipping, and modal input blocking",
            "GUIUtility.hotControl, keyboardControl, focus, and Event.Use transitions",
            "scroll offsets, GUILayout allocation, and final screen-space rectangles",
            "packet/event arguments, timer thresholds, and externally mutated state",
            "virtual, delegate, reflection, coroutine, and cross-assembly call targets",
        ],
        publication_allowed: false,
    }
}

pub(super) fn is_candidate_control_kind(kind: &InteractionKind) -> bool {
    matches!(
        kind,
        InteractionKind::Button
            | InteractionKind::Toggle
            | InteractionKind::Toolbar
            | InteractionKind::SelectionGrid
            | InteractionKind::Slider
            | InteractionKind::Scrollbar
            | InteractionKind::DragWindow
            | InteractionKind::TextField
            | InteractionKind::TextArea
            | InteractionKind::ScrollViewBegin
            | InteractionKind::Window
    )
}

pub(super) fn global_limitations() -> Vec<EvidenceLimitation> {
    [
        ("staticEvidenceOnly", "IL call sites and CFG edges do not prove that Unity executed a method or branch"),
        ("serializedStateNotJoined", "v1 does not join MonoScript/MonoBehaviour owner mapping, m_Enabled, GameObject active state, or serialized field defaults"),
        ("fieldReadsAndControlDependenciesUnresolved", "v1 records exact writes but does not recover arbitrary field reads, expression dataflow, dominance-based control dependencies, or visibility/enabled gates"),
        ("crossComponentOrderUnknown", "managed IL cannot prove Unity OnGUI order between MonoBehaviours or event passes"),
        ("dynamicDispatchUnknown", "virtual dispatch, delegates, reflection, coroutines, and external assemblies require additional evidence"),
        ("layoutAndPickingUnknown", "IL alone cannot prove GUILayout rectangles, overlap priority, clipping, or pointer hit results"),
    ]
    .into_iter()
    .map(|(code, detail)| EvidenceLimitation {
        code: code.to_string(),
        detail: detail.to_string(),
        level: EvidenceLevel::Unresolved,
    })
    .collect()
}

pub(super) fn single_operand_len(opcode: u8) -> Option<usize> {
    Some(match opcode {
        0x0e..=0x13 | 0x1f | 0x2b..=0x37 | 0xde => 1,
        0x20
        | 0x22
        | 0x27..=0x29
        | 0x38..=0x44
        | 0x6f..=0x75
        | 0x79
        | 0x7b..=0x81
        | 0x8c
        | 0x8d
        | 0x8f
        | 0xa5
        | 0xc2
        | 0xc6
        | 0xd0
        | 0xdd => 4,
        0x21 | 0x23 => 8,
        0x00..=0x0d
        | 0x14..=0x1e
        | 0x25..=0x26
        | 0x2a
        | 0x46..=0x6e
        | 0x76
        | 0x7a
        | 0x82..=0x8b
        | 0x8e
        | 0x90..=0xa4
        | 0xb3..=0xba
        | 0xc3
        | 0xd1..=0xdc
        | 0xdf..=0xe0 => 0,
        _ => return None,
    })
}

pub(super) fn extended_operand_len(opcode: u8) -> Option<usize> {
    Some(match opcode {
        0x12 => 1,
        0x09..=0x0e => 2,
        0x06 | 0x07 | 0x15 | 0x16 | 0x1c => 4,
        0x00..=0x05 | 0x0f | 0x11 | 0x13 | 0x14 | 0x17 | 0x18 | 0x1a | 0x1d | 0x1e => 0,
        _ => return None,
    })
}

pub(super) fn build_cfg(instructions: &[IlInstruction], code: &[u8]) -> Result<InternalCfg, String> {
    if instructions.is_empty() {
        return Ok(InternalCfg {
            blocks: Vec::new(),
            edges: Vec::new(),
            instruction_to_block: BTreeMap::new(),
        });
    }
    let instruction_offsets = instructions
        .iter()
        .map(|instruction| instruction.offset)
        .collect::<BTreeSet<_>>();
    let mut leaders = BTreeSet::from([instructions[0].offset]);
    for instruction in instructions {
        for target in branch_targets(instruction, code)? {
            if !instruction_offsets.contains(&target) {
                return Err(format!(
                    "branch from IL_{:04X} targets non-instruction IL_{target:04X}",
                    instruction.offset
                ));
            }
            leaders.insert(target);
        }
        if is_branch(instruction.opcode) || is_terminator(instruction.opcode) {
            let next = instruction.offset + instruction.size;
            if next < code.len() {
                leaders.insert(next);
            }
        }
    }
    let starts = leaders.into_iter().collect::<Vec<_>>();
    let blocks = starts
        .iter()
        .enumerate()
        .map(|(index, start)| BasicBlockEvidence {
            id: index as u32,
            start_il_offset: *start as u32,
            end_il_offset_exclusive: starts.get(index + 1).copied().unwrap_or(code.len()) as u32,
        })
        .collect::<Vec<_>>();
    let start_to_block = blocks
        .iter()
        .map(|block| (block.start_il_offset as usize, block.id))
        .collect::<BTreeMap<_, _>>();
    let mut instruction_to_block = BTreeMap::new();
    for instruction in instructions {
        let (_, block) = start_to_block
            .range(..=instruction.offset)
            .next_back()
            .ok_or_else(|| format!("IL_{:04X} has no containing block", instruction.offset))?;
        instruction_to_block.insert(instruction.offset, *block);
    }
    let mut edges = Vec::new();
    for block in &blocks {
        let last = instructions
            .iter()
            .rev()
            .find(|instruction| {
                instruction.offset >= block.start_il_offset as usize
                    && instruction.offset < block.end_il_offset_exclusive as usize
            })
            .ok_or_else(|| format!("basic block {} is empty", block.id))?;
        let next = last.offset + last.size;
        if is_switch(last.opcode) {
            for target in branch_targets(last, code)? {
                push_cfg_edge(
                    &mut edges,
                    block.id,
                    block_for_offset(&start_to_block, target)?,
                    ControlFlowEdgeKind::SwitchCase,
                );
            }
            if next < code.len() {
                push_cfg_edge(
                    &mut edges,
                    block.id,
                    block_for_offset(&start_to_block, next)?,
                    ControlFlowEdgeKind::Fallthrough,
                );
            }
        } else if is_unconditional_branch(last.opcode) {
            let target = branch_targets(last, code)?[0];
            push_cfg_edge(
                &mut edges,
                block.id,
                block_for_offset(&start_to_block, target)?,
                ControlFlowEdgeKind::BranchTaken,
            );
        } else if is_conditional_branch(last.opcode) {
            let target = branch_targets(last, code)?[0];
            push_cfg_edge(
                &mut edges,
                block.id,
                block_for_offset(&start_to_block, target)?,
                ControlFlowEdgeKind::BranchTaken,
            );
            if next < code.len() {
                push_cfg_edge(
                    &mut edges,
                    block.id,
                    block_for_offset(&start_to_block, next)?,
                    ControlFlowEdgeKind::Fallthrough,
                );
            }
        } else if !is_terminator(last.opcode) && next < code.len() {
            push_cfg_edge(
                &mut edges,
                block.id,
                block_for_offset(&start_to_block, next)?,
                ControlFlowEdgeKind::Fallthrough,
            );
        }
    }
    edges.sort_by_key(|edge| (edge.from, edge.to, edge_kind_rank(&edge.kind)));
    Ok(InternalCfg {
        blocks,
        edges,
        instruction_to_block,
    })
}

pub(super) fn push_cfg_edge(
    edges: &mut Vec<ControlFlowEdgeEvidence>,
    from: u32,
    to: u32,
    kind: ControlFlowEdgeKind,
) {
    if edges
        .iter()
        .any(|edge| edge.from == from && edge.to == to && edge.kind == kind)
    {
        return;
    }
    edges.push(ControlFlowEdgeEvidence {
        from,
        to,
        kind,
        certainty: EvidenceLevel::StaticProven,
    });
}

pub(super) fn edge_kind_rank(kind: &ControlFlowEdgeKind) -> u8 {
    match kind {
        ControlFlowEdgeKind::Fallthrough => 0,
        ControlFlowEdgeKind::BranchTaken => 1,
        ControlFlowEdgeKind::SwitchCase => 2,
    }
}

pub(super) fn block_for_offset(starts: &BTreeMap<usize, u32>, offset: usize) -> Result<u32, String> {
    starts
        .get(&offset)
        .copied()
        .ok_or_else(|| format!("IL_{offset:04X} is not a basic-block leader"))
}

pub(super) fn branch_targets(instruction: &IlInstruction, code: &[u8]) -> Result<Vec<usize>, String> {
    let base = instruction.offset + instruction.size;
    let operand = code
        .get(instruction.operand_offset..instruction.operand_offset + instruction.operand_len)
        .ok_or_else(|| format!("truncated branch at IL_{:04X}", instruction.offset))?;
    match instruction.opcode {
        IlOpcode::Single(0x2b..=0x37 | 0xde) => Ok(vec![checked_branch_target(
            base,
            i8::from_le_bytes([operand[0]]) as i64,
            code.len(),
            instruction.offset,
        )?]),
        IlOpcode::Single(0x38..=0x44 | 0xdd) => Ok(vec![checked_branch_target(
            base,
            i32::from_le_bytes([operand[0], operand[1], operand[2], operand[3]]) as i64,
            code.len(),
            instruction.offset,
        )?]),
        IlOpcode::Single(0x45) => {
            let count = read_u32_le(operand, 0)? as usize;
            if operand.len() != 4 + count * 4 {
                return Err(format!("truncated switch at IL_{:04X}", instruction.offset));
            }
            let mut targets = Vec::with_capacity(count);
            for index in 0..count {
                let offset = 4 + index * 4;
                targets.push(checked_branch_target(
                    base,
                    i32::from_le_bytes([
                        operand[offset],
                        operand[offset + 1],
                        operand[offset + 2],
                        operand[offset + 3],
                    ]) as i64,
                    code.len(),
                    instruction.offset,
                )?);
            }
            Ok(targets)
        }
        _ => Ok(Vec::new()),
    }
}

pub(super) fn checked_branch_target(
    base: usize,
    relative: i64,
    code_len: usize,
    source: usize,
) -> Result<usize, String> {
    let target = base as i64 + relative;
    if target < 0 || target as usize >= code_len {
        return Err(format!(
            "branch at IL_{source:04X} targets out-of-range IL_{target:04X}"
        ));
    }
    Ok(target as usize)
}

pub(super) fn direct_result_branches(
    calls: &[CallSiteEvidence],
    instructions: &[IlInstruction],
    code: &[u8],
    cfg: &InternalCfg,
) -> Result<Vec<ResultBranchEvidence>, String> {
    let calls_by_offset = calls
        .iter()
        .map(|call| (call.il_offset as usize, call))
        .collect::<BTreeMap<_, _>>();
    let mut result = Vec::new();
    for pair in instructions.windows(2) {
        let call_instruction = &pair[0];
        let branch = &pair[1];
        let Some(call) = calls_by_offset.get(&call_instruction.offset).copied() else {
            continue;
        };
        if !matches!(
            call.interaction,
            InteractionKind::Button | InteractionKind::Toggle
        ) || call.referenced_member.return_type != "System.Boolean"
        {
            continue;
        }
        let false_branch = matches!(branch.opcode, IlOpcode::Single(0x2c | 0x39));
        let true_branch = matches!(branch.opcode, IlOpcode::Single(0x2d | 0x3a));
        if !false_branch && !true_branch {
            continue;
        }
        let target = branch_targets(branch, code)?[0];
        let fallthrough = branch.offset + branch.size;
        let target_block = *cfg
            .instruction_to_block
            .get(&target)
            .ok_or_else(|| format!("branch target IL_{target:04X} has no basic block"))?;
        let fallthrough_block = *cfg
            .instruction_to_block
            .get(&fallthrough)
            .ok_or_else(|| format!("fallthrough IL_{fallthrough:04X} has no basic block"))?;
        let (true_successor, false_successor) = if false_branch {
            (fallthrough_block, target_block)
        } else {
            (target_block, fallthrough_block)
        };
        result.push(ResultBranchEvidence {
            predicate_site_id: call.site_id.clone(),
            branch_il_offset: branch.offset as u32,
            true_successor,
            false_successor,
            certainty: EvidenceLevel::StaticProven,
            proof: "adjacent boolean-return call followed by brtrue/brfalse",
        });
    }
    Ok(result)
}

pub(super) fn call_order_within_blocks(calls: &[CallSiteEvidence]) -> Vec<CallOrderEvidence> {
    let mut by_block = BTreeMap::<u32, Vec<&CallSiteEvidence>>::new();
    for call in calls {
        by_block.entry(call.basic_block).or_default().push(call);
    }
    let mut result = Vec::new();
    for (block, mut block_calls) in by_block {
        block_calls.sort_by_key(|call| call.il_offset);
        for pair in block_calls.windows(2) {
            result.push(CallOrderEvidence {
                before_site_id: pair[0].site_id.clone(),
                after_site_id: pair[1].site_id.clone(),
                basic_block: block,
                certainty: EvidenceLevel::StaticProven,
                proof: "increasing IL offset within one basic block",
            });
        }
    }
    result
}

pub(super) fn previous_i32_constant(instructions: &[IlInstruction], code: &[u8], index: usize) -> Option<i32> {
    let previous = instructions.get(index.checked_sub(1)?)?;
    match previous.opcode {
        IlOpcode::Single(0x15) => Some(-1),
        IlOpcode::Single(value @ 0x16..=0x1e) => Some((value - 0x16) as i32),
        IlOpcode::Single(0x1f) => code
            .get(previous.operand_offset)
            .copied()
            .map(|value| i8::from_le_bytes([value]) as i32),
        IlOpcode::Single(0x20) => {
            let value = code.get(previous.operand_offset..previous.operand_offset + 4)?;
            Some(i32::from_le_bytes([value[0], value[1], value[2], value[3]]))
        }
        _ => None,
    }
}

pub(super) fn should_traverse_call(opcode: IlOpcode) -> bool {
    opcode == IlOpcode::Single(0x28)
}

pub(super) fn call_dispatch_kind(opcode: IlOpcode) -> CallDispatchKind {
    match opcode {
        IlOpcode::Single(0x28) => CallDispatchKind::Direct,
        IlOpcode::Single(0x6f) => CallDispatchKind::VirtualReference,
        IlOpcode::Single(0x73) => CallDispatchKind::Constructor,
        _ => CallDispatchKind::VirtualReference,
    }
}

pub(super) fn is_call_opcode(opcode: IlOpcode) -> bool {
    matches!(opcode, IlOpcode::Single(0x28 | 0x6f | 0x73))
}

pub(super) fn is_switch(opcode: IlOpcode) -> bool {
    opcode == IlOpcode::Single(0x45)
}

pub(super) fn is_unconditional_branch(opcode: IlOpcode) -> bool {
    matches!(opcode, IlOpcode::Single(0x2b | 0x38 | 0xdd | 0xde))
}

pub(super) fn is_conditional_branch(opcode: IlOpcode) -> bool {
    matches!(opcode, IlOpcode::Single(0x2c..=0x37 | 0x39..=0x44))
}

pub(super) fn is_branch(opcode: IlOpcode) -> bool {
    is_switch(opcode) || is_unconditional_branch(opcode) || is_conditional_branch(opcode)
}

pub(super) fn is_terminator(opcode: IlOpcode) -> bool {
    matches!(
        opcode,
        IlOpcode::Single(0x27 | 0x2a | 0x7a | 0xdc) | IlOpcode::Extended(0x11 | 0x1a)
    ) || is_unconditional_branch(opcode)
}

pub(super) fn instruction_u32_operand(instruction: &IlInstruction, code: &[u8]) -> Result<u32, String> {
    if instruction.operand_len != 4 {
        return Err(format!(
            "IL_{:04X} expected a four-byte metadata token",
            instruction.offset
        ));
    }
    read_u32_le(code, instruction.operand_offset)
}

pub(super) fn opcode_label(opcode: IlOpcode) -> String {
    match opcode {
        IlOpcode::Single(0x28) => "call".to_string(),
        IlOpcode::Single(0x6f) => "callvirt".to_string(),
        IlOpcode::Single(0x73) => "newobj".to_string(),
        IlOpcode::Single(value) => format!("0x{value:02X}"),
        IlOpcode::Extended(value) => format!("0xFE{value:02X}"),
    }
}
