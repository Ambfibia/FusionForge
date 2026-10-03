use super::*;

#[test]
fn custom_draft_builds_supported_blueprint_without_spawn_or_audio_promises() {
    let mut draft = NpcEditorDraft::default();
    draft.name = "Test NPC".to_string();
    draft.internal_name = "test_npc".to_string();
    draft.authoring_model_path = "D:/models/test.glb".to_string();
    draft.model_asset = "mob/test_npc.kfm".to_string();
    let blueprint = draft.to_blueprint().expect("blueprint");
    assert_eq!(
        blueprint.model_bundle.as_deref(),
        Some("D:/models/test.glb")
    );
    assert_eq!(
        blueprint.authoring_model_path.as_deref(),
        Some("D:/models/test.glb")
    );
    assert!(blueprint.audio_source.is_none());
    assert!(blueprint.spawn_map.is_none());
    assert_eq!(blueprint.profile.get("m_iTeam"), Some(&json!(1)));
}

#[test]
fn advanced_profile_must_be_an_object() {
    let mut draft = NpcEditorDraft::default();
    draft.advanced_profile_json = "[]".to_string();
    assert!(draft
        .to_blueprint()
        .expect_err("array must fail")
        .contains("JSON-объектом"));
}

#[test]
fn workspace_breakpoints_cover_windowed_16_by_9_and_16_by_10() {
    assert_eq!(
        npc_workspace_layout(1670.0, 940.0),
        NpcWorkspaceLayout::Wide
    );
    assert_eq!(
        npc_workspace_layout(1350.0, 720.0),
        NpcWorkspaceLayout::Wide
    );
    assert_eq!(
        npc_workspace_layout(1350.0, 640.0),
        NpcWorkspaceLayout::Medium
    );
    assert_eq!(
        npc_workspace_layout(1030.0, 740.0),
        NpcWorkspaceLayout::Medium
    );
    assert_eq!(
        npc_workspace_layout(780.0, 700.0),
        NpcWorkspaceLayout::Compact
    );
}

#[test]
fn wizard_generates_stable_alias_from_russian_name() {
    assert_eq!(npc_alias_from_name("Механик Эдди"), "mehanik_eddi");
}
