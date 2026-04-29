//! Zero-copy XDR parser for Stellar `TransactionEnvelope` (Issue #345).
//!
//! # Memory layout & alignment strategy
//!
//! XDR is big-endian, 4-byte aligned, with no padding between fields.
//! Rather than deserialising into heap-allocated owned types (the old Node.js
//! path), this module works in two layers:
//!
//! 1. **Raw layer** (`RawXdrHeader`, `RawXdrU32`, `RawXdrI64`):
//!    `zerocopy`-annotated structs with `#[repr(C)]` that map directly onto
//!    the incoming `&[u8]` slice.  `zerocopy::Ref` performs a compile-time
//!    size check and a runtime alignment check — no `unsafe` required.
//!    Because XDR guarantees 4-byte alignment and the structs are `#[repr(C)]`
//!    with only `u8`/`[u8; N]` fields, alignment is always satisfied.
//!
//! 2. **Parsed layer** (`ParsedEnvelope<'a>`):
//!    Borrows sub-slices of the original buffer via lifetimes (`'a`).
//!    No field is ever cloned or re-allocated; the struct cannot outlive the
//!    buffer it was parsed from.
//!
//! # Buffer pooling
//!
//! `BufferPool` (see `pool` sub-module) hands out `bytes::BytesMut` buffers
//! from a fixed-size pool, avoiding per-packet allocator calls on the hot path.
//! Callers call `pool.acquire()` → fill the buffer → parse → release.

use std::fmt;
use zerocopy::{byteorder::big_endian as BE, FromBytes, Immutable, KnownLayout};

// ─────────────────────────────────────────────────────────────────────────────
// Error type
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum XdrParseError {
    /// Buffer is shorter than the minimum envelope size.
    TooShort { got: usize, need: usize },
    /// Discriminant value is not a recognised XDR union variant.
    UnknownDiscriminant(u32),
    /// Slice alignment is wrong for the target type (should not happen for
    /// `u8`-based structs, but reported if `zerocopy::Ref` rejects the cast).
    Misaligned,
}

impl fmt::Display for XdrParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { got, need } => {
                write!(f, "XDR buffer too short: got {got} bytes, need {need}")
            }
            Self::UnknownDiscriminant(d) => write!(f, "unknown XDR discriminant: {d:#010x}"),
            Self::Misaligned => write!(f, "XDR buffer is misaligned"),
        }
    }
}

impl std::error::Error for XdrParseError {}

// ─────────────────────────────────────────────────────────────────────────────
// Raw zero-copy structs
//
// All fields are `u8` arrays so alignment is always 1 — zerocopy::Ref will
// never reject the cast on alignment grounds.  Big-endian interpretation is
// done via `zerocopy::byteorder::big_endian` wrappers when we need a numeric
// value.
// ─────────────────────────────────────────────────────────────────────────────

/// First 4 bytes of any XDR envelope: the union discriminant.
/// Layout: `[discriminant: u32 BE]`
#[derive(FromBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct RawXdrDiscriminant {
    pub value: BE::U32,
}

/// XDR `TransactionV1` fixed header (the fields that are always present and
/// fixed-width, before the variable-length operations list).
///
/// Layout (big-endian, 4-byte aligned):
/// ```text
/// offset  size  field
///      0     4  envelope discriminant  (0x00000002 = ENVELOPE_TYPE_TX)
///      4     4  source account type   (0x00000000 = KEY_TYPE_ED25519)
///      8    32  source account key    (Ed25519 public key bytes)
///     40     4  fee                   (u32, stroops)
///     44     8  sequence number       (i64)
///     52     4  preconditions type    (0 = PRECOND_NONE, 1 = PRECOND_TIME, …)
/// ```
/// Total: 56 bytes.  Variable-length fields (memo, operations, ext) follow.
#[derive(FromBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct RawTxV1Header {
    pub envelope_type:    BE::U32,   //  4 bytes — must be 0x00000002
    pub source_acct_type: BE::U32,   //  4 bytes — key type discriminant
    pub source_key:       [u8; 32],  // 32 bytes — Ed25519 public key
    pub fee:              BE::U32,   //  4 bytes — base fee in stroops
    pub seq_num:          BE::I64,   //  8 bytes — sequence number
    pub precond_type:     BE::U32,   //  4 bytes — preconditions discriminant
}

// XDR envelope type discriminants (stellar-xdr "next" spec).
pub const ENVELOPE_TYPE_TX: u32 = 2;
pub const ENVELOPE_TYPE_TX_FEE_BUMP: u32 = 5;

// ─────────────────────────────────────────────────────────────────────────────
// Parsed envelope — borrows from the input buffer
// ─────────────────────────────────────────────────────────────────────────────

/// Envelope type decoded from the discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeType {
    Tx,
    TxFeeBump,
}

/// Zero-copy parsed view of a Stellar `TransactionEnvelope`.
///
/// The lifetime `'a` ties every field to the original `&'a [u8]` buffer.
/// No heap allocation occurs during parsing.
#[derive(Debug)]
pub struct ParsedEnvelope<'a> {
    pub envelope_type: EnvelopeType,
    /// Ed25519 public key bytes of the source account (32 bytes, borrowed).
    pub source_key: &'a [u8; 32],
    /// Transaction fee in stroops.
    pub fee: u32,
    /// Sequence number.
    pub seq_num: i64,
    /// Raw bytes of the remainder (memo + operations + ext + signatures).
    /// Callers that need full deserialisation can pass this to `stellar-xdr`.
    pub remainder: &'a [u8],
}

// ─────────────────────────────────────────────────────────────────────────────
// Parser
// ─────────────────────────────────────────────────────────────────────────────

/// Minimum byte length for a `TransactionV1Envelope` with zero operations.
pub const MIN_TX_V1_LEN: usize = std::mem::size_of::<RawTxV1Header>();

/// Parse a raw XDR `TransactionEnvelope` byte slice without copying.
///
/// # Alignment
/// XDR mandates 4-byte alignment.  `zerocopy::Ref::from_bytes` checks this at
/// runtime and returns `Err` if the slice is misaligned.  Because
/// `RawTxV1Header` contains only `u8` arrays, its alignment requirement is 1,
/// so the check always passes — the `Misaligned` error path is unreachable in
/// practice but is kept for correctness.
///
/// # Lifetimes
/// The returned `ParsedEnvelope<'a>` borrows directly from `buf`; it cannot
/// outlive the buffer.
pub fn parse_envelope(buf: &[u8]) -> Result<ParsedEnvelope<'_>, XdrParseError> {
    if buf.len() < MIN_TX_V1_LEN {
        return Err(XdrParseError::TooShort {
            got: buf.len(),
            need: MIN_TX_V1_LEN,
        });
    }

    // Zero-copy cast — no allocation, no copy.
    let (header, remainder) =
        zerocopy::Ref::<_, RawTxV1Header>::from_prefix(buf).map_err(|_| XdrParseError::Misaligned)?;

    let discriminant = header.envelope_type.get();
    let envelope_type = match discriminant {
        ENVELOPE_TYPE_TX => EnvelopeType::Tx,
        ENVELOPE_TYPE_TX_FEE_BUMP => EnvelopeType::TxFeeBump,
        other => return Err(XdrParseError::UnknownDiscriminant(other)),
    };

    Ok(ParsedEnvelope {
        envelope_type,
        source_key: &header.source_key,
        fee: header.fee.get(),
        seq_num: header.seq_num.get(),
        remainder,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Buffer pool
// ─────────────────────────────────────────────────────────────────────────────

pub mod pool {
    //! Object-pool of `bytes::BytesMut` buffers for incoming network packets.
    //!
    //! Each buffer is pre-allocated to `BUFFER_CAPACITY` bytes.  When a caller
    //! is done with a buffer it calls `release`, which clears and returns it to
    //! the pool — avoiding a round-trip to the system allocator on every packet.

    use bytes::BytesMut;
    use std::sync::{Arc, Mutex};

    /// Default per-buffer capacity (4 KiB — covers the largest Stellar tx).
    pub const BUFFER_CAPACITY: usize = 4096;

    /// A fixed-size pool of reusable `BytesMut` buffers.
    #[derive(Clone)]
    pub struct BufferPool {
        inner: Arc<Mutex<Vec<BytesMut>>>,
    }

    impl BufferPool {
        /// Create a pool pre-populated with `size` buffers.
        pub fn new(size: usize) -> Self {
            let buffers = (0..size)
                .map(|_| BytesMut::with_capacity(BUFFER_CAPACITY))
                .collect();
            Self {
                inner: Arc::new(Mutex::new(buffers)),
            }
        }

        /// Acquire a buffer from the pool, or allocate a fresh one if empty.
        pub fn acquire(&self) -> BytesMut {
            self.inner
                .lock()
                .expect("pool lock poisoned")
                .pop()
                .unwrap_or_else(|| BytesMut::with_capacity(BUFFER_CAPACITY))
        }

        /// Return a buffer to the pool after clearing it.
        pub fn release(&self, mut buf: BytesMut) {
            buf.clear();
            self.inner
                .lock()
                .expect("pool lock poisoned")
                .push(buf);
        }

        /// Number of buffers currently available in the pool.
        pub fn available(&self) -> usize {
            self.inner.lock().expect("pool lock poisoned").len()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BufMut;

    /// Build a minimal valid `TransactionV1Envelope` byte slice for testing.
    fn minimal_tx_v1() -> Vec<u8> {
        let mut buf = Vec::with_capacity(MIN_TX_V1_LEN + 16);
        buf.put_u32(ENVELOPE_TYPE_TX);          // envelope discriminant
        buf.put_u32(0);                          // source account type: ED25519
        buf.put_bytes(0xAB, 32);                 // source key (dummy)
        buf.put_u32(100);                        // fee: 100 stroops
        buf.put_i64(12345678);                   // sequence number
        buf.put_u32(0);                          // preconditions: NONE
        // remainder: empty memo + 0 operations + ext
        buf.put_u32(0);                          // memo type: MEMO_NONE
        buf.put_u32(0);                          // operations count: 0
        buf.put_u32(0);                          // ext: 0
        buf.put_u32(0);                          // signatures count: 0
        buf
    }

    #[test]
    fn parses_minimal_envelope() {
        let raw = minimal_tx_v1();
        let env = parse_envelope(&raw).expect("parse failed");
        assert_eq!(env.envelope_type, EnvelopeType::Tx);
        assert_eq!(env.fee, 100);
        assert_eq!(env.seq_num, 12345678);
        assert!(env.source_key.iter().all(|&b| b == 0xAB));
    }

    #[test]
    fn rejects_too_short() {
        let raw = vec![0u8; 10];
        assert!(matches!(
            parse_envelope(&raw),
            Err(XdrParseError::TooShort { .. })
        ));
    }

    #[test]
    fn rejects_unknown_discriminant() {
        let mut raw = minimal_tx_v1();
        // Overwrite discriminant with an invalid value.
        raw[0..4].copy_from_slice(&0xDEADBEEFu32.to_be_bytes());
        assert!(matches!(
            parse_envelope(&raw),
            Err(XdrParseError::UnknownDiscriminant(_))
        ));
    }

    #[test]
    fn buffer_pool_acquire_release() {
        let pool = pool::BufferPool::new(4);
        assert_eq!(pool.available(), 4);
        let buf = pool.acquire();
        assert_eq!(pool.available(), 3);
        pool.release(buf);
        assert_eq!(pool.available(), 4);
    }

    #[test]
    fn buffer_pool_allocates_on_empty() {
        let pool = pool::BufferPool::new(0);
        let buf = pool.acquire(); // should not panic
        assert_eq!(buf.capacity(), pool::BUFFER_CAPACITY);
    }
}
