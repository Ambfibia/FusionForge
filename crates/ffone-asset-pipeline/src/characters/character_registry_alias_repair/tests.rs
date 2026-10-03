use super::*;

#[test]
fn canonical_package_still_adopts_exact_table_case_and_preserves_existing_root() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write_registry(root, &registry_json(&[
        ("mob/mob_queenspider", "mob_queenspider",
         "characters/mobs/mob_queenspider/mob_queenspider.glb", &[]),
        ("fusion/fusion_spidermonkey", "fusion_Spidermonkey",
         "characters/fusions/fusion_spidermonkey/fusion_Spidermonkey.glb", &[]),
    ]));
    write_table_set(root, &["mob_queenSpider", "fusion_spidermonkey"]);
    let report = repair_character_registry_aliases(
        &CharacterRegistryAliasRepairOptions::new(root, true)).unwrap();
    assert_eq!(report.changes.len(), 2);
    let registry = plan_registry(root);
    assert_eq!(registry.models[0].legacy_aliases, ["mob_queenSpider"]);
    assert_eq!(registry.models[1].legacy_aliases, ["fusion_Spidermonkey", "fusion_spidermonkey"]);
    let before = fs::read(root.join("_runtime/characters.json")).unwrap();
    let replay = repair_character_registry_aliases(
        &CharacterRegistryAliasRepairOptions::new(root, true)).unwrap();
    assert!(replay.changes.is_empty());
    assert_eq!(fs::read(root.join("_runtime/characters.json")).unwrap(), before);
}

fn write_plan(root: &Path, entries: &[(&str, &str)]) -> PathBuf {
    let plan = CharacterRegistryAliasPlan {
        schema: CHARACTER_REGISTRY_ALIAS_PLAN_SCHEMA.to_owned(),
        remove_aliases: Vec::new(),
        aliases: entries
            .iter()
            .map(|(logical, alias)| CharacterRegistryAliasPlanEntry {
                logical_name: (*logical).to_owned(),
                alias: (*alias).to_owned(),
                evidence: "modded snapshot imported this model".to_owned(),
            })
            .collect(),
    };
    let path = root.join("alias-plan.json");
    fs::write(&path, serde_json::to_vec_pretty(&plan).unwrap()).unwrap();
    path
}

fn plan_registry(root: &Path) -> SemanticCharacterRegistry {
    serde_json::from_slice(&fs::read(root.join("_runtime/characters.json")).unwrap()).unwrap()
}

#[test]
fn adds_a_stated_table_alias_to_an_already_installed_model() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write_registry(
        root,
        &registry_json(&[(
            "npc/npc_tom",
            "npc_tom",
            "characters/npcs/npc_tom/npc_tom.glb",
            &[],
        )]),
    );
    write_table_set(root, &["npc_tom", "npc_3399_tom"]);
    let plan = write_plan(root, &[("npc_tom", "npc_3399_tom")]);

    let report = apply_character_registry_alias_plan(&CharacterRegistryAliasPlanOptions::new(
        root, &plan, true,
    ))
    .unwrap();

    assert!(report.applied);
    assert_eq!(report.planned, 1);
    assert_eq!(report.changes.len(), 1);
    assert_eq!(report.changes[0].added_aliases, vec!["npc_3399_tom"]);
    assert_eq!(
        plan_registry(root).models[0].legacy_aliases,
        vec!["npc_3399_tom".to_owned()]
    );
}

#[test]
fn moves_an_alias_between_models_in_one_transaction() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write_registry(
        root,
        &registry_json(&[
            (
                "npc/npc_3449_vilgax",
                "npc_vilgax",
                "characters/npcs/npc_3449_vilgax/npc_vilgax.glb",
                &[],
            ),
            (
                "shared/npc_vilgax",
                "npc_vilgax_shared",
                "characters/shared/npc_vilgax/npc_vilgax_shared.glb",
                &["npc_3449_vilgax"],
            ),
        ]),
    );
    write_table_set(root, &["npc_vilgax", "npc_3449_vilgax"]);
    let plan = CharacterRegistryAliasPlan {
        schema: CHARACTER_REGISTRY_ALIAS_PLAN_SCHEMA.to_owned(),
        aliases: vec![CharacterRegistryAliasPlanEntry {
            logical_name: "npc_vilgax".to_owned(),
            alias: "npc_3449_vilgax".to_owned(),
            evidence: "3449 ships its own model".to_owned(),
        }],
        remove_aliases: vec![CharacterRegistryAliasPlanEntry {
            logical_name: "npc_vilgax_shared".to_owned(),
            alias: "npc_3449_vilgax".to_owned(),
            evidence: "the shared model is a different character".to_owned(),
        }],
    };
    let path = root.join("move-plan.json");
    fs::write(&path, serde_json::to_vec_pretty(&plan).unwrap()).unwrap();

    let report = apply_character_registry_alias_plan(&CharacterRegistryAliasPlanOptions::new(
        root, &path, true,
    ))
    .unwrap();

    assert!(report.applied);
    assert_eq!(report.removals.len(), 1);
    assert_eq!(report.changes.len(), 1);
    let registry = plan_registry(root);
    let moved_to = registry
        .models
        .iter()
        .find(|model| model.logical_name == "npc_vilgax")
        .unwrap();
    let moved_from = registry
        .models
        .iter()
        .find(|model| model.logical_name == "npc_vilgax_shared")
        .unwrap();
    assert_eq!(moved_to.legacy_aliases, vec!["npc_3449_vilgax".to_owned()]);
    assert!(moved_from.legacy_aliases.is_empty());
}

#[test]
fn refuses_removing_an_alias_the_named_model_does_not_carry() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write_registry(
        root,
        &registry_json(&[(
            "npc/npc_tom",
            "npc_tom",
            "characters/npcs/npc_tom/npc_tom.glb",
            &[],
        )]),
    );
    write_table_set(root, &["npc_tom", "npc_3399_tom"]);
    let plan = CharacterRegistryAliasPlan {
        schema: CHARACTER_REGISTRY_ALIAS_PLAN_SCHEMA.to_owned(),
        aliases: Vec::new(),
        remove_aliases: vec![CharacterRegistryAliasPlanEntry {
            logical_name: "npc_tom".to_owned(),
            alias: "npc_3399_tom".to_owned(),
            evidence: "stale".to_owned(),
        }],
    };
    let path = root.join("bad-remove.json");
    fs::write(&path, serde_json::to_vec_pretty(&plan).unwrap()).unwrap();

    let error = match apply_character_registry_alias_plan(
        &CharacterRegistryAliasPlanOptions::new(root, &path, true),
    ) {
        Ok(_) => panic!("removing an absent alias must be refused"),
        Err(error) => error.to_string(),
    };
    assert!(
        error.contains("is not carried by exactly one model"),
        "{error}"
    );
}

#[test]
fn refuses_an_alias_the_installed_table_set_does_not_name() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write_registry(
        root,
        &registry_json(&[(
            "npc/npc_tom",
            "npc_tom",
            "characters/npcs/npc_tom/npc_tom.glb",
            &[],
        )]),
    );
    write_table_set(root, &["npc_tom"]);
    let plan = write_plan(root, &[("npc_tom", "npc_9999_tom")]);

    let error = match apply_character_registry_alias_plan(
        &CharacterRegistryAliasPlanOptions::new(root, &plan, true),
    ) {
        Ok(_) => panic!("an alias no table row names must be refused"),
        Err(error) => error.to_string(),
    };
    assert!(
        error.contains("no mesh string of the installed table set"),
        "{error}"
    );
    assert!(plan_registry(root).models[0].legacy_aliases.is_empty());
}

#[test]
fn refuses_an_alias_that_already_resolves_to_another_model() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write_registry(
        root,
        &registry_json(&[
            (
                "npc/npc_tom",
                "npc_tom",
                "characters/npcs/npc_tom/npc_tom.glb",
                &[],
            ),
            (
                "npc/npc_clyde",
                "npc_clyde",
                "characters/npcs/npc_clyde/npc_clyde.glb",
                &["npc_3399_tom"],
            ),
        ]),
    );
    write_table_set(root, &["npc_tom", "npc_clyde", "npc_3399_tom"]);
    let plan = write_plan(root, &[("npc_tom", "npc_3399_tom")]);

    let error = match apply_character_registry_alias_plan(
        &CharacterRegistryAliasPlanOptions::new(root, &plan, true),
    ) {
        Ok(_) => panic!("an alias owned by another model must be refused"),
        Err(error) => error.to_string(),
    };
    assert!(
        error.contains("already resolves to a different model"),
        "{error}"
    );
}

#[test]
fn reports_without_writing_until_apply_is_requested() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write_registry(
        root,
        &registry_json(&[(
            "npc/npc_tom",
            "npc_tom",
            "characters/npcs/npc_tom/npc_tom.glb",
            &[],
        )]),
    );
    write_table_set(root, &["npc_tom", "npc_3399_tom"]);
    let plan = write_plan(root, &[("npc_tom", "npc_3399_tom")]);

    let report = apply_character_registry_alias_plan(&CharacterRegistryAliasPlanOptions::new(
        root, &plan, false,
    ))
    .unwrap();

    assert!(!report.applied);
    assert_eq!(report.changes.len(), 1);
    assert!(plan_registry(root).models[0].legacy_aliases.is_empty());
}
use crate::RuntimeCharacterCategory;

fn registry_json(models: &[(&str, &str, &str, &[&str])]) -> String {
    let models = models
        .iter()
        .map(|(id, logical, glb, aliases)| crate::RuntimeCharacterModel {
            id: (*id).to_owned(),
            logical_name: (*logical).to_owned(),
            legacy_aliases: aliases.iter().map(|value| (*value).to_owned()).collect(),
            category: RuntimeCharacterCategory::Npc,
            glb: (*glb).to_owned(),
            glb_blake3: "0".repeat(64),
            collision: None,
            animations: vec!["stand1".to_owned()],
        })
        .collect();
    serde_json::to_string_pretty(&SemanticCharacterRegistry {
        schema: SEMANTIC_CHARACTER_REGISTRY_SCHEMA.to_owned(),
        models,
    })
    .unwrap()
}

fn write_registry(root: &Path, json: &str) {
    let path = root.join("_runtime");
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("characters.json"), json).unwrap();
}

fn write_table_set(root: &Path, npc_meshes: &[&str]) {
    let path = root.join("data/tables");
    fs::create_dir_all(&path).unwrap();
    let rows: Vec<_> = npc_meshes
        .iter()
        .map(|name| serde_json::json!({ "m_pstrMMeshModelString": name }))
        .collect();
    let document = serde_json::json!({
        "schema": "ffone.table-set.v1",
        "tables": [{
            "key": "k",
            "name": "npc_imports_consolidated",
            "value": { "m_pNpcTable": { "m_pNpcMeshData": rows } }
        }]
    });
    fs::write(
        path.join("table-set.json"),
        serde_json::to_vec_pretty(&document).unwrap(),
    )
    .unwrap();
}

#[test]
fn adds_the_installer_alias_only_where_the_package_route_differs() {
    let temp = tempfile::tempdir().unwrap();
    write_registry(
        temp.path(),
        &registry_json(&[
            // canonical: route == logical name
            (
                "npc/npc_dexter",
                "npc_dexter",
                "characters/npcs/npc_dexter/npc_dexter.glb",
                &[],
            ),
            // already aliased
            (
                "fusion/fusion_echo",
                "fusion_echoecho",
                "characters/fusions/fusion_echo/fusion_echoecho.glb",
                &["fusion_echo"],
            ),
            // missing alias: XDT `npc_key` cannot reach `npc_icekey`
            (
                "npc/npc_key",
                "npc_icekey",
                "characters/npcs/npc_key/npc_icekey.glb",
                &[],
            ),
        ]),
    );

    let plan = repair_character_registry_aliases(&CharacterRegistryAliasRepairOptions::new(
        temp.path(),
        false,
    ))
    .unwrap();
    assert_eq!(plan.models, 3);
    assert_eq!(plan.canonical, 1);
    assert_eq!(plan.already_aliased, 1);
    assert_eq!(plan.changes.len(), 1);
    assert_eq!(plan.changes[0].id, "npc/npc_key");
    assert_eq!(plan.changes[0].added_aliases, vec!["npc_key".to_owned()]);
    assert!(!plan.changes[0].from_table_set);
    assert_eq!(plan.table_set_mesh_names, 0);
    assert!(!plan.applied);
    // A plan run must not touch the registry.
    let unchanged: SemanticCharacterRegistry = serde_json::from_slice(
        &fs::read(temp.path().join("_runtime/characters.json")).unwrap(),
    )
    .unwrap();
    assert!(unchanged.models[2].legacy_aliases.is_empty());

    let applied = repair_character_registry_aliases(&CharacterRegistryAliasRepairOptions::new(
        temp.path(),
        true,
    ))
    .unwrap();
    assert!(applied.applied);
    assert_eq!(applied.changes.len(), 1);
    let after: SemanticCharacterRegistry = serde_json::from_slice(
        &fs::read(temp.path().join("_runtime/characters.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(after.models[2].legacy_aliases, vec!["npc_key".to_owned()]);
    // Nothing else moved.
    assert!(after.models[0].legacy_aliases.is_empty());
    assert_eq!(
        after.models[1].legacy_aliases,
        vec!["fusion_echo".to_owned()]
    );

    // Re-running is a no-op.
    let again = repair_character_registry_aliases(&CharacterRegistryAliasRepairOptions::new(
        temp.path(),
        true,
    ))
    .unwrap();
    assert!(again.changes.is_empty());
    assert!(!again.applied);
    assert_eq!(again.already_aliased, 2);
}

#[test]
fn adopts_the_table_set_spelling_because_the_runtime_probe_is_case_sensitive() {
    let temp = tempfile::tempdir().unwrap();
    write_registry(
        temp.path(),
        &registry_json(&[(
            "fusion/fusion_belladonnaphase1",
            "fusion_butterdonnaPhase1",
            "characters/fusions/fusion_belladonnaphase1/fusion_butterdonnaPhase1.glb",
            &[],
        )]),
    );
    // The package directory is lowercase; the table is not.
    write_table_set(temp.path(), &["fusion_belladonnaPhase1", "npc_dexter"]);

    let report = repair_character_registry_aliases(&CharacterRegistryAliasRepairOptions::new(
        temp.path(),
        true,
    ))
    .unwrap();
    assert_eq!(report.table_set_mesh_names, 2);
    assert_eq!(report.changes.len(), 1);
    assert!(report.changes[0].from_table_set);
    assert_eq!(
        report.changes[0].added_aliases,
        vec!["fusion_belladonnaPhase1".to_owned()],
        "the alias must carry the table's exact spelling"
    );

    let after: SemanticCharacterRegistry = serde_json::from_slice(
        &fs::read(temp.path().join("_runtime/characters.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        after.models[0].legacy_aliases,
        vec!["fusion_belladonnaPhase1".to_owned()]
    );

    let again = repair_character_registry_aliases(&CharacterRegistryAliasRepairOptions::new(
        temp.path(),
        true,
    ))
    .unwrap();
    assert!(again.changes.is_empty());
    assert_eq!(again.already_aliased, 1);
}

#[test]
fn rejects_a_registry_glb_without_a_package_directory() {
    let temp = tempfile::tempdir().unwrap();
    write_registry(
        temp.path(),
        &registry_json(&[("npc/loose", "loose_root", "loose_root.glb", &[])]),
    );
    let error = repair_character_registry_aliases(&CharacterRegistryAliasRepairOptions::new(
        temp.path(),
        false,
    ))
    .unwrap_err();
    assert!(error.to_string().contains("package directory"), "{error}");
}
