use std::io::{Cursor, Read, Seek, SeekFrom, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    Little,
    Big,
}

pub struct BinaryReader {
    cursor: Cursor<Vec<u8>>,
    pub endian: Endian,
}

pub struct BinaryWriter {
    cursor: Cursor<Vec<u8>>,
    pub endian: Endian,
}

impl BinaryWriter {
    pub fn new(endian: Endian) -> Self {
        Self {
            cursor: Cursor::new(Vec::new()),
            endian,
        }
    }

    pub fn position(&self) -> u64 {
        self.cursor.position()
    }

    pub fn seek(&mut self, position: u64) -> Result<(), String> {
        self.cursor
            .seek(SeekFrom::Start(position))
            .map(|_| ())
            .map_err(|err| err.to_string())
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.cursor.into_inner()
    }

    pub fn write_exact(&mut self, data: &[u8]) -> Result<(), String> {
        self.cursor.write_all(data).map_err(|err| err.to_string())
    }

    pub fn align4(&mut self) -> Result<(), String> {
        let old = self.position();
        let new = (old + 3) & !3;
        if new > old {
            self.write_exact(&vec![0; (new - old) as usize])?;
        }
        Ok(())
    }

    pub fn write_u8(&mut self, value: u8) -> Result<(), String> {
        self.write_exact(&[value])
    }

    pub fn write_i8(&mut self, value: i8) -> Result<(), String> {
        self.write_u8(value as u8)
    }

    pub fn write_bool(&mut self, value: bool) -> Result<(), String> {
        self.write_i8(i8::from(value))
    }

    pub fn write_i16(&mut self, value: i16) -> Result<(), String> {
        self.write_exact(&match self.endian {
            Endian::Little => value.to_le_bytes(),
            Endian::Big => value.to_be_bytes(),
        })
    }

    pub fn write_u16(&mut self, value: u16) -> Result<(), String> {
        self.write_exact(&match self.endian {
            Endian::Little => value.to_le_bytes(),
            Endian::Big => value.to_be_bytes(),
        })
    }

    pub fn write_i32(&mut self, value: i32) -> Result<(), String> {
        self.write_exact(&match self.endian {
            Endian::Little => value.to_le_bytes(),
            Endian::Big => value.to_be_bytes(),
        })
    }

    pub fn write_u32(&mut self, value: u32) -> Result<(), String> {
        self.write_exact(&match self.endian {
            Endian::Little => value.to_le_bytes(),
            Endian::Big => value.to_be_bytes(),
        })
    }

    pub fn write_i64(&mut self, value: i64) -> Result<(), String> {
        self.write_exact(&match self.endian {
            Endian::Little => value.to_le_bytes(),
            Endian::Big => value.to_be_bytes(),
        })
    }

    pub fn write_u64(&mut self, value: u64) -> Result<(), String> {
        self.write_exact(&match self.endian {
            Endian::Little => value.to_le_bytes(),
            Endian::Big => value.to_be_bytes(),
        })
    }

    pub fn write_f32(&mut self, value: f32) -> Result<(), String> {
        self.write_u32(value.to_bits())
    }

    pub fn write_f64(&mut self, value: f64) -> Result<(), String> {
        self.write_u64(value.to_bits())
    }

    pub fn write_cstring(&mut self, value: &str) -> Result<(), String> {
        self.write_exact(value.as_bytes())?;
        self.write_u8(0)
    }

    pub fn write_string_bytes(&mut self, value: &str) -> Result<(), String> {
        self.write_exact(value.as_bytes())
    }
}

impl BinaryReader {
    pub fn new(data: Vec<u8>, endian: Endian) -> Self {
        Self {
            cursor: Cursor::new(data),
            endian,
        }
    }

    pub fn from_slice(data: &[u8], endian: Endian) -> Self {
        Self::new(data.to_vec(), endian)
    }

    pub fn position(&self) -> u64 {
        self.cursor.position()
    }

    pub fn remaining(&self) -> usize {
        let position = self.cursor.position() as usize;
        self.cursor.get_ref().len().saturating_sub(position)
    }

    pub fn seek(&mut self, position: u64) -> Result<(), String> {
        self.cursor
            .seek(SeekFrom::Start(position))
            .map(|_| ())
            .map_err(|err| err.to_string())
    }

    pub fn skip(&mut self, bytes: u64) -> Result<(), String> {
        self.cursor
            .seek(SeekFrom::Current(bytes as i64))
            .map(|_| ())
            .map_err(|err| err.to_string())
    }

    pub fn align4(&mut self) -> Result<(), String> {
        let old = self.position();
        let new = (old + 3) & !3;
        if new > old {
            self.seek(new)?;
        }
        Ok(())
    }

    pub fn read_exact_vec(&mut self, size: usize) -> Result<Vec<u8>, String> {
        if size > self.remaining() {
            return Err(format!(
                "requested {size} byte(s), only {} remain",
                self.remaining()
            ));
        }
        let mut data = vec![0; size];
        self.cursor
            .read_exact(&mut data)
            .map_err(|err| err.to_string())?;
        Ok(data)
    }

    pub fn read_u8(&mut self) -> Result<u8, String> {
        Ok(self.read_exact_vec(1)?[0])
    }

    pub fn read_i8(&mut self) -> Result<i8, String> {
        Ok(self.read_u8()? as i8)
    }

    pub fn read_bool(&mut self) -> Result<bool, String> {
        Ok(self.read_i8()? != 0)
    }

    pub fn read_i16(&mut self) -> Result<i16, String> {
        let data = self.read_exact_vec(2)?;
        Ok(match self.endian {
            Endian::Little => i16::from_le_bytes([data[0], data[1]]),
            Endian::Big => i16::from_be_bytes([data[0], data[1]]),
        })
    }

    pub fn read_u16(&mut self) -> Result<u16, String> {
        let data = self.read_exact_vec(2)?;
        Ok(match self.endian {
            Endian::Little => u16::from_le_bytes([data[0], data[1]]),
            Endian::Big => u16::from_be_bytes([data[0], data[1]]),
        })
    }

    pub fn read_i32(&mut self) -> Result<i32, String> {
        let data = self.read_exact_vec(4)?;
        Ok(match self.endian {
            Endian::Little => i32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            Endian::Big => i32::from_be_bytes([data[0], data[1], data[2], data[3]]),
        })
    }

    pub fn read_u32(&mut self) -> Result<u32, String> {
        let data = self.read_exact_vec(4)?;
        Ok(match self.endian {
            Endian::Little => u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            Endian::Big => u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
        })
    }

    pub fn read_i64(&mut self) -> Result<i64, String> {
        let data = self.read_exact_vec(8)?;
        Ok(match self.endian {
            Endian::Little => i64::from_le_bytes(data.try_into().map_err(|_| "bad i64 read")?),
            Endian::Big => i64::from_be_bytes(data.try_into().map_err(|_| "bad i64 read")?),
        })
    }

    pub fn read_u64(&mut self) -> Result<u64, String> {
        let data = self.read_exact_vec(8)?;
        Ok(match self.endian {
            Endian::Little => u64::from_le_bytes(data.try_into().map_err(|_| "bad u64 read")?),
            Endian::Big => u64::from_be_bytes(data.try_into().map_err(|_| "bad u64 read")?),
        })
    }

    pub fn read_f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_bits(self.read_u32()?))
    }

    pub fn read_f64(&mut self) -> Result<f64, String> {
        Ok(f64::from_bits(self.read_u64()?))
    }

    pub fn read_cstring(&mut self) -> Result<String, String> {
        let mut data = Vec::new();
        loop {
            let value = self.read_u8()?;
            if value == 0 {
                break;
            }
            data.push(value);
        }
        Ok(String::from_utf8_lossy(&data).to_string())
    }

    pub fn read_string_bytes(&mut self, size: usize) -> Result<String, String> {
        let data = self.read_exact_vec(size)?;
        Ok(String::from_utf8_lossy(&data).to_string())
    }
}

pub struct BitReader<'a> {
    data: &'a [u8],
    bit_size: u32,
    bit_count: u32,
    index: usize,
    byte: u64,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8], bit_size: u32) -> Self {
        let mut reader = Self {
            data,
            bit_size,
            bit_count: 8,
            index: 0,
            byte: 0,
        };
        reader.byte = reader.next_byte() as u64;
        reader
    }

    fn next_byte(&mut self) -> u8 {
        if self.index >= self.data.len() {
            return 0;
        }
        let value = self.data[self.index];
        self.index += 1;
        value
    }

    pub fn read(&mut self) -> u32 {
        if self.bit_size == 8 && self.bit_count == 0 {
            return self.next_byte() as u32;
        }

        while self.bit_count < self.bit_size {
            let value = self.next_byte() as u64;
            self.byte |= value << self.bit_count;
            self.bit_count += 8;
        }

        let mask = if self.bit_size >= 32 {
            u64::from(u32::MAX)
        } else {
            (1_u64 << self.bit_size) - 1
        };
        let ret = self.byte & mask;
        self.byte >>= self.bit_size;
        self.bit_count -= self.bit_size;
        ret as u32
    }
}
