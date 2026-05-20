use criterion::{black_box, criterion_group, criterion_main, Criterion};
use stat_rain::effect::{EffectState, RenderCell};

fn build_cells(width: usize, height: usize, state: EffectState) -> Vec<RenderCell> {
    let len = width * height;
    let hotness = (state.color_hotness.clamp(0.0, 1.0) * 255.0) as u8;
    let brightness = (state.brightness.clamp(0.0, 1.0) * 255.0) as u8;

    (0..len)
        .map(|index| RenderCell {
            glyph: if index % 7 == 0 { 'ﾊ' } else { '0' },
            color_hotness_bucket: hotness,
            brightness_bucket: brightness,
        })
        .collect()
}

fn bench_build_80x24_cells(c: &mut Criterion) {
    c.bench_function("build_80x24_cells", |b| {
        b.iter(|| {
            black_box(build_cells(
                black_box(80),
                black_box(24),
                black_box(EffectState::default()),
            ))
        })
    });
}

criterion_group!(benches, bench_build_80x24_cells);
criterion_main!(benches);
