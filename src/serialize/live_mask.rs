//! Live-mask batch combinators — the "u8 live-mask byte every 8 entries"
//!
//! Several replicated collection formats (`HouseDataComponent::housingItems`,
//! `Projectile::piercingHits`, `GlobalMapDataManager::map`, and the internal
//! replicated map/vector helpers) use chunks of up to 8 entries. Each chunk
//! starts with a single `u8` whose bit `i` indicates whether entry `i` in
//! the chunk carries an optional value (typically the "this slot was
//! updated" flag in a sparse delta). Per-entry headers (keys, indices,
//! sequence numbers) are always emitted; the value body is only present when
//! the bit is set.
//!
//! Before this module the loop was hand-rolled at every callsite. Capturing
//! it once gives one place to look for the protocol shape and one place to
//! cover with tests.

use super::{
    buffer::{ReadBuffer, WriteBuffer},
    error::MarshalerError,
};

/// Read `total` entries laid out as chunks of up to 8: each chunk is
/// preceded by a `u8` live-mask byte, then `min(8, remaining)` per-entry
/// bodies decoded by `f`.
///
/// `f` is called with the buffer and a `live: bool` from the chunk's mask
/// — typically "this slot's optional value is present on the wire." The
/// closure returns the constructed entry; allocate-once via the
/// preallocated `Vec` (`Vec::with_capacity(total)`) avoids reallocation
/// during the read.
///
/// # Errors
///
/// Returns the first error reported while reading a live-mask byte or entry body.
pub fn read_live_mask_batches<T, F>(
    rb: &mut ReadBuffer,
    total: usize,
    mut f: F,
) -> Result<Vec<T>, MarshalerError>
where
    F: FnMut(&mut ReadBuffer, bool) -> Result<T, MarshalerError>,
{
    let mut out = Vec::with_capacity(total);
    let mut remaining = total;
    while remaining > 0 {
        let mask = rb.read_u8()?;
        let batch = remaining.min(8);
        for bit in 0..batch {
            let live = (mask & (1 << bit)) != 0;
            out.push(f(rb, live)?);
        }
        remaining -= batch;
    }
    Ok(out)
}

/// Write `entries` as chunks of up to 8: each chunk is preceded by a `u8`
/// live-mask byte (bit `i` is set when `is_live(&chunk[i])`), then per-entry
/// bodies emitted by `emit`.
///
/// `emit` receives `(wb, entry, live)` so it can decide whether the body
/// includes the value payload. Per-entry headers should always be emitted;
/// the `live` flag controls *only* the optional value.
pub fn write_live_mask_batches<T, L, E>(
    wb: &mut WriteBuffer,
    entries: &[T],
    mut is_live: L,
    mut emit: E,
) where
    L: FnMut(&T) -> bool,
    E: FnMut(&mut WriteBuffer, &T, bool),
{
    let mut start = 0;
    while start < entries.len() {
        let batch = (entries.len() - start).min(8);
        let chunk = &entries[start..start + batch];
        let mut mask = 0u8;
        for (bit, entry) in chunk.iter().enumerate() {
            if is_live(entry) {
                mask |= 1 << bit;
            }
        }
        wb.write_u8(mask);
        for entry in chunk {
            emit(wb, entry, is_live(entry));
        }
        start += batch;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::buffer::CARRIER_ENDIAN;

    /// Round-trip a vec of `Option<u32>` through the combinator pair using
    /// "bit set ⇒ value present" as the live-mask convention.
    #[test]
    fn round_trips_sparse_optionals() {
        let entries = [
            Some(1u32),
            None,
            Some(3),
            Some(4),
            None,
            None,
            None,
            Some(8),
            Some(9),
        ];

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        write_live_mask_batches(&mut wb, &entries, Option::is_some, |wb, opt, live| {
            if live {
                wb.write_u32(opt.unwrap());
            }
        });
        let bytes = wb.into_vec();

        // Two batch bytes (8 + 1) plus four u32 values in the first batch
        // (entries 0,2,3,7) and one u32 in the second (entry 8).
        // 1 + 4*4 + 1 + 1*4 = 22 bytes.
        assert_eq!(bytes.len(), 22);

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded =
            read_live_mask_batches::<Option<u32>, _>(&mut rb, entries.len(), |rb, live| {
                if live {
                    let mut buf = [0u8; 4];
                    buf.copy_from_slice(rb.read_bytes(4)?);
                    Ok(Some(u32::from_be_bytes(buf)))
                } else {
                    Ok(None)
                }
            })
            .unwrap();
        assert_eq!(rb.left(), 0);
        assert_eq!(decoded, entries);
    }

    /// Empty input emits no bytes and reads nothing — boundary safety.
    #[test]
    fn empty_is_a_no_op() {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        write_live_mask_batches(&mut wb, &[] as &[u32], |_| true, |_, _, _| {});
        assert!(wb.is_empty());

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &[]);
        let out: Vec<u8> = read_live_mask_batches(&mut rb, 0, |_, _| Ok(0)).unwrap();
        assert!(out.is_empty());
    }
}
