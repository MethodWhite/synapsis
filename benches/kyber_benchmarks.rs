//! Synapsis CRYSTALS-Kyber Performance Benchmarks
//! 
//! Run with: cargo bench --bench kyber_benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pqcrypto_kyber::kyber512;
use pqcrypto_traits::kem::{PublicKey, SecretKey, Ciphertext, SharedSecret};

/// Benchmark Kyber512 keypair generation
fn bench_kyber512_keygen(c: &mut Criterion) {
    c.bench_function("kyber512_keygen", |b| {
        b.iter(|| {
            let (pk, sk) = kyber512::keypair();
            black_box((pk, sk))
        })
    });
}

/// Benchmark Kyber512 encapsulation
fn bench_kyber512_encapsulate(c: &mut Criterion) {
    let (pk, _) = kyber512::keypair();
    
    c.bench_function("kyber512_encapsulate", |b| {
        b.iter(|| {
            let (ss, ct) = kyber512::encapsulate(&pk);
            black_box((ss, ct))
        })
    });
}

/// Benchmark Kyber512 decapsulation
fn bench_kyber512_decapsulate(c: &mut Criterion) {
    let (pk, sk) = kyber512::keypair();
    let (_, ct) = kyber512::encapsulate(&pk);
    
    c.bench_function("kyber512_decapsulate", |b| {
        b.iter(|| {
            let ss = kyber512::decapsulate(&ct, &sk);
            black_box(ss)
        })
    });
}

/// Benchmark full Kyber512 roundtrip (keygen + encap + decap)
fn bench_kyber512_full_roundtrip(c: &mut Criterion) {
    c.bench_function("kyber512_full_roundtrip", |b| {
        b.iter(|| {
            let (pk, sk) = kyber512::keypair();
            let (ss1, ct) = kyber512::encapsulate(&pk);
            let ss2 = kyber512::decapsulate(&ct, &sk);
            black_box((ss1, ss2))
        })
    });
}

/// Benchmark multiple encapsulations (batch)
fn bench_kyber512_batch_encapsulate(c: &mut Criterion) {
    let (pk, _) = kyber512::keypair();
    
    c.bench_function("kyber512_batch_10_encapsulate", |b| {
        b.iter(|| {
            for _ in 0..10 {
                let (ss, ct) = kyber512::encapsulate(&pk);
                black_box((ss, ct));
            }
        })
    });
}

/// Benchmark multiple decapsulations (batch)
fn bench_kyber512_batch_decapsulate(c: &mut Criterion) {
    let (pk, sk) = kyber512::keypair();
    let ciphertexts: Vec<_> = (0..10)
        .map(|_| kyber512::encapsulate(&pk).1)
        .collect();
    
    c.bench_function("kyber512_batch_10_decapsulate", |b| {
        b.iter(|| {
            for ct in &ciphertexts {
                let ss = kyber512::decapsulate(ct, &sk);
                black_box(ss);
            }
        })
    });
}

/// Benchmark key sizes
fn bench_kyber512_key_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("kyber512_key_sizes");
    
    group.bench_function("pk_size", |b| {
        b.iter(|| {
            let (pk, _) = kyber512::keypair();
            black_box(pk.as_bytes().len())
        })
    });
    
    group.bench_function("sk_size", |b| {
        b.iter(|| {
            let (_, sk) = kyber512::keypair();
            black_box(sk.as_bytes().len())
        })
    });
    
    group.bench_function("ct_size", |b| {
        b.iter(|| {
            let (pk, _) = kyber512::keypair();
            let (_, ct) = kyber512::encapsulate(&pk);
            black_box(ct.as_bytes().len())
        })
    });
    
    group.bench_function("ss_size", |b| {
        b.iter(|| {
            let (pk, _) = kyber512::keypair();
            let (ss, _) = kyber512::encapsulate(&pk);
            black_box(ss.as_bytes().len())
        })
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_kyber512_keygen,
    bench_kyber512_encapsulate,
    bench_kyber512_decapsulate,
    bench_kyber512_full_roundtrip,
    bench_kyber512_batch_encapsulate,
    bench_kyber512_batch_decapsulate,
    bench_kyber512_key_sizes,
);

criterion_main!(benches);
