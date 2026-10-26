//! Bounded reading of cache records. Cache files can come from anywhere (a player's computer,
//! a copy sent with a report): every read is checked against the record's end and every count
//! against a cap, so a crafted record gives an error, never a panic or a large allocation.

use std::fmt;

/// Why a cache record was not decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WdbError {
    /// The record ends before a field.
    Truncated,
    /// A count or a size above what the game sends.
    Limit(&'static str),
    /// A field with a value the layout does not allow.
    Invalid(&'static str),
    /// Bytes left after the last field: not this layout.
    Trailing,
}

impl fmt::Display for WdbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => f.write_str("truncated record"),
            Self::Limit(what) => write!(f, "too many or too large: {what}"),
            Self::Invalid(what) => write!(f, "bad value: {what}"),
            Self::Trailing => f.write_str("bytes after the record's last field"),
        }
    }
}

impl std::error::Error for WdbError {}

pub type Result<T> = std::result::Result<T, WdbError>;

/// Little endian fields read in order.
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn at(data: &'a [u8], pos: usize) -> Result<Self> {
        if pos > data.len() {
            return Err(WdbError::Truncated);
        }
        Ok(Self { data, pos })
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or(WdbError::Truncated)?;
        let bytes = self.data.get(self.pos..end).ok_or(WdbError::Truncated)?;
        self.pos = end;
        Ok(bytes)
    }

    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.take(n).map(|_| ())
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut out = [0; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    pub fn i8(&mut self) -> Result<i8> {
        Ok(i8::from_le_bytes(self.array()?))
    }

    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    /// A count of following entries, at most `max`.
    pub fn count(&mut self, max: usize, what: &'static str) -> Result<usize> {
        let n = self.u32()? as usize;
        if n > max {
            return Err(WdbError::Limit(what));
        }
        Ok(n)
    }

    /// A NUL terminated string of at most `max` bytes (without the NUL).
    pub fn cstr(&mut self, max: usize, what: &'static str) -> Result<&'a [u8]> {
        let rest = self.data.get(self.pos..).ok_or(WdbError::Truncated)?;
        let len = rest
            .iter()
            .take(max + 1)
            .position(|b| *b == 0)
            .ok_or(if rest.len() > max {
                WdbError::Limit(what)
            } else {
                WdbError::Truncated
            })?;
        let s = self.take(len)?;
        self.pos += 1;
        Ok(s)
    }

    /// Fails unless every byte was read.
    pub fn finish(&self) -> Result<()> {
        if self.pos == self.data.len() {
            Ok(())
        } else {
            Err(WdbError::Trailing)
        }
    }
}

/// Bit-packed fields, most significant bit first, as the game writes string lengths.
pub struct Bits<'a> {
    data: &'a [u8],
    bit: usize,
}

impl<'a> Bits<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, bit: 0 }
    }

    pub fn read(&mut self, n: u32) -> Result<u32> {
        debug_assert!(n <= 32);
        let mut v = 0u32;
        for _ in 0..n {
            let byte = self.data.get(self.bit / 8).ok_or(WdbError::Truncated)?;
            v = (v << 1) | u32::from((byte >> (7 - self.bit % 8)) & 1);
            self.bit += 1;
        }
        Ok(v)
    }

    /// The byte after the last bit read (the game pads to a byte before the next fields).
    pub fn flush(&self) -> usize {
        self.bit.div_ceil(8)
    }
}

/// UTF-8 text of a record.
pub fn text(bytes: &[u8], what: &'static str) -> Result<String> {
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| WdbError::Invalid(what))
}

/// Bit-packed fields (value, width), most significant bit first, padded to a byte.
#[cfg(test)]
pub fn pack_bits(fields: &[(u32, u32)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut bit = 0;
    for &(value, width) in fields {
        for i in (0..width).rev() {
            if bit % 8 == 0 {
                out.push(0);
            }
            if (value >> i) & 1 == 1 {
                *out.last_mut().unwrap() |= 0x80 >> (bit % 8);
            }
            bit += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// The client's cache files are not ours to trust: whatever bytes, whatever the reads
        /// asked, the reader answers or refuses, and never goes past the data.
        #[test]
        fn any_bytes_are_read_or_refused(
            data in proptest::collection::vec(any::<u8>(), 0..256),
            asks in proptest::collection::vec((0u8..6, any::<u32>()), 0..32),
        ) {
            let mut r = Reader::new(&data);
            for (kind, n) in asks {
                let n = n as usize;
                let _ = match kind {
                    0 => r.u32().map(|_| ()),
                    1 => r.i32().map(|_| ()),
                    2 => r.take(n % 300).map(|_| ()),
                    3 => r.count(n % 64, "count").map(|_| ()),
                    4 => r.cstr(n % 64, "text").map(|_| ()),
                    _ => r.skip(n),
                };
            }
        }
    }

    #[test]
    fn reads_are_bounded() {
        let mut r = Reader::new(&[1, 0, 0, 0, 7]);
        assert_eq!(r.u32(), Ok(1));
        assert_eq!(r.i32(), Err(WdbError::Truncated));
        assert_eq!(r.u8(), Ok(7));
        assert_eq!(r.finish(), Ok(()));
        assert_eq!(Reader::new(&[]).skip(usize::MAX), Err(WdbError::Truncated));
        assert!(Reader::at(&[0], 2).is_err());
    }

    #[test]
    fn counts_and_strings_are_capped() {
        assert_eq!(Reader::new(&[9, 0, 0, 0]).count(8, "n"), Err(WdbError::Limit("n")));
        let mut r = Reader::new(b"ab\0cd");
        assert_eq!(r.cstr(4, "s"), Ok(&b"ab"[..]));
        assert_eq!(r.cstr(4, "s"), Err(WdbError::Truncated));
        assert_eq!(Reader::new(b"abcdef\0").cstr(4, "s"), Err(WdbError::Limit("s")));
    }

    #[test]
    fn reads_bits_msb_first() {
        assert_eq!(pack_bits(&[(0b101, 3), (1, 7)]), [0b1010_0000, 0b0100_0000]);
        let mut b = Bits::new(&[0b1010_0000, 0b0100_0000]);
        assert_eq!(b.read(3), Ok(0b101));
        assert_eq!(b.read(7), Ok(1));
        assert_eq!(b.flush(), 2);
        assert_eq!(b.read(6), Ok(0));
        assert_eq!(b.read(1), Err(WdbError::Truncated));
    }
}
