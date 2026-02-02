#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rusty_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use rusty_os::println;

entry_point!(test_kernel_main);

fn test_kernel_main(boot_info: &'static BootInfo) -> ! {
    use rusty_os::allocator;
    use rusty_os::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    println!("ATA Direct Read Test");
    rusty_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    // Initialize ATA driver
    let mut ata_driver = rusty_os::ata::AtaDriver::new();
    ata_driver.init();

    println!("Testing direct sector read without async...");
    
    // Issue a read command directly
    let _ = ata_driver.read_sector(0);
    println!("Read command issued for LBA 0");
    
    // Wait for interrupt with busy-wait
    use x86_64::instructions::interrupts;
    println!("Waiting for interrupt (10 second timeout)...");
    let mut count = 0;
    while count < 1000000000 {
        if rusty_os::ata::OPERATION_COMPLETE.load(core::sync::atomic::Ordering::SeqCst) {
            println!("✓ Interrupt fired!");
            break;
        }
        count += 1;
    }
    
    if count >= 1000000000 {
        println!("✗ Timeout - interrupt never fired");
    }
    
    // Check buffer contents
    let buffer = unsafe { &*rusty_os::ata::AtaDriver::get_sector_buffer() };
    println!("Buffer contents (first 16 bytes):");
    for i in 0..16 {
        println!("  [{:2}] = {:#04x}", i, buffer[i]);
    }

    test_main();

    println!("Direct read test complete");
    rusty_os::hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rusty_os::test_panic_handler(info)
}

#[test_case]
fn test_dummy() {
    println!("Dummy test to keep harness happy");
}
