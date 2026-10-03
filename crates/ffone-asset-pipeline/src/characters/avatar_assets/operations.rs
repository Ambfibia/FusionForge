use super::*;

pub(super) fn canonical_directory(path: &Path) -> Result<PathBuf, AvatarCatalogError> {
    let canonical = fs::canonicalize(path).map_err(|source| AvatarCatalogError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !canonical.is_dir() {
        return Err(AvatarCatalogError::Invalid(format!(
            "{} is not a directory",
            canonical.display()
        )));
    }
    Ok(canonical)
}

pub(super) fn canonical_file(path: &Path) -> Result<PathBuf, AvatarCatalogError> {
    let canonical = fs::canonicalize(path).map_err(|source| AvatarCatalogError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !canonical.is_file() {
        return Err(AvatarCatalogError::Invalid(format!(
            "{} is not a file",
            canonical.display()
        )));
    }
    Ok(canonical)
}

pub(super) fn unique_input<'a>(
    inputs: &'a [PlanInput],
    role: &str,
) -> Result<&'a PlanInput, AvatarCatalogError> {
    let matches = inputs
        .iter()
        .filter(|input| input.role == role)
        .collect::<Vec<_>>();
    let [input] = matches.as_slice() else {
        return Err(AvatarCatalogError::Invalid(format!(
            "semantic plan requires exactly one {role:?} input, found {}",
            matches.len()
        )));
    };
    Ok(input)
}

pub(super) fn verify_evidence(input: &PlanInput, path: &Path, issues: &mut Vec<String>) {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            issues.push(format!(
                "{} evidence {} is unreadable: {error}",
                input.role,
                path.display()
            ));
            return;
        }
    };
    if metadata.len() != input.bytes {
        issues.push(format!(
            "{} evidence length mismatch for {}: plan={}, disk={}",
            input.role,
            path.display(),
            input.bytes,
            metadata.len()
        ));
        return;
    }
    match blake3_file(path) {
        Ok(actual) if actual == input.blake3 => {}
        Ok(actual) => issues.push(format!(
            "{} evidence BLAKE3 mismatch for {}: plan={}, disk={actual}",
            input.role,
            path.display(),
            input.blake3
        )),
        Err(error) => issues.push(format!(
            "{} evidence hash failed for {}: {error}",
            input.role,
            path.display()
        )),
    }
}

pub(super) fn blake3_file(path: &Path) -> Result<String, std::io::Error> {
    let mut file = fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub(super) fn int_field(value: &Value, field: &str) -> Option<i64> {
    value.get(field).and_then(Value::as_i64)
}

pub(super) fn string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| {
            !value.is_empty()
                && !value.eq_ignore_ascii_case("null")
                && !value.eq_ignore_ascii_case("none")
        })
}

pub(super) fn gender_string<'a>(mesh: &'a Value, gender: AvatarGender, suffix: &str) -> Option<&'a str> {
    string_field(mesh, &format!("m_pstr{}{}", gender.field_prefix(), suffix))
}

pub(super) fn source_stem(route: &str) -> Option<&str> {
    route
        .rsplit('/')
        .next()
        .and_then(|name| {
            name.rsplit_once('.')
                .map_or(Some(name), |(stem, _)| Some(stem))
        })
        .filter(|stem| !stem.is_empty())
}

pub(super) const fn semantic_category(part: AvatarPartKind) -> &'static str {
    match part {
        AvatarPartKind::Body => "body",
        AvatarPartKind::Face => "face",
        AvatarPartKind::Hair => "hair",
        AvatarPartKind::UpperBody => "equipment/shirt",
        AvatarPartKind::LowerBody => "equipment/pants",
        AvatarPartKind::Foot => "equipment/shoes",
        AvatarPartKind::Hat => "equipment/hat",
        AvatarPartKind::Glasses => "equipment/glasses",
        AvatarPartKind::Back => "equipment/back",
        AvatarPartKind::Hand | AvatarPartKind::ExtendedHand => "equipment/weapon",
        AvatarPartKind::Vehicle => "equipment/vehicle",
    }
}

pub(super) fn disposition_for_assets(
    model: Option<&NativeAvatarAssetResolution>,
    textures: &[NativeAvatarAssetResolution],
) -> AvatarPartDisposition {
    let model_ready = model.is_none_or(|asset| asset.status == NativeResolutionStatus::Verified);
    let textures_ready = textures
        .iter()
        .all(|asset| asset.status == NativeResolutionStatus::Verified);
    if model_ready && textures_ready {
        AvatarPartDisposition::Ready
    } else {
        AvatarPartDisposition::Missing
    }
}

pub(super) fn suppress_part(part: &mut AvatarPartResolution, detail: impl Into<String>) {
    part.disposition = AvatarPartDisposition::SuppressedByHat;
    part.model = None;
    part.textures.clear();
    part.blockers.push(detail.into());
}

pub(super) fn block_part(part: &mut AvatarPartResolution, detail: impl Into<String>) {
    part.disposition = AvatarPartDisposition::Missing;
    part.blockers.push(detail.into());
}

pub(super) fn empty_style_part(part: AvatarPartKind, name: &str) -> AvatarPartResolution {
    AvatarPartResolution {
        part,
        semantic_name: name.to_owned(),
        participation: AvatarPartParticipation::CombinedSkinnedMesh,
        disposition: AvatarPartDisposition::Empty,
        table: None,
        model: None,
        textures: Vec::new(),
        blockers: Vec::new(),
    }
}

pub(super) fn unresolved_style_part(part: AvatarPartKind, name: &str, detail: String) -> AvatarPartResolution {
    AvatarPartResolution {
        part,
        semantic_name: name.to_owned(),
        participation: match part {
            AvatarPartKind::Body => AvatarPartParticipation::SharedSkeletonBase,
            AvatarPartKind::Face
            | AvatarPartKind::Hair
            | AvatarPartKind::UpperBody
            | AvatarPartKind::LowerBody
            | AvatarPartKind::Foot => AvatarPartParticipation::CombinedSkinnedMesh,
            AvatarPartKind::Back => AvatarPartParticipation::BackTablePolicy,
            AvatarPartKind::ExtendedHand => AvatarPartParticipation::CombatSecondary,
            AvatarPartKind::Vehicle => AvatarPartParticipation::SeparateVehicle,
            AvatarPartKind::Hat | AvatarPartKind::Glasses | AvatarPartKind::Hand => {
                AvatarPartParticipation::SocketAttachment
            }
        },
        disposition: AvatarPartDisposition::Missing,
        table: None,
        model: None,
        textures: Vec::new(),
        blockers: vec![detail],
    }
}

pub(super) fn missing_sources(parts: &[AvatarPartResolution]) -> Vec<String> {
    let mut missing = BTreeSet::new();
    for part in parts {
        if part.disposition != AvatarPartDisposition::Missing {
            continue;
        }
        for blocker in &part.blockers {
            missing.insert(format!("{:?}: {blocker}", part.part));
        }
        if let Some(model) = &part.model
            && model.status != NativeResolutionStatus::Verified
        {
            missing.insert(format!("model:{}", model.source_route));
        }
        for texture in &part.textures {
            if texture.status != NativeResolutionStatus::Verified {
                missing.insert(format!("texture:{}", texture.source_route));
            }
        }
    }
    missing.into_iter().collect()
}
