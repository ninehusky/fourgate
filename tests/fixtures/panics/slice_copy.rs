#![no_std]
#![no_main]
mod runtime;

#[unsafe(no_mangle)]
#[inline(never)]
pub fn before(dest: &mut [u8; 3], source: &[u8]) {
    dest.copy_from_slice(source); // Panic site: lengths may differ.
}

#[unsafe(no_mangle)]
#[inline(never)]
pub fn after(dest: &mut [u8; 3], source: &[u8; 3]) {
    dest.copy_from_slice(source); // Equal lengths by type; no panic site.
}

fn run() {
    let mut dest = [0; 3];
    let source = core::hint::black_box(&[1, 2, 3]);
    before(&mut dest, core::hint::black_box(&source[..]));
    after(&mut dest, source);
    core::hint::black_box(dest);
}
