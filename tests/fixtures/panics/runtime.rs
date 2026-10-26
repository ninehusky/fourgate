// Shared entry point only: fixtures are disassembled, never executed.
#[unsafe(no_mangle)]
extern "C" fn _start() -> ! {
    crate::run();
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
