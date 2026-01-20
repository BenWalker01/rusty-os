#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rusty_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{BootInfo, entry_point};
use x86_64::structures::paging::Page;
use core::panic::PanicInfo;
use rusty_os::{memory::{BootInfoFrameAllocator, translate_addr}, println};
use alloc::boxed::Box;

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
    use x86_64::{VirtAddr, structures::paging::Translate};
    use rusty_os::memory;

    println!("Hello World{}", "!");
    rusty_os::init();

    // let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    // let mut mapper = unsafe { memory::init(phys_mem_offset) };
    // let mut frame_allocator = unsafe {
    //     BootInfoFrameAllocator::init(&boot_info.memory_map)
    // };

    // let page = Page::containing_address(VirtAddr::new(0));
    // memory::create_example_mapping(page, &mut mapper, &mut frame_allocator);

    // let page_ptr: *mut u64 = page.start_address().as_mut_ptr();
    // unsafe { page_ptr.offset(400).write_volatile(0xf021_f077_f065_f04e) };

    let x = Box::new(41);

    #[cfg(test)]
    test_main();

    println!("It did not crash!");

    rusty_os::hlt_loop();
}
