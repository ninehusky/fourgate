#![no_std]
#![no_main]
mod runtime;

#[unsafe(no_mangle)]
#[inline(never)]
pub fn before(value: Option<u8>) -> u8 {
    value.unwrap() // Panic site: value may be None.
}

#[unsafe(no_mangle)]
#[inline(never)]
pub fn after(value: Option<u8>) -> u8 {
    value.unwrap_or(0) // Handles None; no panic site.
}

fn run() {
    let value = core::hint::black_box(Some(5));
    core::hint::black_box(before(value));
    core::hint::black_box(after(value));
}
