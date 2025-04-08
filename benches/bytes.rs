use criterion::measurement::WallTime;
use criterion::{Criterion, criterion_group, criterion_main};

use humanize::bytes::{bytes, ibytes, parse_bytes};

pub fn benchmark(c: &mut Criterion) -> &mut Criterion<WallTime> {
    c.bench_function("bytes", |b| b.iter(|| bytes(1005030000)));

    c.bench_function("ibytes", |b| b.iter(|| ibytes(44040192)));

    c.bench_function("parse", |b| b.iter(|| parse_bytes("32.23mib").unwrap()))
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
