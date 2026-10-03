use super::*;

pub(super) fn expected_program(family: ShaderFamily) -> ExpectedProgram {
    let alpha_blend = || RawBlend::Enabled {
        source: MaterialBlendFactor::SourceAlpha,
        destination: MaterialBlendFactor::OneMinusSourceAlpha,
    };

    match family {
        ShaderFamily::OpaqueNormal => ExpectedProgram {
            category: RawState::default(),
            passes: vec![RawState::default()],
        },
        ShaderFamily::AdditiveOneOne
        | ShaderFamily::AdditiveOneOneDepthWrite
        | ShaderFamily::AdditiveOneOneCullOff
        | ShaderFamily::AdditiveOneOneCullOffVertexColorAd
        | ShaderFamily::AdditiveOneOneCullOffDepthWrite => ExpectedProgram {
            category: RawState {
                queue: Some(
                    if matches!(
                        family,
                        ShaderFamily::AdditiveOneOneDepthWrite
                            | ShaderFamily::AdditiveOneOneCullOffDepthWrite
                    ) {
                        "transparent"
                    } else {
                        "transparent+11"
                    }
                    .to_owned(),
                ),
                blend: Some(RawBlend::Enabled {
                    source: MaterialBlendFactor::One,
                    destination: MaterialBlendFactor::One,
                }),
                cull: matches!(
                    family,
                    ShaderFamily::AdditiveOneOneCullOff
                        | ShaderFamily::AdditiveOneOneCullOffVertexColorAd
                        | ShaderFamily::AdditiveOneOneCullOffDepthWrite
                )
                .then_some(MaterialCullMode::Off),
                z_write: (!matches!(
                    family,
                    ShaderFamily::AdditiveOneOneDepthWrite
                        | ShaderFamily::AdditiveOneOneCullOffDepthWrite
                ))
                .then_some(false),
                color_mask: Some(0b0111),
                ..RawState::default()
            },
            passes: vec![RawState::default()],
        },
        ShaderFamily::AlphaBlendNormal | ShaderFamily::AlphaBlendVertexColor => ExpectedProgram {
            category: RawState {
                queue: Some("transparent".to_owned()),
                blend: Some(alpha_blend()),
                color_mask: Some(0b1111),
                ..RawState::default()
            },
            passes: vec![RawState::default()],
        },
        ShaderFamily::SkinDirectionalAlphaBlend => ExpectedProgram {
            category: RawState {
                queue: Some("transparent-100".to_owned()),
                blend: Some(alpha_blend()),
                color_mask: Some(0b1111),
                ..RawState::default()
            },
            passes: vec![RawState {
                name: Some("BASE".to_owned()),
                ..RawState::default()
            }],
        },
        ShaderFamily::AlphaBlendNormalCullOff => ExpectedProgram {
            category: RawState {
                queue: Some("transparent".to_owned()),
                blend: Some(alpha_blend()),
                cull: Some(MaterialCullMode::Off),
                color_mask: Some(0b1111),
                ..RawState::default()
            },
            passes: vec![RawState::default()],
        },
        ShaderFamily::GlowAlphaBlend => ExpectedProgram {
            category: RawState {
                queue: Some("transparent".to_owned()),
                blend: Some(alpha_blend()),
                color_mask: Some(0b0111),
                ..RawState::default()
            },
            passes: vec![RawState::default()],
        },
        ShaderFamily::AdditiveTransparent => ExpectedProgram {
            category: RawState {
                queue: Some("transparent+10".to_owned()),
                blend: Some(RawBlend::Enabled {
                    source: MaterialBlendFactor::SourceAlpha,
                    destination: MaterialBlendFactor::One,
                }),
                z_write: Some(false),
                color_mask: Some(0b0111),
                ..RawState::default()
            },
            passes: vec![RawState::default()],
        },
        ShaderFamily::AdditiveTransparentDepthWrite => ExpectedProgram {
            category: RawState {
                queue: Some("transparent".to_owned()),
                blend: Some(RawBlend::Enabled {
                    source: MaterialBlendFactor::SourceAlpha,
                    destination: MaterialBlendFactor::One,
                }),
                color_mask: Some(0b0111),
                ..RawState::default()
            },
            passes: vec![RawState::default()],
        },
        ShaderFamily::AdditiveTransparentCullOff => ExpectedProgram {
            category: RawState {
                queue: Some("transparent+10".to_owned()),
                blend: Some(RawBlend::Enabled {
                    source: MaterialBlendFactor::SourceAlpha,
                    destination: MaterialBlendFactor::One,
                }),
                cull: Some(MaterialCullMode::Off),
                z_write: Some(false),
                color_mask: Some(0b0111),
                ..RawState::default()
            },
            passes: vec![RawState::default()],
        },
        ShaderFamily::ParticleAdditiveTransparentCullOff => ExpectedProgram {
            category: RawState {
                queue: Some("transparent+3".to_owned()),
                blend: Some(RawBlend::Enabled {
                    source: MaterialBlendFactor::SourceAlpha,
                    destination: MaterialBlendFactor::One,
                }),
                cull: Some(MaterialCullMode::Off),
                z_write: Some(false),
                alpha_test: Some(RawAlphaTest::Enabled {
                    compare: MaterialCompareFunction::Greater,
                    reference: RawAlphaReference::Literal(0.01),
                }),
                color_mask: Some(0b0111),
                ..RawState::default()
            },
            passes: vec![RawState::default()],
        },
        ShaderFamily::ParticleOneMinusSourceAlpha
        | ShaderFamily::ParticleOneMinusDestinationColor
        | ShaderFamily::ParticleOneMinusDestinationColorBackface => ExpectedProgram {
            category: RawState {
                queue: Some("transparent+3".to_owned()),
                blend: Some(RawBlend::Enabled {
                    source: if family == ShaderFamily::ParticleOneMinusSourceAlpha {
                        MaterialBlendFactor::OneMinusSourceAlpha
                    } else {
                        MaterialBlendFactor::OneMinusDestinationColor
                    },
                    destination: MaterialBlendFactor::One,
                }),
                cull: Some(
                    if family == ShaderFamily::ParticleOneMinusDestinationColorBackface {
                        MaterialCullMode::Back
                    } else {
                        MaterialCullMode::Off
                    },
                ),
                z_write: Some(false),
                alpha_test: Some(RawAlphaTest::Enabled {
                    compare: MaterialCompareFunction::Greater,
                    reference: RawAlphaReference::Literal(0.01),
                }),
                color_mask: Some(0b0111),
                ..RawState::default()
            },
            passes: vec![RawState::default()],
        },
        ShaderFamily::VfxRotatingFlipbook | ShaderFamily::VfxScrollDistortAdditive => {
            ExpectedProgram {
                category: RawState {
                    queue: Some(
                        if family == ShaderFamily::VfxRotatingFlipbook {
                            "transparent+3"
                        } else {
                            "transparent"
                        }
                        .to_owned(),
                    ),
                    blend: Some(RawBlend::Enabled {
                        source: MaterialBlendFactor::SourceAlpha,
                        destination: if family == ShaderFamily::VfxRotatingFlipbook {
                            MaterialBlendFactor::OneMinusSourceAlpha
                        } else {
                            MaterialBlendFactor::One
                        },
                    }),
                    cull: Some(MaterialCullMode::Off),
                    z_write: Some(false),
                    color_mask: Some(0b1111),
                    ..RawState::default()
                },
                passes: vec![RawState {
                    name: Some("BASE".to_owned()),
                    ..RawState::default()
                }],
            }
        }
        ShaderFamily::VfxHologramSolidAdditive => ExpectedProgram {
            category: RawState {
                queue: Some("transparent-100".to_owned()),
                cull: Some(MaterialCullMode::Off),
                color_mask: Some(0b1111),
                ..RawState::default()
            },
            passes: vec![
                RawState {
                    name: Some("BASE".to_owned()),
                    blend: Some(alpha_blend()),
                    ..RawState::default()
                },
                RawState {
                    name: Some("OVERLAYS".to_owned()),
                    blend: Some(RawBlend::Enabled {
                        source: MaterialBlendFactor::SourceAlpha,
                        destination: MaterialBlendFactor::One,
                    }),
                    ..RawState::default()
                },
            ],
        },
        ShaderFamily::TransparentNormal | ShaderFamily::TransparentNormalCullOff => {
            ExpectedProgram {
                category: RawState {
                    queue: Some("transparent+10".to_owned()),
                    blend: Some(alpha_blend()),
                    cull: (family == ShaderFamily::TransparentNormalCullOff)
                        .then_some(MaterialCullMode::Off),
                    z_write: Some(false),
                    color_mask: Some(if family == ShaderFamily::TransparentNormalCullOff {
                        0b0111
                    } else {
                        0b1111
                    }),
                    ..RawState::default()
                },
                passes: vec![RawState::default()],
            }
        }
        ShaderFamily::Toon | ShaderFamily::ToonCullOffFallback => ExpectedProgram {
            category: RawState {
                queue: Some("transparent-100".to_owned()),
                blend: Some(alpha_blend()),
                color_mask: Some(0b1111),
                ..RawState::default()
            },
            passes: vec![
                RawState {
                    name: Some("BASE".to_owned()),
                    cull: Some(MaterialCullMode::Back),
                    ..RawState::default()
                },
                RawState {
                    name: Some("OUTLINE".to_owned()),
                    blend: Some(alpha_blend()),
                    cull: Some(MaterialCullMode::Front),
                    z_write: Some(true),
                    color_mask: Some(0b1111),
                    ..RawState::default()
                },
            ],
        },
        ShaderFamily::ToonImplicitBackface | ShaderFamily::ToonRimImplicitBackface => {
            ExpectedProgram {
                category: RawState {
                    queue: Some("transparent-100".to_owned()),
                    blend: Some(alpha_blend()),
                    color_mask: Some(0b1111),
                    ..RawState::default()
                },
                passes: vec![
                    RawState {
                        name: Some("BASE".to_owned()),
                        // ShaderLab's omitted Cull directive resolves to Back.
                        ..RawState::default()
                    },
                    RawState {
                        name: Some("OUTLINE".to_owned()),
                        blend: Some(alpha_blend()),
                        cull: Some(MaterialCullMode::Front),
                        z_write: Some(true),
                        color_mask: Some(0b1111),
                        ..RawState::default()
                    },
                ],
            }
        }
        ShaderFamily::ToonBaseCullOff => ExpectedProgram {
            category: RawState {
                queue: Some("transparent-100".to_owned()),
                blend: Some(alpha_blend()),
                color_mask: Some(0b1111),
                ..RawState::default()
            },
            passes: vec![
                RawState {
                    name: Some("BASE".to_owned()),
                    cull: Some(MaterialCullMode::Off),
                    ..RawState::default()
                },
                RawState {
                    name: Some("OUTLINE".to_owned()),
                    blend: Some(alpha_blend()),
                    cull: Some(MaterialCullMode::Front),
                    z_write: Some(true),
                    color_mask: Some(0b1111),
                    ..RawState::default()
                },
            ],
        },
        ShaderFamily::ToonCullOffCategory => ExpectedProgram {
            category: RawState {
                queue: Some("transparent-100".to_owned()),
                blend: Some(alpha_blend()),
                cull: Some(MaterialCullMode::Off),
                color_mask: Some(0b1111),
                ..RawState::default()
            },
            passes: vec![
                RawState {
                    name: Some("BASE".to_owned()),
                    ..RawState::default()
                },
                RawState {
                    name: Some("OUTLINE".to_owned()),
                    blend: Some(alpha_blend()),
                    cull: Some(MaterialCullMode::Front),
                    z_write: Some(true),
                    color_mask: Some(0b1111),
                    ..RawState::default()
                },
            ],
        },
        ShaderFamily::ToonExplicitDepth
        | ShaderFamily::ToonSkyRimExplicitDepth
        | ShaderFamily::SkinnedFusionMatterLightDir => ExpectedProgram {
            category: RawState {
                queue: Some("transparent-100".to_owned()),
                blend: Some(alpha_blend()),
                color_mask: Some(0b1111),
                ..RawState::default()
            },
            passes: vec![
                RawState {
                    name: Some("BASE".to_owned()),
                    cull: Some(MaterialCullMode::Back),
                    z_write: Some(true),
                    z_test: Some(MaterialCompareFunction::LessEqual),
                    ..RawState::default()
                },
                RawState {
                    name: Some("OUTLINE".to_owned()),
                    blend: Some(alpha_blend()),
                    cull: Some(MaterialCullMode::Front),
                    z_write: Some(true),
                    color_mask: Some(0b1111),
                    ..RawState::default()
                },
            ],
        },
        ShaderFamily::ToonRimTransparent => ExpectedProgram {
            category: RawState {
                queue: Some("transparent+3".to_owned()),
                blend: Some(alpha_blend()),
                color_mask: Some(0b1111),
                ..RawState::default()
            },
            passes: vec![
                RawState {
                    name: Some("BASE".to_owned()),
                    cull: Some(MaterialCullMode::Back),
                    z_write: Some(true),
                    z_test: Some(MaterialCompareFunction::LessEqual),
                    ..RawState::default()
                },
                RawState {
                    name: Some("OUTLINE".to_owned()),
                    blend: Some(alpha_blend()),
                    cull: Some(MaterialCullMode::Front),
                    z_write: Some(true),
                    color_mask: Some(0b1111),
                    ..RawState::default()
                },
            ],
        },
        ShaderFamily::ToonFlipped => ExpectedProgram {
            category: RawState {
                queue: Some("transparent-100".to_owned()),
                blend: Some(alpha_blend()),
                color_mask: Some(0b1111),
                ..RawState::default()
            },
            passes: vec![
                RawState {
                    name: Some("BASE".to_owned()),
                    ..RawState::default()
                },
                RawState {
                    name: Some("OUTLINE".to_owned()),
                    blend: Some(alpha_blend()),
                    cull: Some(MaterialCullMode::Back),
                    z_write: Some(true),
                    color_mask: Some(0b1111),
                    ..RawState::default()
                },
            ],
        },
        ShaderFamily::RimEmissiveToon => ExpectedProgram {
            category: RawState {
                queue: Some("transparent".to_owned()),
                blend: Some(alpha_blend()),
                ..RawState::default()
            },
            passes: vec![
                RawState {
                    cull: Some(MaterialCullMode::Back),
                    ..RawState::default()
                },
                RawState {
                    name: Some("OUTLINE".to_owned()),
                    blend: Some(alpha_blend()),
                    cull: Some(MaterialCullMode::Front),
                    z_write: Some(true),
                    color_mask: Some(0b1111),
                    ..RawState::default()
                },
            ],
        },
        ShaderFamily::FusionEffect | ShaderFamily::DiffuseFade => ExpectedProgram {
            category: RawState {
                queue: Some("transparent-100".to_owned()),
                blend: Some(alpha_blend()),
                color_mask: Some(0b0111),
                ..RawState::default()
            },
            passes: vec![RawState {
                name: Some("BASE".to_owned()),
                ..RawState::default()
            }],
        },
        ShaderFamily::AdditiveTest | ShaderFamily::AdditiveTestZWriteOffCullOff | ShaderFamily::SourceAlphaTest => {
            ExpectedProgram {
                category: RawState {
                    queue: Some(
                        if family == ShaderFamily::AdditiveTestZWriteOffCullOff {
                            "transparent+11"
                        } else {
                            "transparent"
                        }
                        .to_owned(),
                    ),
                    blend: Some(RawBlend::Enabled {
                        source: if family == ShaderFamily::SourceAlphaTest { MaterialBlendFactor::SourceAlpha } else { MaterialBlendFactor::One },
                        destination: MaterialBlendFactor::One,
                    }),
                    cull: Some(MaterialCullMode::Off),
                    z_write: if family == ShaderFamily::SourceAlphaTest { None } else { Some(false) },
                    alpha_test: Some(RawAlphaTest::Enabled {
                        compare: MaterialCompareFunction::Greater,
                        reference: RawAlphaReference::FloatProperty("_Cutoff".to_owned()),
                    }),
                    color_mask: Some(0b0111),
                    ..RawState::default()
                },
                passes: vec![RawState::default()],
            }
        }
        ShaderFamily::CutoutTwoSided | ShaderFamily::CutoutDefaultCulling => ExpectedProgram {
            category: RawState {
                queue: Some("transparent-100".to_owned()),
                cull: (family == ShaderFamily::CutoutTwoSided).then_some(MaterialCullMode::Off),
                ..RawState::default()
            },
            passes: vec![
                RawState {
                    alpha_test: Some(RawAlphaTest::Enabled {
                        compare: MaterialCompareFunction::GreaterEqual,
                        reference: RawAlphaReference::Literal(0.9),
                    }),
                    color_mask: Some(0b1111),
                    ..RawState::default()
                },
                RawState {
                    blend: Some(alpha_blend()),
                    z_write: Some(false),
                    color_mask: Some(0b0111),
                    ..RawState::default()
                },
            ],
        },
        ShaderFamily::CutoutZWriteOffDefaultCulling => ExpectedProgram {
            category: RawState {
                queue: Some("transparent-100".to_owned()),
                z_write: Some(false),
                alpha_test: Some(RawAlphaTest::Enabled {
                    compare: MaterialCompareFunction::Greater,
                    reference: RawAlphaReference::FloatProperty("_Cutoff".to_owned()),
                }),
                color_mask: Some(0b1111),
                ..RawState::default()
            },
            passes: vec![RawState::default()],
        },
    }
}

pub(super) fn effective_finite_float(
    properties: &[MaterialFloatProperty],
    script: &str,
    masked: &[u8],
    name: &str,
) -> Result<f64, String> {
    let matches = properties
        .iter()
        .filter(|property| property.name == name)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => exact_shader_float_default(script, masked, name),
        [property] if property.value.is_finite() => Ok(property.value),
        [_] => Err(format!(
            "required exact material float {name:?} is not finite"
        )),
        _ => Err(format!(
            "required exact material float {name:?} occurs more than once"
        )),
    }
}

pub(super) fn effective_finite_color(
    properties: &[MaterialColorProperty],
    script: &str,
    masked: &[u8],
    name: &str,
) -> Result<[f64; 4], String> {
    let matches = properties
        .iter()
        .filter(|property| property.name == name)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => exact_shader_color_default(script, masked, name),
        [property] if property.value.iter().all(|component| component.is_finite()) => {
            Ok(property.value)
        }
        [_] => Err(format!(
            "required exact material color {name:?} contains a non-finite component"
        )),
        _ => Err(format!(
            "required exact material color {name:?} occurs more than once"
        )),
    }
}

pub(super) fn exact_properties_block(script: &str, masked: &[u8]) -> Result<(usize, usize), String> {
    let (shader_start, shader_keyword_end) =
        first_identifier(masked).ok_or_else(|| "ShaderLab script is empty".to_owned())?;
    if !script[shader_start..shader_keyword_end].eq_ignore_ascii_case("Shader") {
        return Err("ShaderLab script does not begin with a Shader declaration".to_owned());
    }
    let (_, declaration_end) =
        parse_string_after(script, shader_keyword_end, "Shader declaration")?;
    let mut shader_open = declaration_end;
    while shader_open < masked.len() && masked[shader_open].is_ascii_whitespace() {
        shader_open += 1;
    }
    if masked.get(shader_open) != Some(&b'{') {
        return Err("ShaderLab declaration is not followed by its body".to_owned());
    }
    let shader_close = matching_brace(masked, shader_open, masked.len())
        .ok_or_else(|| "ShaderLab body is unterminated".to_owned())?;
    let blocks = direct_child_blocks(script, masked, "Properties", shader_open + 1, shader_close)?;
    match blocks.as_slice() {
        [properties] => Ok((properties.open, properties.close)),
        [] => Err("ShaderLab script has no direct Properties block".to_owned()),
        _ => Err("ShaderLab script has more than one direct Properties block".to_owned()),
    }
}

pub(super) fn queue_from_tag(tag: &str) -> Result<i32, String> {
    match tag {
        "transparent-100" => Ok(2_900),
        "transparent" => Ok(3_000),
        "transparent+3" => Ok(3_003),
        "transparent+10" => Ok(3_010),
        "transparent+11" => Ok(3_011),
        _ => Err(format!(
            "unsupported exact ShaderLab Queue tag {tag:?} in Dexter material"
        )),
    }
}

pub(super) fn set_once<T>(slot: &mut Option<T>, value: T, field: &str, context: &str) -> Result<(), String> {
    if slot.is_some() {
        Err(format!("{context} declares {field} more than once"))
    } else {
        *slot = Some(value);
        Ok(())
    }
}

pub(super) fn mask_shaderlab(script: &str) -> Result<Vec<u8>, String> {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Mode {
        Code,
        String,
        LineComment,
        BlockComment,
    }

    let bytes = script.as_bytes();
    let mut masked = bytes.to_vec();
    let mut mode = Mode::Code;
    let mut escaped = false;
    let mut index = 0;
    while index < bytes.len() {
        match mode {
            Mode::Code if bytes[index] == b'"' => {
                masked[index] = b' ';
                mode = Mode::String;
                escaped = false;
            }
            Mode::Code if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/') => {
                masked[index] = b' ';
                masked[index + 1] = b' ';
                mode = Mode::LineComment;
                index += 1;
            }
            Mode::Code if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') => {
                masked[index] = b' ';
                masked[index + 1] = b' ';
                mode = Mode::BlockComment;
                index += 1;
            }
            Mode::String => {
                if bytes[index] != b'\n' && bytes[index] != b'\r' {
                    masked[index] = b' ';
                }
                if escaped {
                    escaped = false;
                } else if bytes[index] == b'\\' {
                    escaped = true;
                } else if bytes[index] == b'"' {
                    mode = Mode::Code;
                }
            }
            Mode::LineComment => {
                if bytes[index] == b'\n' {
                    mode = Mode::Code;
                } else if bytes[index] != b'\r' {
                    masked[index] = b' ';
                }
            }
            Mode::BlockComment => {
                if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    masked[index] = b' ';
                    masked[index + 1] = b' ';
                    mode = Mode::Code;
                    index += 1;
                } else if bytes[index] != b'\n' && bytes[index] != b'\r' {
                    masked[index] = b' ';
                }
            }
            Mode::Code => {}
        }
        index += 1;
    }

    match mode {
        Mode::Code | Mode::LineComment => Ok(masked),
        Mode::String => Err("unterminated string in ShaderLab source".to_owned()),
        Mode::BlockComment => Err("unterminated block comment in ShaderLab source".to_owned()),
    }
}

pub(super) fn first_identifier(masked: &[u8]) -> Option<(usize, usize)> {
    let mut index = 0;
    while index < masked.len() {
        if is_identifier_start(masked[index]) {
            let start = index;
            index += 1;
            while index < masked.len() && is_identifier_continue(masked[index]) {
                index += 1;
            }
            return Some((start, index));
        }
        index += 1;
    }
    None
}

pub(super) fn block_from_keyword(
    _script: &str,
    masked: &[u8],
    keyword: &str,
    keyword_start: usize,
    end: usize,
) -> Result<Block, String> {
    let mut open = keyword_start + keyword.len();
    while open < end && masked[open].is_ascii_whitespace() {
        open += 1;
    }
    if masked.get(open) != Some(&b'{') {
        return Err(format!("ShaderLab {keyword} is not followed by a block"));
    }
    let close = matching_brace(masked, open, end)
        .ok_or_else(|| format!("ShaderLab {keyword} block is unterminated"))?;
    Ok(Block { open, close })
}

pub(super) fn matching_brace(masked: &[u8], open: usize, end: usize) -> Option<usize> {
    let mut depth = 0_usize;
    for (offset, byte) in masked[open..end].iter().copied().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

pub(super) fn brace_depth(masked: &[u8], start: usize, end: usize) -> usize {
    let mut depth = 0_usize;
    for byte in &masked[start..end] {
        match byte {
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}
