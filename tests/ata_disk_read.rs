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

    println!("ATA Disk Read Tests");
    rusty_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    // Initialize ATA driver
    let mut ata_driver = rusty_os::ata::AtaDriver::new();
    ata_driver.init();

    test_main();

    println!("All ATA tests passed!");
    rusty_os::hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rusty_os::test_panic_handler(info)
}

#[test_case]
fn test_read_sector_0_pattern() {
    use rusty_os::task::{Task, executor::Executor};

    async fn read_and_verify_sector_0() {
        println!("Testing sector 0 read and pattern verification...");

        let sector = rusty_os::ata::AtaDriver::read_sector_async(0).await;

        // Verify FAT32 boot sector signature
        assert_eq!(
            sector[0], 0xEB,
            "Sector 0 byte 0 should be 0xEB (FAT32 jump)"
        );
        assert_eq!(sector[1], 0x58, "Sector 0 byte 1 should be 0x58");
        assert_eq!(sector[2], 0x90, "Sector 0 byte 2 should be 0x90");

        // Verify boot sector signature at end
        assert_eq!(sector[510], 0x55, "Sector 0 byte 510 should be 0x55");
        assert_eq!(sector[511], 0xAA, "Sector 0 byte 511 should be 0xAA");

        println!("FAT32 boot sector verified");
    }

    println!("Sector 0 pattern verified successfully");

    let mut executor = Executor::new();
    executor.spawn(Task::new(read_and_verify_sector_0()));
    executor.run_until_idle();
}

#[test_case]
fn test_read_multiple_sectors() {
    use rusty_os::task::{Task, executor::Executor};

    async fn read_sectors_0_through_2() {
        println!("Testing multiple sector reads...");

        // Read sector 0 (FAT32 boot sector)
        let sector_0 = rusty_os::ata::AtaDriver::read_sector_async(0).await;
        assert_eq!(
            sector_0[0], 0xEB,
            "Sector 0, byte 0 should be 0xEB (FAT32 jump)"
        );
        assert_eq!(sector_0[1], 0x58, "Sector 0, byte 1 should be 0x58");
        println!("Sector 0 read verified");

        // Read sector 1
        let sector_1 = rusty_os::ata::AtaDriver::read_sector_async(1).await;
        // Just verify we can read it without panicking
        assert!(sector_1.len() == 512, "Sector 1 should be 512 bytes");
        println!("Sector 1 read verified");

        // Read sector 2
        let sector_2 = rusty_os::ata::AtaDriver::read_sector_async(2).await;
        // Just verify we can read it without panicking
        assert!(sector_2.len() == 512, "Sector 2 should be 512 bytes");
        println!("Sector 2 read verified");
    }

    let mut executor = Executor::new();
    executor.spawn(Task::new(read_sectors_0_through_2()));
    executor.run_until_idle();
}

#[test_case]
fn test_sector_read_consistency() {
    use rusty_os::task::{Task, executor::Executor};

    async fn read_same_sector_twice() {
        println!("Testing sector read consistency (idempotency)...");

        let sector_1_read_a = rusty_os::ata::AtaDriver::read_sector_async(0).await;
        let sector_1_read_b = rusty_os::ata::AtaDriver::read_sector_async(0).await;

        for i in 0..512 {
            assert_eq!(
                sector_1_read_a[i], sector_1_read_b[i],
                "Sector 0 byte {} differs between reads: {:#x} vs {:#x}",
                i, sector_1_read_a[i], sector_1_read_b[i]
            );
        }

        println!("Sector reads are consistent");
    }

    let mut executor = Executor::new();
    executor.spawn(Task::new(read_same_sector_twice()));
    executor.run_until_idle();
}

#[test_case]
fn test_sector_0_specific_values() {
    use rusty_os::task::{Task, executor::Executor};

    async fn verify_specific_bytes() {
        println!("Testing specific byte values in sector 0...");

        let sector = rusty_os::ata::AtaDriver::read_sector_async(0).await;

        // Test FAT32 boot sector specific positions
        assert_eq!(sector[0], 0xEB);  // FAT32 jump
        assert_eq!(sector[1], 0x58);
        assert_eq!(sector[2], 0x90);
        assert_eq!(sector[3], 0x6D);  // mkfs.fat signature
        assert_eq!(sector[510], 0x55); // Boot signature
        assert_eq!(sector[511], 0xAA); // Boot signature

        println!("All FAT32 boot sector values verified");
    }

    let mut executor = Executor::new();
    executor.spawn(Task::new(verify_specific_bytes()));
    executor.run_until_idle();
}

#[test_case]
fn test_buffer_data() {
    use rusty_os::task::{Task, executor::Executor};

    async fn verify_sector_data() {
        println!("Testing sector data integrity...");

        let sector_0 = rusty_os::ata::AtaDriver::read_sector_async(0).await;

        // FAT32 boot sector signature
        assert_eq!(sector_0[0], 0xEB, "FAT32 boot sector jump");
        assert_eq!(sector_0[1], 0x58, "byte 1");
        assert_eq!(sector_0[2], 0x90, "byte 2");
        assert_eq!(sector_0[510], 0x55, "boot signature 1");
        assert_eq!(sector_0[511], 0xAA, "boot signature 2");

        println!("Sector data integrity verified");
    }

    let mut executor = Executor::new();
    executor.spawn(Task::new(verify_sector_data()));
    executor.run_until_idle();
}
