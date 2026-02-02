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

    println!("ATA Simple Single Test");
    rusty_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    // Initialize ATA driver
    let mut ata_driver = rusty_os::ata::AtaDriver::new();
    ata_driver.init();

    test_main();

    println!("Test passed!");
    rusty_os::hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rusty_os::test_panic_handler(info)
}

#[test_case]
fn test_simple_sector_read() {
    use rusty_os::task::{Task, executor::Executor};

    async fn read_sector_0() {
        println!("Reading sector 0...");
        let sector = rusty_os::ata::AtaDriver::read_sector_async(0).await;
        
        println!("Got sector data, first 5 bytes: {:02x} {:02x} {:02x} {:02x} {:02x}", 
            sector[0], sector[1], sector[2], sector[3], sector[4]);
        
        println!("Byte 0 = {:#x} (decimal {})", sector[0], sector[0]);
        
        assert_eq!(sector[0], 0x00, "Expected 0x00, got {:#x}", sector[0]);
        assert_eq!(sector[1], 0x01, "Expected 0x01, got {:#x}", sector[1]);
        
        println!("Test passed!");
    }

    println!("Creating executor...");
    let mut executor = Executor::new();
    println!("Spawning task...");
    executor.spawn(Task::new(read_sector_0()));
    println!("Running executor...");
    executor.run_until_idle();
    println!("Executor done");
}
