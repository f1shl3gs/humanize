use criterion::measurement::WallTime;
use criterion::{Criterion, criterion_group, criterion_main};

use humanize::duration::{duration, parse_duration};

pub fn benchmark(c: &mut Criterion) -> &mut Criterion<WallTime> {
    c.bench_function("parse_duration", |b| {
        b.iter(|| {
            parse_duration("3m20s").unwrap();
        })
    });

    c.bench_function("duration_to_string", |b| {
        let d = parse_duration("1h20m30s40ms").unwrap();

        b.iter(|| duration(&d))
    })
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
