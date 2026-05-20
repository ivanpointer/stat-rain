use criterion::{black_box, criterion_group, criterion_main, Criterion};
use stat_rain::effect::{EffectState, RainEngine};

fn bench_build_80x24_cells(c: &mut Criterion) {
    c.bench_function("build_80x24_cells", |b| {
        let mut engine = RainEngine::new(80, 24, 42);
        b.iter(|| {
            black_box(engine.step(black_box(EffectState::default())));
        })
    });
}

criterion_group!(benches, bench_build_80x24_cells);
criterion_main!(benches);
