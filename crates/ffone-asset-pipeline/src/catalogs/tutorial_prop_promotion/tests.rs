use super::*;
use tempfile::tempdir;

#[test]
fn stale_recovery_stage_blocks_a_new_transaction() {
    let temp = tempdir().expect("temporary project root");
    reject_stale_staging(temp.path()).expect("clean project root is accepted");

    let stale = temp.path().join(".tutorial-prop-promotion-123-456");
    fs::create_dir(&stale).expect("stale transaction fixture");
    let error = reject_stale_staging(temp.path()).expect_err("stale stage must block");

    assert!(
        error
            .to_string()
            .contains("must be inspected before retrying")
    );
    assert!(error.to_string().contains(&stale.display().to_string()));
}

#[test]
fn runtime_reference_replacement_is_exact_and_idempotent() {
    let temp = tempdir().expect("temporary project root");
    let candidate = CANDIDATES[0];
    let runtime_path = temp.path().join(native_path(candidate.runtime_source));
    fs::create_dir_all(runtime_path.parent().expect("runtime parent"))
        .expect("runtime directory");
    let legacy = candidate.source_glb();
    let promoted = candidate.target_glb();
    fs::write(
        &runtime_path,
        format!("const MODEL: &str = \"{legacy}\";\n"),
    )
    .expect("legacy runtime fixture");

    let (_, before, after, proof) =
        prepare_runtime_reference(temp.path(), &candidate, false).expect("exact replacement");
    let after_text = std::str::from_utf8(&after).expect("UTF-8 replacement");
    assert_eq!(
        std::str::from_utf8(&before)
            .unwrap()
            .matches(&legacy)
            .count(),
        1
    );
    assert_eq!(after_text.matches(&legacy).count(), 0);
    assert_eq!(after_text.matches(&promoted).count(), 1);
    assert_ne!(proof.before_sha256, proof.after_sha256);

    fs::write(&runtime_path, &after).expect("install promoted runtime fixture");
    let (_, promoted_before, promoted_after, second_proof) =
        prepare_runtime_reference(temp.path(), &candidate, true).expect("idempotent proof");
    assert_eq!(promoted_before, promoted_after);
    assert_eq!(second_proof.before_sha256, second_proof.after_sha256);

    fs::write(
        &runtime_path,
        format!("const OLD: &str = \"{legacy}\"; const NEW: &str = \"{promoted}\";\n"),
    )
    .expect("mixed runtime fixture");
    assert!(prepare_runtime_reference(temp.path(), &candidate, false).is_err());
    assert!(prepare_runtime_reference(temp.path(), &candidate, true).is_err());
}
