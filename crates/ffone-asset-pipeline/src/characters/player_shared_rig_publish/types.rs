use super::*;

#[derive(Clone, Copy)]
pub(super) struct GenderSpec {
    pub(super) gender: PlayerRigGender,
    pub(super) route: &'static str,
    pub(super) root_name: &'static str,
    pub(super) expected_root_path_id: i64,
    pub(super) expected_actor_bones: usize,
    pub(super) expected_clip_count: usize,
    pub(super) expected_creator_choices: usize,
    pub(super) expected_unique_creator_parts: usize,
    pub(super) transform_indices_field: &'static str,
    pub(super) skeleton_glb: &'static str,
    pub(super) clips: [(&'static str, i64); 5],
    pub(super) default_parts: [&'static str; 5],
}

#[derive(Clone, Copy)]
pub(super) enum Vec3Track {
    Translation,
    Scale,
}

#[derive(Clone)]
pub(super) struct LegacySkinRemap {
    pub(super) renderer_name: String,
    pub(super) renderer_path_id: i64,
    pub(super) table_path_id: i64,
    pub(super) actor_indices: Vec<u32>,
}

pub(super) struct BitReader<'a> {
    pub(super) data: &'a [u8],
    pub(super) bit_size: u32,
    pub(super) bit_count: u32,
    pub(super) index: usize,
    pub(super) byte: u64,
}

impl<'a> BitReader<'a> {
    pub(super) fn new(data: &'a [u8], bit_size: u32) -> Self {
        let mut reader = Self {
            data,
            bit_size,
            bit_count: 8,
            index: 0,
            byte: 0,
        };
        reader.byte = u64::from(reader.next_byte());
        reader
    }

    pub(super) fn next_byte(&mut self) -> u8 {
        let value = self.data.get(self.index).copied().unwrap_or(0);
        self.index += usize::from(self.index < self.data.len());
        value
    }

    pub(super) fn read(&mut self) -> u32 {
        while self.bit_count < self.bit_size {
            self.byte |= u64::from(self.next_byte()) << self.bit_count;
            self.bit_count += 8;
        }
        let mask = if self.bit_size == 32 {
            u64::from(u32::MAX)
        } else {
            (1_u64 << self.bit_size) - 1
        };
        let value = self.byte & mask;
        self.byte >>= self.bit_size;
        self.bit_count -= self.bit_size;
        value as u32
    }
}
