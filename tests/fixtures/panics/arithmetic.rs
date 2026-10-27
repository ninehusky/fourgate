#![no_std]
#![no_main]
mod runtime;

#[unsafe(no_mangle)]
#[inline(never)]
pub fn before(divisor: u32) -> u32 {
    100 / divisor // Panic site: divisor may be zero.
}

#[unsafe(no_mangle)]
#[inline(never)]
pub fn after(divisor: u32) -> u32 {
    100 / divisor.max(1) // Nonzero divisor; no panic site.
}

fn run() {
    let divisor = core::hint::black_box(5);
    core::hint::black_box(before(divisor));
    core::hint::black_box(after(divisor));
}
