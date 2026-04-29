//! Criterion benchmarks for the zero-copy XDR parser (Issue #345).
//!
//! Run with:
//!   cargo bench --bench xdr_parser
//!
//! Target: sub-microsecond parsing of a standard Stellar TransactionEnvelope.

use bytes::BufMut;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

// Re-use the parser directly from the library crate.
use Bitmesh_backend::chains::stellar::xdr_parser::{
    parse_envelope, pool::BufferPool, ENVELOPE_TYPE_TX, MIN_TX_V1_LEN,
};

// ─────────────────────────────────────────────────────────────────────────────
// Fixture helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Minimal valid TransactionV1Envelope (56-byte header + 16-byte body).
fn make_tx_v1_bytes() -> Vec<u8> {
    let mut buf = Vec::with_capacity(MIN_TX_V1_LEN + 16);
    buf.put_u32(ENVELOPE_TYPE_TX);
    buf.put_u32(0);           // source account type: ED25519
    buf.put_bytes(0x42, 32);  // source key
    buf.put_u32(100);         // fee
    buf.put_i64(9_999_999);   // sequence number
    buf.put_u32(0);           // preconditions: NONE
    buf.put_u32(0);           // memo: NONE
    buf.put_u32(0);           // operations: 0
    buf.put_u32(0);           // ext: 0
    buf.put_u32(0);           // signatures: 0
    buf
}

// ─────────────────────────────────────────────────────────────────────────────
// Benchmarks
// ─────────────────────────────────────────────────────────────────────────────

fn bench_parse_envelope(c: &mut Criterion) {
    let raw = make_tx_v1_bytes();

    c.bench_function("parse_envelope/tx_v1_minimal", |b| {
        b.iter(|| {
            let env = parse_envelope(black_box(&raw)).expect("parse failed");
            black_box(env.fee);
        });
    });
}

fn bench_buffer_pool(c: &mut Criterion) {
    let pool = BufferPool::new(32);

    c.bench_function("buffer_pool/acquire_fill_release", |b| {
        b.iter(|| {
            let mut buf = pool.acquire();
            buf.put_slice(black_box(b"hello stellar"));
            pool.release(buf);
        });
    });
}

criterion_group!(benches, bench_parse_envelope, bench_buffer_pool);
criterion_main!(benches);
