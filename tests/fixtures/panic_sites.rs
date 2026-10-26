//! Real executable fixture: runtime inputs retain several known panic families.
//! Its inventory validates detection; it is not the research benchmark.
use std::hint::black_box;

#[inline(never)]
fn fixture_bounds(index: usize) -> u8 {
    black_box([1_u8, 2, 3])[index]
}

#[inline(never)]
fn fixture_unwrap(value: Option<u8>) -> u8 {
    value.unwrap()
}

#[inline(never)]
fn fixture_divide(divisor: u32) -> u32 {
    100 / divisor
}

#[inline(never)]
fn fixture_copy(source: &[u8]) -> [u8; 3] {
    let mut dest = [0; 3];
    dest.copy_from_slice(source);
    dest
}

#[inline(never)]
fn fixture_explicit(value: usize) {
    if value == 0 {
        panic!("fixture explicit panic");
    }
}

fn main() {
    let input = black_box(std::env::args().count());
    black_box(fixture_bounds(input));
    black_box(fixture_unwrap(black_box(Some(input as u8))));
    black_box(fixture_divide(input as u32));
    black_box(fixture_copy(black_box(&[1, 2, 3])));
    fixture_explicit(input);
}
