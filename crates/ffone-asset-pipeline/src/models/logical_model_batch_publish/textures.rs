use super::*;

#[derive(Debug, Deserialize)]
pub(super) struct PreflightTexture {
    pub(super) name: String,
    #[serde(rename = "mipLevels")]
    pub(super) mip_levels: ArrayLength,
}

pub(super) fn invalid_saved_texture_slot_evidence(
    materials: &BTreeMap<String, Value>,
    source_relative: &str,
) -> Result<Option<Value>> {
    let mut invalid_properties = Vec::new();
    for (material_key, material) in materials {
        let texture_environments = material
            .pointer("/savedProperties/textureEnvs")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                batch_error_value(format!(
                    "material {material_key:?} in {source_relative:?} has no exact savedProperties.textureEnvs array"
                ))
            })?;
        let raw_texture_environments = material
            .pointer("/rawSavedProperties/m_TexEnvs")
            .cloned()
            .unwrap_or(Value::Null);
        for (array_index, environment) in texture_environments.iter().enumerate() {
            let name = environment.get("name").and_then(Value::as_str).ok_or_else(|| {
                batch_error_value(format!(
                    "material {material_key:?} texture environment {array_index} in {source_relative:?} has no serialized name"
                ))
            })?;
            if !invalid_native_name(name) {
                continue;
            }
            let proven_unassigned_stale_null =
                environment.get("unassignedSlot").and_then(Value::as_bool) == Some(true)
                    && environment.get("slot").and_then(Value::as_u64)
                        == u64::try_from(array_index).ok()
                    && environment.get("textureId").is_some_and(Value::is_null)
                    && environment
                        .pointer("/texturePointer/isNull")
                        .and_then(Value::as_bool)
                        == Some(true)
                    && environment
                        .pointer("/texturePointer/pathId")
                        .and_then(Value::as_i64)
                        == Some(0);
            if proven_unassigned_stale_null {
                continue;
            }

            let script = material
                .pointer("/shader/script/text")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    batch_error_value(format!(
                        "material {material_key:?} in {source_relative:?} has no UTF-8 ShaderLab script"
                    ))
                })?;
            let declared_name = material
                .pointer("/shader/declaredName")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    batch_error_value(format!(
                        "material {material_key:?} in {source_relative:?} has no declared shader name"
                    ))
                })?;
            let script_sha256 = material
                .pointer("/shader/scriptSha256")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    batch_error_value(format!(
                        "material {material_key:?} in {source_relative:?} has no shader SHA-256"
                    ))
                })?;
            let actual_sha256 = sha256_hex(script.as_bytes());
            if actual_sha256 != script_sha256 {
                return batch_error(format!(
                    "material {material_key:?} in {source_relative:?} ShaderLab SHA-256 evidence is contradictory"
                ));
            }
            let declared_texture_slots = exact_shader_texture_defaults(script)
                .map_err(batch_error_value)
                .and_then(|defaults| {
                    serde_json::to_value(defaults).map_err(|error| {
                        batch_error_value(format!(
                            "could not serialize exact ShaderLab texture defaults: {error}"
                        ))
                    })
                })?;
            invalid_properties.push(json!({
                "materialKey": material_key,
                "materialId": material.get("id"),
                "materialName": material.get("name"),
                "materialSource": material.get("source"),
                "materialSourcePathId": material.pointer("/source/pathId"),
                "serializedShaderName": material.get("shaderName"),
                "declaredShaderName": declared_name,
                "shaderScriptByteLength": material.pointer("/shader/scriptByteLength"),
                "shaderScriptSha256": script_sha256,
                "declaredTextureSlots": declared_texture_slots,
                "invalidSavedTextureArrayIndex": array_index,
                "invalidSavedTextureSlot": environment.get("slot"),
                "invalidSerializedName": name,
                "exactSavedTextureArray": texture_environments,
                "exactRawSavedTextureArray": raw_texture_environments,
            }));
        }
    }

    Ok((!invalid_properties.is_empty()).then(|| {
        json!({
            "schema": "ffone.invalid-serialized-texture-slot.v1",
            "proof": "exact-saved-texture-array-contains-invalid-name-and-declared-slot-mapping-is-not-authoritative",
            "invalidProperties": invalid_properties,
            "fallbackApplied": false,
        })
    }))
}
