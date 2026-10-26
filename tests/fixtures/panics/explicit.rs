#![no_std]
#![no_main]
mod runtime;

#[unsafe(no_mangle)]
#[inline(never)]
pub fn before(value: u8) -> u8 {
    if value == 0 {
        panic!("zero is invalid"); // Panic site.
    }
    value
}

#[unsafe(no_mangle)]
#[inline(never)]
pub fn after(value: u8) -> Option<u8> {
    if value == 0 { None } else { Some(value) } // Returns failure; no panic site.
}

fn run() {
    let value = core::hint::black_box(5);
    core::hint::black_box(before(value));
    core::hint::black_box(after(value));
}
