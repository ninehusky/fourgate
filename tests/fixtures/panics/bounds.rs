#![no_std]
#![no_main]
mod runtime;

#[unsafe(no_mangle)]
#[inline(never)]
pub fn before(data: &[u8; 3], index: usize) -> u8 {
    data[index] // Panic site: index may be out of bounds.
}

#[unsafe(no_mangle)]
#[inline(never)]
pub fn after(data: &[u8; 3], index: usize) -> u8 {
    data[index % 3] // Always in bounds; no panic site.
}

fn run() {
    let index = core::hint::black_box(5);
    core::hint::black_box(before(&[1, 2, 3], index));
    core::hint::black_box(after(&[1, 2, 3], index));
}
