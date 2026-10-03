#[derive(Debug, Clone, Copy)]
pub(super) struct OggVorbisMetadata {
    pub(super) sample_rate: u32,
    pub(super) duration_seconds: f64,
}

#[derive(Debug, Clone)]
pub(super) struct FontSummary {
    pub(super) name: String,
    pub(super) has_cyrillic: bool,
    pub(super) line_spacing: f64,
}

#[derive(Debug, Clone)]
pub(super) struct GlyphBitmap {
    pub(super) code: u32,
    pub(super) render_code: u32,
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) origin_x: i32,
    pub(super) origin_y: i32,
    pub(super) advance: i32,
    pub(super) rows: Vec<Vec<u8>>,
}
