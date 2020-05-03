#![feature(test)]

extern crate test;

use starship_poc::prompt;
use test::Bencher;

#[bench]
fn bench_render(b: &mut Bencher) {
    b.iter(|| {
        let prompt_opts = Default::default();
        prompt::render(prompt_opts);
    })
}
