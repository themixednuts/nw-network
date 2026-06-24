//! Read and write buffers for packet serialization.

use super::error::MarshalerError;
use std::io::{Read, Result as IoResult, Write};

/// Integer byte order used by [`ReadBuffer`] and [`WriteBuffer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Endian {
    #[default]
    BigEndian,
    LittleEndian,
}

/// Default packet byte order.
pub const CARRIER_ENDIAN: Endian = Endian::BigEndian;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReadBufferMark(usize);

impl ReadBufferMark {
    #[must_use]
    pub const fn position(self) -> usize {
        self.0
    }
}

/// Borrowing read cursor over packet bytes.
pub struct ReadBuffer<'a> {
    data: &'a [u8],
    position: usize,
    endian: Endian,
}

impl<'a> ReadBuffer<'a> {
    /// Create a read buffer with explicit endianness.
    #[must_use]
    pub fn new(endian: Endian, data: &'a [u8]) -> Self {
        Self {
            data,
            position: 0,
            endian,
        }
    }

    /// Create a big-endian read buffer.
    #[must_use]
    pub fn carrier(data: &'a [u8]) -> Self {
        Self::new(CARRIER_ENDIAN, data)
    }

    /// Create a little-endian read buffer.
    #[must_use]
    pub fn little_endian(data: &'a [u8]) -> Self {
        Self::new(Endian::LittleEndian, data)
    }

    /// Read a single byte.
    ///
    /// # Errors
    ///
    /// Returns [`MarshalerError::BufferUnderrun`] when no byte remains.
    pub fn read_u8(&mut self) -> Result<u8, MarshalerError> {
        if self.position < self.data.len() {
            let value = self.data[self.position];
            self.position += 1;
            Ok(value)
        } else {
            Err(MarshalerError::buffer_underrun(self.left(), 1))
        }
    }

    /// Read a boolean encoded as a raw byte value.
    ///
    /// # Errors
    ///
    /// Returns an error when the buffer is empty or the byte is not `0` or `1`.
    pub fn read_raw(&mut self) -> Result<bool, MarshalerError> {
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(MarshalerError::InvalidDiscriminant { value }),
        }
    }

    /// Read a `u16` using this buffer's endian mode.
    ///
    /// # Errors
    ///
    /// Returns [`MarshalerError::BufferUnderrun`] when fewer than two bytes remain.
    pub fn read_u16(&mut self) -> Result<u16, MarshalerError> {
        if self.position + 2 <= self.data.len() {
            let bytes = [self.data[self.position], self.data[self.position + 1]];
            self.position += 2;
            Ok(match self.endian {
                Endian::BigEndian => u16::from_be_bytes(bytes),
                Endian::LittleEndian => u16::from_le_bytes(bytes),
            })
        } else {
            Err(MarshalerError::buffer_underrun(self.left(), 2))
        }
    }

    /// Read a `u32` using this buffer's endian mode.
    ///
    /// # Errors
    ///
    /// Returns [`MarshalerError::BufferUnderrun`] when fewer than four bytes remain.
    pub fn read_u32(&mut self) -> Result<u32, MarshalerError> {
        if self.position + 4 <= self.data.len() {
            let bytes = [
                self.data[self.position],
                self.data[self.position + 1],
                self.data[self.position + 2],
                self.data[self.position + 3],
            ];
            self.position += 4;
            Ok(match self.endian {
                Endian::BigEndian => u32::from_be_bytes(bytes),
                Endian::LittleEndian => u32::from_le_bytes(bytes),
            })
        } else {
            Err(MarshalerError::buffer_underrun(self.left(), 4))
        }
    }

    /// Read `len` bytes as a borrowed slice.
    ///
    /// # Errors
    ///
    /// Returns [`MarshalerError::BufferUnderrun`] when fewer than `len` bytes remain.
    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], MarshalerError> {
        if self.position + len <= self.data.len() {
            let slice = &self.data[self.position..self.position + len];
            self.position += len;
            Ok(slice)
        } else {
            Err(MarshalerError::buffer_underrun(self.left(), len))
        }
    }

    /// Read `N` bytes as a borrowed array reference.
    ///
    /// # Errors
    ///
    /// Returns [`MarshalerError::BufferUnderrun`] when fewer than `N` bytes remain.
    pub fn read_array<const N: usize>(&mut self) -> Result<&'a [u8; N], MarshalerError> {
        self.read_bytes(N)?
            .try_into()
            .map_err(|_| MarshalerError::buffer_underrun(0, N))
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.position >= self.data.len()
    }

    #[must_use]
    pub const fn left(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }

    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }

    #[must_use]
    pub const fn mark(&self) -> ReadBufferMark {
        ReadBufferMark(self.position)
    }

    #[must_use]
    pub const fn endian(&self) -> Endian {
        self.endian
    }

    /// Bytes that have not yet been consumed.
    #[must_use]
    pub fn remaining(&self) -> &'a [u8] {
        &self.data[self.position..]
    }

    /// Bytes in an already-observed absolute range.
    ///
    /// # Errors
    ///
    /// Returns [`MarshalerError::BufferUnderrun`] when the requested range is invalid.
    pub fn range(&self, range: std::ops::Range<usize>) -> Result<&'a [u8], MarshalerError> {
        if range.start <= range.end && range.end <= self.data.len() {
            Ok(&self.data[range])
        } else {
            Err(MarshalerError::buffer_underrun(
                self.left(),
                range.end.saturating_sub(range.start),
            ))
        }
    }

    pub fn skip(&mut self, bytes: usize) {
        self.position = self.position.saturating_add(bytes).min(self.data.len());
    }

    /// Advance by `bytes`, returning an error if the requested range is not
    /// available.
    ///
    /// # Errors
    ///
    /// Returns [`MarshalerError::BufferUnderrun`] when fewer than `bytes` bytes remain.
    pub fn try_skip(&mut self, bytes: usize) -> Result<(), MarshalerError> {
        if self.position + bytes <= self.data.len() {
            self.position += bytes;
            Ok(())
        } else {
            Err(MarshalerError::buffer_underrun(self.left(), bytes))
        }
    }

    pub fn rewind_to(&mut self, mark: ReadBufferMark) {
        self.position = mark.0.min(self.data.len());
    }
}

impl Read for ReadBuffer<'_> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let available = self.left();
        if available == 0 || buf.is_empty() {
            return Ok(0);
        }
        let to_copy = available.min(buf.len());
        buf[..to_copy].copy_from_slice(&self.data[self.position..self.position + to_copy]);
        self.position += to_copy;
        Ok(to_copy)
    }
}

/// Growable write buffer for packet bytes.
pub struct WriteBuffer {
    data: Vec<u8>,
    endian: Endian,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WriteBufferMark(usize);

impl WriteBufferMark {
    #[must_use]
    pub const fn position(self) -> usize {
        self.0
    }
}

impl WriteBuffer {
    /// Create a write buffer with explicit endianness.
    #[must_use]
    pub fn new(endian: Endian) -> Self {
        Self {
            data: Vec::new(),
            endian,
        }
    }

    /// Create a big-endian write buffer.
    #[must_use]
    pub fn carrier() -> Self {
        Self::new(CARRIER_ENDIAN)
    }

    /// Create a big-endian write buffer with pre-allocated capacity.
    #[must_use]
    pub fn carrier_with_capacity(capacity: usize) -> Self {
        Self::with_capacity(CARRIER_ENDIAN, capacity)
    }

    /// Create a little-endian write buffer.
    #[must_use]
    pub fn little_endian() -> Self {
        Self::new(Endian::LittleEndian)
    }

    #[must_use]
    pub fn with_capacity(endian: Endian, capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            endian,
        }
    }

    #[must_use]
    pub fn from_vec(endian: Endian, data: Vec<u8>) -> Self {
        Self { data, endian }
    }

    /// Endianness this buffer uses for integer encoding.
    #[must_use]
    pub fn endian(&self) -> Endian {
        self.endian
    }

    #[must_use]
    pub const fn mark(&self) -> WriteBufferMark {
        WriteBufferMark(self.data.len())
    }

    pub fn write_u8(&mut self, value: u8) {
        self.data.push(value);
    }

    /// Write a boolean encoded as a raw byte value.
    pub fn write_raw(&mut self, value: bool) {
        self.write_u8(u8::from(value));
    }

    pub fn write_u16(&mut self, value: u16) {
        let bytes = match self.endian {
            Endian::BigEndian => value.to_be_bytes(),
            Endian::LittleEndian => value.to_le_bytes(),
        };
        self.data.extend_from_slice(&bytes);
    }

    pub fn write_u32(&mut self, value: u32) {
        let bytes = match self.endian {
            Endian::BigEndian => value.to_be_bytes(),
            Endian::LittleEndian => value.to_le_bytes(),
        };
        self.data.extend_from_slice(&bytes);
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.data
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn reserve(&mut self, additional: usize) {
        self.data.reserve(additional);
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    /// Reset length to zero, keeping the allocated capacity. Use to
    /// reuse the same buffer across many encodes (per-system `Local<>`
    /// in Bevy, thread-local outside Bevy).
    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn truncate(&mut self, len: usize) {
        self.data.truncate(len);
    }

    pub fn truncate_to(&mut self, mark: WriteBufferMark) {
        self.data.truncate(mark.0);
    }

    /// Reserve a fixed-length prefix region, write the body, then patch
    /// the prefix in place once the body is known.
    ///
    /// This is the back-patch dance every length-prefixed framing needs:
    /// 1. Reserve `prefix_len` zero bytes upfront so the body lands at a
    ///    known offset.
    /// 2. Run `write_body` to append the body.
    /// 3. Invoke `patch_prefix(prefix_slice, body_slice)` so the caller
    ///    can compute checksums / sizes / headers over the body and write
    ///    them into the reserved region.
    ///
    /// Owning the dance here keeps the magic-offset arithmetic (and the
    /// need for `pub` mutable buffer access) inside `WriteBuffer`.
    #[must_use]
    pub fn with_fixed_prefix<R>(
        &mut self,
        prefix_len: usize,
        write_body: impl FnOnce(&mut Self) -> R,
        patch_prefix: impl FnOnce(&mut [u8], &[u8]),
    ) -> R {
        let prefix_pos = self.data.len();
        self.data.resize(prefix_pos + prefix_len, 0);
        let body_pos = self.data.len();
        let result = write_body(self);
        let body_end = self.data.len();
        let body_len = body_end - body_pos;
        let region = &mut self.data[prefix_pos..body_end];
        let (prefix_slice, body_tail) = region.split_at_mut(prefix_len);
        let body_slice: &[u8] = &body_tail[..body_len];
        patch_prefix(prefix_slice, body_slice);
        result
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.data.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl Default for WriteBuffer {
    fn default() -> Self {
        Self::new(CARRIER_ENDIAN)
    }
}

impl AsRef<[u8]> for WriteBuffer {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl Write for WriteBuffer {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.write_bytes(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod buffer_api_tests {
    use super::*;

    #[test]
    fn read_buffer_default_endian_constructors() {
        let bytes = [0x01, 0x02];

        let mut carrier = ReadBuffer::carrier(&bytes);
        assert_eq!(carrier.read_u16().unwrap(), 0x0102);

        let mut little = ReadBuffer::little_endian(&bytes);
        assert_eq!(little.read_u16().unwrap(), 0x0201);
    }

    #[test]
    fn try_skip_is_fallible_and_preserves_position_on_error() {
        let bytes = [1, 2, 3];
        let mut rb = ReadBuffer::carrier(&bytes);

        rb.try_skip(2).unwrap();
        assert_eq!(rb.position(), 2);
        assert!(rb.try_skip(2).is_err());
        assert_eq!(rb.position(), 2);
    }

    #[test]
    fn read_buffer_marks_and_fixed_arrays_are_borrowed() {
        let bytes = [1, 2, 3, 4];
        let mut rb = ReadBuffer::carrier(&bytes);

        let mark = rb.mark();
        assert_eq!(rb.read_array::<2>().unwrap(), &[1, 2]);
        rb.rewind_to(mark);
        assert_eq!(rb.read_u8().unwrap(), 1);
        assert_eq!(mark.position(), 0);
    }

    #[test]
    fn write_buffer_supports_reuse_and_io_write() {
        let mut wb = WriteBuffer::carrier_with_capacity(8);
        assert!(wb.capacity() >= 8);

        wb.write_all(&[1, 2, 3]).unwrap();
        assert_eq!(wb.as_ref(), &[1, 2, 3]);

        wb.as_mut_slice()[1] = 9;
        assert_eq!(wb.as_slice(), &[1, 9, 3]);

        wb.clear();
        assert!(wb.is_empty());
    }

    #[test]
    fn write_buffer_mark_rolls_back_without_reallocating() {
        let mut wb = WriteBuffer::carrier_with_capacity(16);
        wb.write_bytes(&[1, 2]);
        let mark = wb.mark();
        let capacity = wb.capacity();

        wb.write_bytes(&[3, 4, 5]);
        wb.truncate_to(mark);

        assert_eq!(wb.as_slice(), &[1, 2]);
        assert_eq!(wb.capacity(), capacity);
        assert_eq!(mark.position(), 2);
    }
}
