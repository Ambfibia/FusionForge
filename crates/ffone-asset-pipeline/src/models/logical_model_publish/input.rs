use super::*;

pub(super) fn parse_interpolation(value: &str) -> Result<Interpolation> {
    match value {
        "LINEAR" => Ok(Interpolation::Linear),
        "STEP" => Ok(Interpolation::Step),
        "CUBICSPLINE" => Ok(Interpolation::CubicSpline),
        other => invalid(format!("unsupported animation interpolation {other:?}")),
    }
}

pub(super) fn resolve_global_transform(
    model: &NativeModel,
    index: usize,
    globals: &mut [Option<Mat4>],
    visiting: &mut BTreeSet<usize>,
) -> Result<Mat4> {
    if let Some(matrix) = globals[index] {
        return Ok(matrix);
    }
    if !visiting.insert(index) {
        return invalid("coordinate audit found a hierarchy cycle");
    }
    let node = &model.nodes[index];
    let local = Mat4::from_scale_rotation_translation(
        Vec3::new(
            node.scale[0] as f32,
            node.scale[1] as f32,
            node.scale[2] as f32,
        ),
        Quat::from_xyzw(
            node.rotation[0] as f32,
            node.rotation[1] as f32,
            node.rotation[2] as f32,
            node.rotation[3] as f32,
        ),
        Vec3::new(
            node.translation[0] as f32,
            node.translation[1] as f32,
            node.translation[2] as f32,
        ),
    );
    let global = match node.parent {
        Some(parent) => {
            resolve_global_transform(model, parent as usize, globals, visiting)? * local
        }
        None => local,
    };
    visiting.remove(&index);
    if !global.is_finite() {
        return invalid("coordinate audit produced a non-finite global transform");
    }
    globals[index] = Some(global);
    Ok(global)
}
