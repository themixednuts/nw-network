//! Refcounted ring buffer for zero-copy `recv` paths.
//!
//! UDP and `SSL_read` both want to write into a buffer and hand the
//! result off as an owned `Bytes`. The naïve approach — a reused
//! stack buffer + `Bytes::copy_from_slice` — costs one allocation +
//! memcpy per datagram. At MMO scale (5–10 k peers × tens of Hz
//! each) the per-datagram alloc overhead becomes a real bottleneck;
//! at the same scale, eagerly reserving a large per-peer ring
//! becomes a memory-footprint bottleneck instead.
//!
//! [`RecvRing`] addresses both. It lazily allocates a [`BytesMut`]
//! of `chunk_size` bytes on the first recv; each subsequent recv
//! writes into the zero-padded tail and returns an owned `Bytes`
//! that shares the ring's allocation via `Arc`. The ring rolls over
//! to a fresh allocation only when its remaining capacity drops
//! below `max_datagram`. Idle peers cost no memory.
//!
//! Slow consumers don't block the ring: when a refcount on the old
//! allocation outlives the rollover, the old chunk stays alive
//! independently and is collected when its last `Bytes` is dropped.
//!
//! ## Safe public API, zero internal `unsafe`
//!
//! Both [`Self::recv_slot`] and [`Self::commit`] are `safe fn`, and
//! the implementation itself contains no `unsafe` blocks at all.
//! The trick is delegating zero-init to [`BytesMut::resize`], a
//! stable safe API that the compiler lowers to `memset` for `u8`:
//!
//! - `recv_slot` resizes the ring to extend its initialised length
//!   by `max_datagram` zero bytes, then returns a `&mut [u8]` slice
//!   over that newly-zeroed tail. Reads of unwritten bytes return
//!   zero (init), not UB.
//! - `commit(len)` only requires `len <= max_datagram` (panics
//!   otherwise). The slot is fully initialised, so truncating to
//!   `len` and splitting off as `Bytes` exposes only init memory.
//!   If the caller passes a `len` larger than the syscall's actual
//!   write count, the application sees zero-padding instead of UB.
//!
//! Compared to the alternative — `BytesMut::spare_capacity_mut()` +
//! `MaybeUninit::write` per cell + `<[MaybeUninit<u8>]>::
//! assume_init_mut` — this design trades one extra advance of the
//! `BytesMut` length cursor for the elimination of every `unsafe`
//! block. The cost (one `memset(max_datagram)`) is identical.
//!
//! `std::io::BorrowedBuf` (stable in 1.94) is the dedicated stdlib
//! abstraction for the "kernel writes into uninit, we track how
//! much is init" pattern, but it doesn't fit cleanly here: it can't
//! be stored as a `RecvRing` field (self-referential with
//! `BytesMut`'s spare capacity), and recv-style FFI takes
//! `&mut [u8]` rather than `BorrowedCursor`. The
//! `BytesMut::resize`-based design above ends up shorter and
//! produces identical machine code.

use bytes::{Bytes, BytesMut};

/// Refcounted recv buffer. See module docs.
pub struct RecvRing {
    /// Lazily allocated — `BytesMut::new()` until the first
    /// `recv_slot`. Idle peers cost zero allocation.
    ring: BytesMut,
    /// Allocation size used at the first recv and on every rollover.
    chunk_size: usize,
    /// Maximum single-recv length. Each `recv_slot` returns a slice
    /// of exactly this many bytes (assuming the ring has room).
    max_datagram: usize,
    /// Tracks whether `recv_slot` has been called without a matching
    /// `commit`. Necessary because `recv_slot` extends the ring's
    /// `len()` via `resize(_, 0)` — if the caller bails out before
    /// `commit` (e.g. `SSL_read` returned `WANT_READ` and the loop
    /// reissues `recv_slot`), the next `recv_slot` must roll the
    /// dangling zero-pad back off first or it accumulates and ends
    /// up prefixed to the next successful datagram.
    slot_active: bool,
}

impl RecvRing {
    /// `chunk_size` is the ring allocation size (used at first recv
    /// and on each rollover); `max_datagram` is the largest single
    /// recv we'll perform. The ring rolls over when fewer than
    /// `max_datagram` bytes remain in the current chunk.
    pub fn new(chunk_size: usize, max_datagram: usize) -> Self {
        assert!(
            chunk_size >= max_datagram,
            "ring chunk must hold at least one max-sized datagram"
        );
        Self {
            ring: BytesMut::new(),
            chunk_size,
            max_datagram,
            slot_active: false,
        }
    }

    /// Reserve a writable, zero-initialised `&mut [u8]` slot of
    /// `max_datagram` bytes for the next recv syscall. The kernel
    /// (or `SSL_read`) overwrites the prefix; pass the byte count
    /// to [`Self::commit`] to produce an owned `Bytes` sharing the
    /// ring's allocation.
    ///
    /// Returns a slice over **initialised** memory — safe to read,
    /// safe to write. Unwritten bytes are zero. Costs one `memset`
    /// of `max_datagram` bytes (via `BytesMut::resize`).
    ///
    /// If a previous `recv_slot` was not matched by a `commit`
    /// (recv failed, caller retried), the dangling zero-pad tail is
    /// trimmed back here so the new slot lands at the correct
    /// offset.
    pub fn recv_slot(&mut self) -> &mut [u8] {
        // Roll back any uncommitted prior slot before reserving a
        // new one. Without this, a failed recv (SSL `WANT_READ` and
        // friends) would leak its zero-padded `max_datagram` bytes
        // into the ring and the next commit would split them off as
        // a leading zero-pad on the real datagram.
        if self.slot_active {
            let target = self.ring.len() - self.max_datagram;
            self.ring.truncate(target);
            self.slot_active = false;
        }

        if self.ring.capacity() - self.ring.len() < self.max_datagram {
            // Out of room (or first call after construction):
            // allocate a fresh chunk. Old datagrams keep the prior
            // chunk alive via Arc refcount until consumers drop them.
            self.ring = BytesMut::with_capacity(self.chunk_size);
        }
        let start = self.ring.len();
        // Safe stable API: extends the BytesMut with zero bytes,
        // advancing `len()` to `start + max_datagram`. Compiler
        // lowers to `memset` for u8.
        self.ring.resize(start + self.max_datagram, 0);
        self.slot_active = true;
        &mut self.ring[start..]
    }

    /// Commit `len` bytes from the prior [`Self::recv_slot`] as an
    /// owned `Bytes` sharing the ring's allocation.
    ///
    /// Panics if `len > max_datagram` or if no slot is active
    /// (callers should always pass the syscall's reported byte
    /// count, which is bounded by the slot size). If the caller's
    /// `len` exceeds the syscall's actual write count, the trailing
    /// bytes are zero-padding from `recv_slot`'s init — sound, just
    /// garbage from the application's POV.
    pub fn commit(&mut self, len: usize) -> Bytes {
        assert!(
            len <= self.max_datagram,
            "RecvRing::commit: len {} exceeds slot size {}",
            len,
            self.max_datagram
        );
        assert!(
            self.slot_active,
            "RecvRing::commit called without a matching recv_slot"
        );
        self.slot_active = false;
        // The slot is at the back of the ring (we just resized).
        // Trim the unwritten zero-pad tail; the ring's length drops
        // back to `slot_start + len`.
        let trim = self.max_datagram - len;
        let new_len = self.ring.len() - trim;
        self.ring.truncate(new_len);
        // Split off everything up to the slot's end as owned Bytes.
        // In steady-state usage the pre-slot portion is empty (we
        // always commit before the next recv_slot), so
        // `split_to(new_len)` hands the slot bytes to the caller
        // and leaves self empty — sharing the underlying chunk via
        // Arc until rollover.
        self.ring.split_to(new_len).freeze()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_recv_lazy_allocates() {
        let mut ring = RecvRing::new(1024, 256);
        assert_eq!(ring.ring.capacity(), 0, "lazy: no alloc before first recv");
        let slot = ring.recv_slot();
        assert_eq!(slot.len(), 256);
        assert!(slot.iter().all(|&b| b == 0), "slot is zero-initialised");
    }

    #[test]
    fn commit_returns_kernel_written_bytes() {
        let mut ring = RecvRing::new(1024, 256);
        let slot = ring.recv_slot();
        slot[0] = 0x42;
        slot[1] = 0x99;
        slot[2] = 0xAA;
        let b = ring.commit(3);
        assert_eq!(b.as_ref(), &[0x42, 0x99, 0xAA]);
    }

    #[test]
    fn second_recv_after_skipped_commit_trims_dangling_pad() {
        // First recv_slot followed by NO commit (simulates SSL
        // `WANT_READ` returning early). The next recv_slot must
        // discard the prior zero-pad so the commit that follows
        // doesn't leak it as a prefix.
        let mut ring = RecvRing::new(1024, 256);
        let _slot = ring.recv_slot();
        // No commit — caller bailed out.
        let slot = ring.recv_slot();
        slot[0] = 0xEE;
        let b = ring.commit(1);
        assert_eq!(b.as_ref(), &[0xEE], "no leading zero-pad from skipped recv");
    }

    #[test]
    #[should_panic(expected = "commit called without")]
    fn commit_without_recv_slot_panics() {
        let mut ring = RecvRing::new(1024, 256);
        let _ = ring.commit(0);
    }

    #[test]
    fn rollover_after_chunk_exhausted() {
        let mut ring = RecvRing::new(512, 256);
        // First commit: ring = 256 bytes used, 256 free.
        let slot = ring.recv_slot();
        slot[0] = 1;
        let _b1 = ring.commit(256);
        // Second commit: ring = 256/256 used, no spare → rollover
        // to a fresh chunk. The first Bytes stays alive via Arc.
        let slot = ring.recv_slot();
        slot[0] = 2;
        let b2 = ring.commit(256);
        assert_eq!(b2[0], 2);
    }
}
