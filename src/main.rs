#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rusty_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use rusty_os::{memory::translate_addr, println};
use x86_64::structures::paging::PageTable;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    rusty_os::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rusty_os::test_panic_handler(info)
}

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use x86_64::VirtAddr;

    println!("Hello World{}", "!");
    rusty_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);

    let addresses = [
        // vga buffer page
        0xb8000,
        // some code page
        0x201008,
        // some stack page
        0x0100_0020_1a10,
        // virtual mem address mapped to physical address 0
        boot_info.physical_memory_offset,
    ];

    for &address in &addresses {
        let virt = VirtAddr::new(address);
        let phys = unsafe { translate_addr(virt, phys_mem_offset) };
        println!("{:?} -> {:?}", virt, phys);
    }

    #[cfg(test)]
    test_main();

    println!("It did not crash!");

    rusty_os::hlt_loop();
}
