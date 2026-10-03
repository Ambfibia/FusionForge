use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{json, Value as JsonValue};

use super::{
    preview::{decode_texture, extract_mesh, DecodedTexture, MeshData},
    unity::{
        object_name, pair_name_value, value_array, vector, Pointer, UnityEnvironment, UnityValue,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TextureFormat {
    Unconfigured = 0,
    Alpha8 = 1,
    Argb4444 = 2,
    Rgb24 = 3,
    Rgba32 = 4,
    Argb32 = 5,
    Rgb565 = 7,
    Dxt1 = 10,
    Dxt3 = 11,
    Dxt5 = 12,
    Rgba4444 = 13,
    Bgra32 = 14,
    Bc6h = 24,
    Bc7 = 25,
    Dxt1Crunched = 28,
    Dxt5Crunched = 29,
    PvrtcRgb2 = 30,
    PvrtcRgba2 = 31,
    PvrtcRgb4 = 32,
    PvrtcRgba4 = 33,
    EtcRgb4 = 34,
    AtcRgb4 = 35,
    AtcRgba8 = 36,
    AtfRgbDxt1 = 38,
    AtfRgbaJpg = 39,
    AtfRgbJpg = 40,
    EacR = 41,
    EacRSigned = 42,
    EacRg = 43,
    EacRgSigned = 44,
    Etc2Rgb = 45,
    Etc2Rgba1 = 46,
    Etc2Rgba8 = 47,
    AstcRgb4x4 = 48,
    AstcRgb5x5 = 49,
    AstcRgb6x6 = 50,
    AstcRgb8x8 = 51,
    AstcRgb10x10 = 52,
    AstcRgb12x12 = 53,
    AstcRgba4x4 = 54,
    AstcRgba5x5 = 55,
    AstcRgba6x6 = 56,
    AstcRgba8x8 = 57,
    AstcRgba10x10 = 58,
    AstcRgba12x12 = 59,
}

impl TextureFormat {
    pub fn from_i32(value: i32) -> Option<Self> {
        Some(match value {
            0 => Self::Unconfigured,
            1 => Self::Alpha8,
            2 => Self::Argb4444,
            3 => Self::Rgb24,
            4 => Self::Rgba32,
            5 => Self::Argb32,
            7 => Self::Rgb565,
            10 => Self::Dxt1,
            11 => Self::Dxt3,
            12 => Self::Dxt5,
            13 => Self::Rgba4444,
            14 => Self::Bgra32,
            24 => Self::Bc6h,
            25 => Self::Bc7,
            28 => Self::Dxt1Crunched,
            29 => Self::Dxt5Crunched,
            30 => Self::PvrtcRgb2,
            31 => Self::PvrtcRgba2,
            32 => Self::PvrtcRgb4,
            33 => Self::PvrtcRgba4,
            34 => Self::EtcRgb4,
            35 => Self::AtcRgb4,
            36 => Self::AtcRgba8,
            38 => Self::AtfRgbDxt1,
            39 => Self::AtfRgbaJpg,
            40 => Self::AtfRgbJpg,
            41 => Self::EacR,
            42 => Self::EacRSigned,
            43 => Self::EacRg,
            44 => Self::EacRgSigned,
            45 => Self::Etc2Rgb,
            46 => Self::Etc2Rgba1,
            47 => Self::Etc2Rgba8,
            48 => Self::AstcRgb4x4,
            49 => Self::AstcRgb5x5,
            50 => Self::AstcRgb6x6,
            51 => Self::AstcRgb8x8,
            52 => Self::AstcRgb10x10,
            53 => Self::AstcRgb12x12,
            54 => Self::AstcRgba4x4,
            55 => Self::AstcRgba5x5,
            56 => Self::AstcRgba6x6,
            57 => Self::AstcRgba8x8,
            58 => Self::AstcRgba10x10,
            59 => Self::AstcRgba12x12,
            _ => return None,
        })
    }

    pub fn supports_preview(self) -> bool {
        matches!(
            self,
            Self::Alpha8
                | Self::Argb4444
                | Self::Rgba4444
                | Self::Rgb565
                | Self::Rgb24
                | Self::Rgba32
                | Self::Argb32
                | Self::Bgra32
                | Self::Dxt1
                | Self::Dxt3
                | Self::Dxt5
                | Self::Dxt1Crunched
                | Self::Dxt5Crunched
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ObjectView<'a> {
    value: &'a UnityValue,
}

impl<'a> ObjectView<'a> {
    pub fn new(value: &'a UnityValue) -> Self {
        Self { value }
    }

    pub fn value(&self) -> &'a UnityValue {
        self.value
    }

    pub fn name(&self) -> String {
        object_name(self.value)
    }

    pub fn field(&self, name: &str) -> Option<&'a UnityValue> {
        self.value.get(name)
    }

    pub fn bool_field(&self, name: &str) -> Option<bool> {
        self.field(name)
            .and_then(UnityValue::as_i64)
            .map(|value| value != 0)
    }

    pub fn i64_field(&self, name: &str) -> Option<i64> {
        self.field(name).and_then(UnityValue::as_i64)
    }

    pub fn f64_field(&self, name: &str) -> Option<f64> {
        self.field(name).and_then(UnityValue::as_f64)
    }

    pub fn pointer_field(&self, name: &str) -> Option<&'a Pointer> {
        self.field(name).and_then(UnityValue::as_pointer)
    }

    pub fn array_field(&self, name: &str) -> &'a [UnityValue] {
        value_array(self.field(name))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GameObjectView<'a>(ObjectView<'a>);

impl<'a> GameObjectView<'a> {
    pub fn new(value: &'a UnityValue) -> Self {
        Self(ObjectView::new(value))
    }

    pub fn name(&self) -> String {
        self.0.name()
    }

    pub fn active(&self) -> Option<bool> {
        self.0.bool_field("m_IsActive")
    }

    pub fn layer(&self) -> Option<i64> {
        self.0.i64_field("m_Layer")
    }

    pub fn components(&self) -> &'a [UnityValue] {
        self.0.array_field("m_Component")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TransformView<'a>(ObjectView<'a>);

impl<'a> TransformView<'a> {
    pub fn new(value: &'a UnityValue) -> Self {
        Self(ObjectView::new(value))
    }

    pub fn game_object(&self) -> Option<&'a Pointer> {
        self.0.pointer_field("m_GameObject")
    }

    pub fn position(&self) -> Option<(f64, f64, f64)> {
        vector(self.0.field("m_LocalPosition"))
    }

    pub fn scale(&self) -> Option<(f64, f64, f64)> {
        vector(self.0.field("m_LocalScale"))
    }

    pub fn parent(&self) -> Option<&'a Pointer> {
        self.0.pointer_field("m_Father")
    }

    pub fn children(&self) -> &'a [UnityValue] {
        self.0.array_field("m_Children")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Texture2DView<'a>(ObjectView<'a>);

impl<'a> Texture2DView<'a> {
    pub fn new(value: &'a UnityValue) -> Self {
        Self(ObjectView::new(value))
    }

    pub fn width(&self) -> Option<u32> {
        self.0.i64_field("m_Width").map(|value| value as u32)
    }

    pub fn height(&self) -> Option<u32> {
        self.0.i64_field("m_Height").map(|value| value as u32)
    }

    pub fn format(&self) -> Option<TextureFormat> {
        TextureFormat::from_i32(self.0.i64_field("m_TextureFormat")? as i32)
    }

    pub fn raw_image_data(&self, env: &UnityEnvironment) -> Option<Vec<u8>> {
        if let Some(data) = self.0.field("image data").and_then(UnityValue::as_bytes) {
            if !data.is_empty() {
                return Some(data.to_vec());
            }
        }
        self.0
            .field("m_StreamData")
            .and_then(|value| env.read_streaming_data(value))
    }

    pub fn decoded(&self, env: &UnityEnvironment) -> Option<DecodedTexture> {
        decode_texture(env, self.0.value())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MaterialView<'a>(ObjectView<'a>);

impl<'a> MaterialView<'a> {
    pub fn new(value: &'a UnityValue) -> Self {
        Self(ObjectView::new(value))
    }

    pub fn shader(&self) -> Option<&'a Pointer> {
        self.0.pointer_field("m_Shader")
    }

    pub fn saved_properties(&self) -> JsonValue {
        let Some(saved) = self.0.field("m_SavedProperties") else {
            return json!({});
        };
        let mut output = serde_json::Map::new();
        for key in ["m_TexEnvs", "m_Floats", "m_Colors"] {
            let entries = value_array(saved.get(key));
            let mut group = serde_json::Map::new();
            for entry in entries {
                if let Some((name, value)) = pair_name_value(entry) {
                    group.insert(name.to_string(), value.to_json_sample());
                }
            }
            output.insert(key.to_string(), JsonValue::Object(group));
        }
        JsonValue::Object(output)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MeshView<'a>(ObjectView<'a>);

impl<'a> MeshView<'a> {
    pub fn new(value: &'a UnityValue) -> Self {
        Self(ObjectView::new(value))
    }

    pub fn compression(&self) -> i64 {
        self.0.i64_field("m_MeshCompression").unwrap_or(0)
    }

    pub fn use_16bit_indices(&self) -> Option<bool> {
        self.0.bool_field("m_Use16BitIndices")
    }

    pub fn submeshes(&self) -> &'a [UnityValue] {
        self.0.array_field("m_SubMeshes")
    }

    pub fn extract(&self) -> Option<MeshData> {
        extract_mesh(self.0.value())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RendererView<'a>(ObjectView<'a>);

impl<'a> RendererView<'a> {
    pub fn new(value: &'a UnityValue) -> Self {
        Self(ObjectView::new(value))
    }

    pub fn enabled(&self) -> Option<bool> {
        self.0.bool_field("m_Enabled")
    }

    pub fn materials(&self) -> &'a [UnityValue] {
        self.0.array_field("m_Materials")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ColliderView<'a>(ObjectView<'a>);

impl<'a> ColliderView<'a> {
    pub fn new(value: &'a UnityValue) -> Self {
        Self(ObjectView::new(value))
    }

    pub fn material(&self) -> Option<&'a Pointer> {
        self.0.pointer_field("m_Material")
    }

    pub fn is_trigger(&self) -> Option<bool> {
        self.0.bool_field("m_IsTrigger")
    }

    pub fn center(&self) -> Option<(f64, f64, f64)> {
        vector(self.0.field("m_Center"))
    }

    pub fn size(&self) -> Option<(f64, f64, f64)> {
        vector(self.0.field("m_Size"))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TextAssetView<'a>(ObjectView<'a>);

impl<'a> TextAssetView<'a> {
    pub fn new(value: &'a UnityValue) -> Self {
        Self(ObjectView::new(value))
    }

    pub fn bytes(&self) -> Option<&'a [u8]> {
        self.0.field("m_Script").and_then(UnityValue::as_bytes)
    }

    pub fn text(&self) -> Option<String> {
        self.bytes()
            .map(|bytes| String::from_utf8_lossy(bytes).to_string())
    }
}

pub fn object_map(value: &UnityValue) -> Option<&BTreeMap<String, UnityValue>> {
    value.as_object()
}
