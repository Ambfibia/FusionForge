use super::*;

/// Install all prepared runtime-animation outputs as one rollback-capable set.
///
/// The gate exists solely so the unit test can inject a deterministic failure
/// after an installed file. Production publication always uses an accepting
/// gate.
pub(super) fn commit_player_rig_animation_outputs_with_gate(
    outputs: &[(PathBuf, Vec<u8>)],
    mut after_install: impl FnMut(usize, &Path) -> Result<()>,
) -> Result<()> {
    let mut destinations = BTreeSet::new();
    let mut transaction = Vec::with_capacity(outputs.len());
    for (destination, bytes) in outputs {
        if !destinations.insert(destination.clone()) {
            return rig_error(format!(
                "runtime animation transaction repeats output {}",
                destination.display()
            ));
        }
        let stage = player_rig_animation_sidecar(destination, PLAYER_RIG_ANIMATION_STAGE_SUFFIX)?;
        let backup = player_rig_animation_sidecar(destination, PLAYER_RIG_ANIMATION_BACKUP_SUFFIX)?;
        if stage.exists() || backup.exists() {
            return rig_error(format!(
                "runtime animation transaction found stale sidecar for {}",
                destination.display()
            ));
        }
        transaction.push((
            destination.clone(),
            stage,
            backup,
            destination.exists(),
            bytes,
        ));
    }

    let prepare_result = (|| {
        for (destination, stage, _, _, bytes) in &transaction {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
            }
            fs::write(stage, bytes).map_err(|error| io_at(stage, error))?;
        }
        Ok(())
    })();
    if let Err(prepare_error) = prepare_result {
        let mut cleanup_errors = Vec::new();
        for (_, prepared_stage, _, _, _) in &transaction {
            if prepared_stage.exists()
                && let Err(error) = fs::remove_file(prepared_stage)
            {
                cleanup_errors.push(format!(
                    "remove stage {}: {error}",
                    prepared_stage.display()
                ));
            }
        }
        if cleanup_errors.is_empty() {
            return Err(prepare_error);
        }
        return rig_error(format!(
            "runtime animation transaction preparation failed ({prepare_error}); cleanup also failed: {}",
            cleanup_errors.join("; ")
        ));
    }

    let install_result = (|| {
        for (index, (destination, stage, backup, had_original, _)) in transaction.iter().enumerate()
        {
            if *had_original {
                fs::rename(destination, backup).map_err(|error| io_at(destination, error))?;
            }
            fs::rename(stage, destination).map_err(|error| io_at(stage, error))?;
            after_install(index + 1, destination)?;
        }
        Ok(())
    })();

    if let Err(install_error) = install_result {
        let mut rollback_errors = Vec::new();
        for (destination, stage, backup, had_original, _) in transaction.iter().rev() {
            if backup.exists() {
                if destination.exists()
                    && let Err(error) = fs::remove_file(destination)
                {
                    rollback_errors.push(format!("remove new {}: {error}", destination.display()));
                    continue;
                }
                if let Err(error) = fs::rename(backup, destination) {
                    rollback_errors.push(format!("restore {}: {error}", destination.display()));
                }
            } else if !*had_original
                && destination.exists()
                && let Err(error) = fs::remove_file(destination)
            {
                rollback_errors.push(format!(
                    "remove newly-created {}: {error}",
                    destination.display()
                ));
            }
            if stage.exists()
                && let Err(error) = fs::remove_file(stage)
            {
                rollback_errors.push(format!("remove stage {}: {error}", stage.display()));
            }
        }
        if rollback_errors.is_empty() {
            return Err(install_error);
        }
        return rig_error(format!(
            "runtime animation transaction failed ({install_error}); rollback also failed: {}",
            rollback_errors.join("; ")
        ));
    }

    for (_, stage, backup, _, _) in &transaction {
        if stage.exists() {
            fs::remove_file(stage).map_err(|error| io_at(stage, error))?;
        }
        if backup.exists() {
            fs::remove_file(backup).map_err(|error| io_at(backup, error))?;
        }
    }
    Ok(())
}

pub(super) fn validate_animation_component(
    catalog: &DumpCatalog,
    component_path_id: i64,
    expected_count: usize,
    required: &[(&str, i64); 5],
) -> Result<()> {
    let body = catalog.value(component_path_id)?;
    let pointers = array_field(&body, "m_Animations")?;
    if pointers.len() != expected_count {
        return rig_error(format!(
            "Animation#{component_path_id} expected {expected_count} clips, found {}",
            pointers.len()
        ));
    }
    let ids = pointers
        .iter()
        .map(pointer_path_id)
        .collect::<Result<BTreeSet<_>>>()?;
    for (name, path_id) in required {
        if !ids.contains(path_id) {
            return rig_error(format!(
                "Animation#{component_path_id} does not reference required {name}#{path_id}"
            ));
        }
        let object = catalog.get(*path_id)?;
        if object.object_type != "AnimationClip" || object.name != *name {
            return rig_error(format!(
                "required {name}#{path_id} identity drifted to {} {:?}",
                object.object_type, object.name
            ));
        }
    }
    Ok(())
}

/// Unity's legacy `AnimationBlendMode.Additive` evaluates each curve relative
/// to that curve's first sample. glTF has no additive-clip marker, so its
/// animation channels necessarily contain the absolute local TR values. Bevy's
/// additive graph instead expects zero-relative translations/scales and
/// identity-relative rotations. Convert only the source-proven additive
/// body-shape and directional-turn clips while keeping times and derivatives
/// exact.
pub(super) fn rebase_legacy_additive_clip(clip: &mut AnimationClip, nodes: &[PlayerRigNode]) -> Result<()> {
    if !matches!(
        clip.name.as_str(),
        "height_Add" | "shape_Add" | "turnleft" | "turnright" | "rifleturnleft" | "rifleturnright" | "scooter_turnleft" | "scooter_turnright"
    ) {
        return Ok(());
    }

    for channel in &mut clip.channels {
        let node = nodes.get(channel.target_node as usize).ok_or_else(|| {
            rig_message(format!(
                "{} additive channel targets missing node {}",
                clip.name, channel.target_node
            ))
        })?;
        match &mut channel.values {
            TrackValues::Translation(values) => {
                let reference = values.first().copied().ok_or_else(|| {
                    rig_message(format!(
                        "{} additive translation channel for {:?} has no samples",
                        clip.name, node.true_name
                    ))
                })?;
                for value in values {
                    for (component, reference) in value.iter_mut().zip(reference) {
                        *component -= reference;
                    }
                }
                // Translation derivatives are unchanged by subtracting a
                // constant reference pose.
            }
            TrackValues::Rotation(values) => {
                let reference = values.first().copied().ok_or_else(|| {
                    rig_message(format!(
                        "{} additive rotation channel for {:?} has no samples",
                        clip.name, node.true_name
                    ))
                })?;
                let inverse_reference = quat_conjugate(reference);
                for value in values {
                    *value = quat_multiply(*value, inverse_reference);
                }
                if let Some(TrackValues::Rotation(tangents)) = &mut channel.in_tangents {
                    for tangent in tangents {
                        *tangent = quat_multiply(*tangent, inverse_reference);
                    }
                }
                if let Some(TrackValues::Rotation(tangents)) = &mut channel.out_tangents {
                    for tangent in tangents {
                        *tangent = quat_multiply(*tangent, inverse_reference);
                    }
                }
            }
            TrackValues::Scale(values) => {
                let reference = values.first().copied().ok_or_else(|| {
                    rig_message(format!(
                        "{} additive scale channel for {:?} has no samples",
                        clip.name, node.true_name
                    ))
                })?;
                for value in values {
                    for (component, reference) in value.iter_mut().zip(reference) {
                        *component -= reference;
                    }
                }
                // Scale derivatives are also unchanged by subtracting the
                // constant imported reference scale. Bevy's additive
                // evaluator adds scale vectors, matching this delta form.
            }
        }
    }
    Ok(())
}

pub(super) fn rig_error<T>(message: impl Into<String>) -> Result<T> {
    Err(rig_message(message))
}

pub(super) fn rig_message(message: impl Into<String>) -> PipelineError {
    PipelineError::PlayerAvatarCook(format!("shared rig: {}", message.into()))
}
