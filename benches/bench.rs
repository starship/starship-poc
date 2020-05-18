#[macro_use]
extern crate criterion;
use criterion::Criterion;
use starship_poc::prompt;

fn bench_render(c: &mut Criterion) {
    c.bench_function("BenchRender", move |b| {
        b.iter(|| {
            let prompt_opts = Default::default();
            let _output = prompt::render(prompt_opts);
        })
    });
}

criterion_group!(benches, bench_render);
criterion_main!(benches);
