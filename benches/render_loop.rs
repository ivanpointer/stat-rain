use criterion::{black_box, criterion_group, criterion_main, Criterion};
use stat_rain::effect::{EffectState, RainEngine};
use stat_rain::metrics::{normalized_cpu_usage, normalized_memory_usage, CpuTicks};

fn bench_build_80x24_cells(c: &mut Criterion) {
    c.bench_function("build_80x24_cells", |b| {
        let mut engine = RainEngine::new(80, 24, 42);
        b.iter(|| {
            black_box(engine.step(black_box(EffectState::default())));
        })
    });
}

fn bench_metric_math(c: &mut Criterion) {
    c.bench_function("metric_math", |b| {
        b.iter(|| {
            let cpu = normalized_cpu_usage(
                black_box(CpuTicks {
                    user: 100,
                    nice: 0,
                    system: 100,
                    idle: 800,
                }),
                black_box(CpuTicks {
                    user: 150,
                    nice: 0,
                    system: 150,
                    idle: 900,
                }),
            );
            let memory = normalized_memory_usage(black_box(1_000_000), black_box(250_000));
            black_box((cpu, memory));
        })
    });
}

criterion_group!(benches, bench_build_80x24_cells, bench_metric_math);
criterion_main!(benches);
