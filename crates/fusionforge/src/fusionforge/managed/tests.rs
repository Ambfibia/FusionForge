use super::*;

#[test]
fn layout_bundle_additions_preserve_dependency_order_and_dedupe_case_insensitively() {
    let existing = BTreeSet::from(["coreshared.resourcefile".to_string()]);
    let requested = requested_asset_loader_downloads(
        &[
            "CoreShared.resourceFile".to_string(),
            "NpcVoiceShared_001.resourceFile".to_string(),
            "NPC_Pack_001.resourceFile".to_string(),
            "npc_pack_001.resourcefile".to_string(),
            "Icons_Pack_001.resourceFile".to_string(),
        ],
        &existing,
    );

    assert_eq!(
        requested,
        vec![
            "NpcVoiceShared_001.resourceFile",
            "NPC_Pack_001.resourceFile",
            "Icons_Pack_001.resourceFile",
        ]
    );
}

#[test]
fn layout_bundle_descriptions_keep_hnpc_distinct_from_regular_npc() {
    assert!(
        asset_loader_bundle_description("HNPC_Pack_001.resourceFile")
            .starts_with("HNPC Character Data")
    );
    assert!(asset_loader_bundle_description("NPC_Pack_001.resourceFile")
        .starts_with("NPC Character Data"));
}

#[test]
fn repairs_utf8_text_decoded_as_windows_1251() {
    let source = "\u{0413}\u{041e}\u{041b}\u{041e}\u{0412}\u{0410}";
    let mojibake = "\u{0420}\u{201c}\u{0420}\u{045b}\u{0420}\u{203a}\u{0420}\u{045b}\u{0420}\u{2019}\u{0420}\u{0452}";

    assert_eq!(repair_cp1251_mojibake(mojibake), Some(source.to_string()));
    assert_eq!(repair_cp1251_mojibake(source), None);
}

#[test]
fn scans_call_tokens_without_confusing_other_inline_tokens() {
    let code = [
        0x72, 0x01, 0x00, 0x00, 0x70, // ldstr
        0x28, 0x17, 0x0b, 0x00, 0x06, // call
        0x6f, 0x01, 0x00, 0x00, 0x0a, // callvirt
        0x00, // nop
    ];

    assert_eq!(scan_call_tokens(&code).unwrap(), vec![(5, 0x0600_0b17)]);
}

#[test]
fn live_tutorial_voice_calls_accept_manifest_clip_durations_when_available() {
    let repo_root = crate::repository_root()
        .parent()
        .expect("workspace root");
    let project = repo_root
        .join("builds")
        .join("6543a2bb-d154-4087-b9ee-3c8aa778580a.ffclient");
    let audio_dir = project.join("audio");
    let manifest_path = audio_dir.join("manifest.json");
    let cache_root = project.join("cache").join("extracted-bundles");
    if !manifest_path.is_file() || !cache_root.is_dir() {
        eprintln!("skipping live tutorial voice patch test: client cache is unavailable");
        return;
    }
    let manifest = read_json(&manifest_path).expect("read audio manifest");
    let entries = manifest
        .get("entries")
        .and_then(JsonValue::as_array)
        .expect("audio manifest entries");
    let durations = crate::tutorial_voice_durations(entries, &audio_dir)
        .expect("read tutorial voice durations");
    let assembly_path = fs::read_dir(&cache_root)
        .expect("read extracted bundle cache")
        .flatten()
        .map(|entry| entry.path().join("Assembly - CSharp.dll"))
        .find(|path| path.is_file())
        .expect("cached Assembly - CSharp.dll");
    let mut context = AssemblyContext::from_path(&assembly_path).expect("parse assembly");
    let patched = patch_tutorial_voice_durations_in_context(&mut context, &durations)
        .expect("patch tutorial durations");

    assert!(durations.len() >= 80, "expected the tutorial audio catalog");
    assert!(
        patched >= 80,
        "expected most voiced tutorial calls to receive clip durations, got {patched}"
    );
}

#[test]
fn patches_binary_reader_unicode_default_decode_pattern() {
    let code = binary_reader_default_decode_code(true);
    let patched = patch_binary_reader_unicode_code(&code, &binary_reader_unicode_tokens())
        .expect("patch should succeed");

    assert_eq!(patched.sites, 1);
    assert_eq!(
        patched.code,
        vec![
            0x03, // ldarg.1
            0x11, 0x05, // ldloc.s 5
            0x18, // ldc.i4.2
            0x5a, // mul
            0x6f, 0x3c, 0x02, 0x00, 0x0a, // callvirt ReadBytes
            0x13, 0x06, // stloc.s 6
            0x28, 0x3f, 0x02, 0x00, 0x0a, // call get_Unicode
            0x11, 0x06, // ldloc.s 6
            0x6f, 0x3e, 0x02, 0x00, 0x0a, // callvirt GetString
            0x13, 0x07, // stloc.s 7
            0x2a, // ret
        ]
    );
}

#[test]
fn patches_binary_reader_unicode_decode_and_updates_short_branch() {
    let mut code = vec![0x2b, 24]; // br.s to ret
    code.extend(binary_reader_default_decode_code(false));
    code.push(0x2a);

    let patched = patch_binary_reader_unicode_code(&code, &binary_reader_unicode_tokens())
        .expect("patch should succeed");

    assert_eq!(patched.sites, 1);
    assert_eq!(patched.code[1], 26);
}

fn binary_reader_unicode_tokens() -> BinaryReaderUnicodeTokens {
    BinaryReaderUnicodeTokens {
        read_bytes: BTreeSet::from([0x0a00_023c]),
        get_default: BTreeSet::from([0x0a00_023d]),
        get_unicode: BTreeSet::from([0x0a00_023f]),
        get_string: BTreeSet::from([0x0a00_023e]),
    }
}

fn binary_reader_default_decode_code(include_ret: bool) -> Vec<u8> {
    let mut code = vec![
        0x03, // ldarg.1
        0x11, 0x05, // ldloc.s 5
        0x6f, 0x3c, 0x02, 0x00, 0x0a, // callvirt ReadBytes
        0x13, 0x06, // stloc.s 6
        0x28, 0x3d, 0x02, 0x00, 0x0a, // call get_Default
        0x11, 0x06, // ldloc.s 6
        0x6f, 0x3e, 0x02, 0x00, 0x0a, // callvirt GetString
        0x13, 0x07, // stloc.s 7
    ];
    if include_ret {
        code.push(0x2a);
    }
    code
}
