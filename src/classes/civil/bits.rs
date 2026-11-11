//! Bounded bit cursor over a DWG main stream.
//!
//! Civil 3D bodies hold large arrays at arbitrary bit offsets. Readers here
//! return `None` past `end` instead of putting a shared reader into an error
//! state, so a caller can probe a position and move on.

pub(super) struct Bits<'a> {
    data: &'a [u8],
    pub pos: usize,
    end: usize,
}

impl<'a> Bits<'a> {
    pub fn new(data: &'a [u8], pos: usize, end: usize) -> Self {
        Self {
            data,
            pos,
            end: end.min(data.len() * 8),
        }
    }

    pub fn remaining(&self) -> usize {
        self.end.saturating_sub(self.pos)
    }

    fn bits(&mut self, count: usize) -> Option<u8> {
        if self.remaining() < count {
            return None;
        }
        let mut value = 0u8;
        for _ in 0..count {
            let bit = (self.data[self.pos / 8] >> (7 - self.pos % 8)) & 1;
            value = (value << 1) | bit;
            self.pos += 1;
        }
        Some(value)
    }

    fn byte(&mut self) -> Option<u8> {
        if self.remaining() < 8 {
            return None;
        }
        let index = self.pos / 8;
        let shift = self.pos % 8;
        let value = if shift == 0 {
            self.data[index]
        } else {
            (self.data[index] << shift) | (self.data[index + 1] >> (8 - shift))
        };
        self.pos += 8;
        Some(value)
    }

    fn bytes<const N: usize>(&mut self) -> Option<[u8; N]> {
        let mut out = [0u8; N];
        for byte in &mut out {
            *byte = self.byte()?;
        }
        Some(out)
    }

    /// `count` (at most 8) raw bits as a number.
    pub fn bits_value(&mut self, count: usize) -> Option<u8> {
        self.bits(count)
    }

    /// One raw byte.
    pub fn raw_short_byte(&mut self) -> Option<u8> {
        self.byte()
    }

    /// RL: raw 32-bit little-endian integer.
    pub fn raw_long(&mut self) -> Option<i32> {
        self.bytes::<4>().map(i32::from_le_bytes)
    }

    /// RD: raw 64-bit little-endian double.
    pub fn raw_double(&mut self) -> Option<f64> {
        self.bytes::<8>().map(f64::from_le_bytes)
    }

    pub fn bit(&mut self) -> Option<bool> {
        self.bits(1).map(|b| b == 1)
    }

    /// BS: bit-coded 16-bit integer.
    pub fn bit_short(&mut self) -> Option<i16> {
        match self.bits(2)? {
            0 => self.bytes::<2>().map(i16::from_le_bytes),
            1 => self.byte().map(i16::from),
            2 => Some(0),
            _ => Some(256),
        }
    }

    /// BS written in its one-byte form (`01` + RC); `None` for other forms.
    pub fn compact_short(&mut self) -> Option<i16> {
        (self.bits(2)? == 1).then_some(())?;
        self.byte().map(i16::from)
    }

    /// BD: bit-coded double.
    pub fn bit_double(&mut self) -> Option<f64> {
        match self.bits(2)? {
            0 => self.raw_double(),
            1 => Some(1.0),
            2 => Some(0.0),
            _ => None,
        }
    }

    /// BL: bit-coded 32-bit integer.
    pub fn bit_long(&mut self) -> Option<i32> {
        match self.bits(2)? {
            0 => self.raw_long(),
            1 => self.byte().map(i32::from),
            2 => Some(0),
            _ => None,
        }
    }
}
