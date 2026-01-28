# Rusty OS

A minimal x86_64 operating system written in Rust, based on a tutorial from https://os.phil-opp.com/.

## Overview

Rusty OS is an educational OS project demonstrating low-level systems programming in Rust. It includes a bootloader, interrupt handling, memory management with custom allocators, and async/await task execution without the standard library.

## Features

- **Bootloader Integration** - Uses bootloader crate for x86_64 boot sequence
- **Global Descriptor Table (GDT)** - CPU privilege levels and task switching
- **Interrupt Handling** - IDT setup with handlers for exceptions and hardware interrupts
- **Memory Management** - Virtual memory mapping with custom frame allocator
- **Heap Allocation** - Multiple allocator strategies:
  - Bump allocator
  - Linked list allocator
  - Fixed-size block allocator
- **Async Runtime** - Custom task executor with async/await support
- **Keyboard Input** - Async keyboard handler via PS/2 controller
- **VGA Text Buffer** - Text-based console output with macros (`print!`, `println!`)
- **Serial Output** - UART for debug logging
- **Testing Framework** - Built-in test runner for kernel testing

## Building

### Prerequisites

- Rust nightly toolchain
- QEMU (for running the OS unless you wish to actually boot this... Please do not)
- `bootimage` tool: `cargo install bootimage` # This should not be run from the project folder

### Build Commands

Build the kernel image:
```bash
cargo build --release
```

Create a bootable image:
```bash
cargo bootimage --release
```

## Running

Run in QEMU:
```bash
cargo run --release
```

## Testing

Run the test suite:
```bash
cargo test
```

Integration tests are located in `tests/` directory:
- `basic_boot.rs` - Kernel startup
- `heap_allocation.rs` - Memory allocation
- `stack_overflow.rs` - Stack protection
- `should_panic.rs` - Panic handling

## Next Steps

- [ ] File system implementation (FAT32 or ext2)
- [ ] ELF binary loading and execution
- [ ] User-space process isolation
- [ ] System call interface
- [ ] Device driver framework
- [ ] Networking stack
- [ ] Better console with colors and scrolling

## Resources

- [Writing an OS in Rust](https://os.phil-opp.com/)
- [x86_64 Crate Documentation](https://docs.rs/x86_64/)
- [Bootloader Crate](https://docs.rs/bootloader/)
