#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rusty_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use rusty_os::println;
use rusty_os::fs::fat32::Fat32;

entry_point!(test_kernel_main);

fn test_kernel_main(boot_info: &'static BootInfo) -> ! {
    use rusty_os::allocator;
    use rusty_os::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    println!("FAT32 Read Test");
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

fn test_fat32_async() {
    use rusty_os::task::{Task, executor::Executor};
    
    println!("[FAT32] TEST START");
    
    async fn verify_fat32() {
        println!("[FAT32] Attempting to initialize FAT32...");
        
        match Fat32::new().await {
            Ok(_fat32) => {
                println!("[FAT32] ✓ FAT32 boot sector parsed successfully");
            }
            Err(e) => {
                println!("[FAT32] ✗ Failed to initialize FAT32: {}", e);
                panic!("FAT32 initialization failed");
            }
        }
        
        println!("[FAT32] TASK: Done");
    }
    
    let mut executor = Executor::new();
    println!("[FAT32] Spawning async task...");
    executor.spawn(Task::new(verify_fat32()));
    println!("[FAT32] Running executor...");
    executor.run_until_idle();
    println!("[FAT32] TEST END");
}

#[test_case]
fn test_fat32_filesystem() {
    test_fat32_async();
}
