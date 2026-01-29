use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use x86_64::instructions::port::Port;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use core::sync::atomic::{AtomicU32, AtomicBool, Ordering};

// ==================== Port Addresses ====================

/// Primary IDE channel data port
const DATA_PORT: u16 = 0x1F0;

/// Primary IDE channel - error/features register
const ERROR_PORT: u16 = 0x1F1;

/// Primary IDE channel - sector count register
const SECTOR_COUNT_PORT: u16 = 0x1F2;

/// Primary IDE channel - LBA low byte
const LBA_LOW_PORT: u16 = 0x1F3;

/// Primary IDE channel - LBA mid byte
const LBA_MID_PORT: u16 = 0x1F4;

/// Primary IDE channel - LBA high byte
const LBA_HIGH_PORT: u16 = 0x1F5;

/// Primary IDE channel - device select and LBA upper bits
const DEVICE_PORT: u16 = 0x1F6;

/// Primary IDE channel - status/command register
const STATUS_COMMAND_PORT: u16 = 0x1F7;

/// Primary IDE channel - alternate status and control register
const ALT_STATUS_CONTROL_PORT: u16 = 0x3F6;

// ==================== Status Register Flags ====================

/// Bit 7: Controller busy
#[allow(dead_code)]
const STATUS_BSY: u8 = 0x80;

/// Bit 6: Drive ready
#[allow(dead_code)]
const STATUS_DRDY: u8 = 0x40;

/// Bit 5: Write fault
#[allow(dead_code)]
const STATUS_DF: u8 = 0x20;

/// Bit 3: Data request (data ready to transfer)
const STATUS_DRQ: u8 = 0x08;

/// Bit 0: Error occurred
const STATUS_ERR: u8 = 0x01;

// ==================== ATA Commands ====================

/// Read sectors command
const CMD_READ_SECTORS: u8 = 0x20;

/// Write sectors command
#[allow(dead_code)]
const CMD_WRITE_SECTORS: u8 = 0x30;

/// Identify device command
#[allow(dead_code)]
const CMD_IDENTIFY: u8 = 0xEC;

// ==================== Device Select ====================

/// Master drive
const DEVICE_MASTER: u8 = 0xA0;

/// Slave drive
#[allow(dead_code)]
const DEVICE_SLAVE: u8 = 0xB0;

/// Sector size in bytes
const SECTOR_SIZE: usize = 512;

// ==================== State Tracking ====================

/// Tracks the current state of pending disk operations
static PENDING_LBA: AtomicU32 = AtomicU32::new(0);
static OPERATION_COMPLETE: AtomicBool = AtomicBool::new(false);

// ==================== Async Future for Sector Reads ====================

/// Future that completes when a disk sector read operation finishes
pub struct SectorRead {
    /// The LBA address being read
    lba: u32,
}

impl SectorRead {
    /// Create a new SectorRead future for the given LBA
    fn new(lba: u32) -> Self {
        SectorRead { lba }
    }
}

impl Future for SectorRead {
    type Output = [u8; SECTOR_SIZE];

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Check if this read operation is complete
        if OPERATION_COMPLETE.load(Ordering::SeqCst) && PENDING_LBA.load(Ordering::SeqCst) == self.lba {
            OPERATION_COMPLETE.store(false, Ordering::SeqCst);
            let buffer = unsafe { *AtaDriver::get_sector_buffer() };
            return Poll::Ready(buffer);
        }

        // Register waker for when interrupt fires
        if let Ok(waker) = DISK_WAKER.try_get() {
            waker.register(&cx.waker());
        }

        // Double-check after registering waker (race condition safety)
        if OPERATION_COMPLETE.load(Ordering::SeqCst) && PENDING_LBA.load(Ordering::SeqCst) == self.lba {
            OPERATION_COMPLETE.store(false, Ordering::SeqCst);
            let buffer = unsafe { *AtaDriver::get_sector_buffer() };
            return Poll::Ready(buffer);
        }

        Poll::Pending
    }
}


#[derive(Debug, Clone, Copy)]
pub enum DiskRequest {
    Read { lba: u32 },
    Write { lba: u32 },
}

static DISK_REQUEST_QUEUE: OnceCell<ArrayQueue<DiskRequest>> = OnceCell::uninit();

static mut SECTOR_BUFFER: [u8; SECTOR_SIZE] = [0; SECTOR_SIZE];

pub static DISK_WAKER: conquer_once::spin::OnceCell<
    alloc::sync::Arc<futures_util::task::AtomicWaker>,
> = OnceCell::uninit();

// ==================== ATA Driver ====================

pub struct AtaDriver {
    initialized: bool,
}

impl AtaDriver {
    pub fn new() -> Self {
        AtaDriver {
            initialized: false,
        }
    }

    /// Initialize the ATA driver
    pub fn init(&mut self) {
        if self.initialized {
            return;
        }
        DISK_REQUEST_QUEUE
            .try_init_once(|| ArrayQueue::new(32))
            .expect("ATA request queue already initialized");

        DISK_WAKER
            .try_init_once(|| alloc::sync::Arc::new(futures_util::task::AtomicWaker::new()))
            .expect("ATA waker already initialized");

        self.soft_reset();

        self.detect_drives();

        self.initialized = true;
        crate::println!("ATA driver initialized");
    }

    fn soft_reset(&self) {
        unsafe {
            let mut control_port: Port<u8> = Port::new(ALT_STATUS_CONTROL_PORT);
            control_port.write(0x04);
            for _ in 0..1000 {
                core::hint::spin_loop();
            }
            control_port.write(0x00);
        }
    }

    fn detect_drives(&self) {
        unsafe {
            // Select master drive
            let mut device_port: Port<u8> = Port::new(DEVICE_PORT);
            device_port.write(DEVICE_MASTER);

            let mut status_port: Port<u8> = Port::new(STATUS_COMMAND_PORT);
            let status = status_port.read();

            if status != 0xFF {
                crate::println!("ATA master drive detected (status: {:#x})", status);
            } else {
                crate::println!("No ATA master drive detected");
            }
        }
    }

    pub fn read_sector(&mut self, lba: u32) -> Result<(), &'static str> {
        let queue = DISK_REQUEST_QUEUE
            .try_get()
            .map_err(|_| "ATA request queue not initialized")?;

        queue
            .push(DiskRequest::Read { lba })
            .map_err(|_| "ATA request queue full")?;

        PENDING_LBA.store(lba, Ordering::SeqCst);
        OPERATION_COMPLETE.store(false, Ordering::SeqCst);

        self.issue_read_command(lba);

        Ok(())
    }

    /// Asynchronously read a sector from disk
    /// Returns a future that resolves with the sector data when the read completes
    pub async fn read_sector_async(lba: u32) -> [u8; SECTOR_SIZE] {
        // Create a new driver instance to issue the command
        let mut driver = AtaDriver::new();
        let _ = driver.read_sector(lba);

        SectorRead::new(lba).await
    }

    fn issue_read_command(&self, lba: u32) {
        unsafe {
            let lba_low = (lba & 0xFF) as u8;
            let lba_mid = ((lba >> 8) & 0xFF) as u8;
            let lba_high = ((lba >> 16) & 0xFF) as u8;
            let lba_upper = ((lba >> 24) & 0x0F) as u8;

            let mut sector_count_port: Port<u8> = Port::new(SECTOR_COUNT_PORT);
            let mut lba_low_port: Port<u8> = Port::new(LBA_LOW_PORT);
            let mut lba_mid_port: Port<u8> = Port::new(LBA_MID_PORT);
            let mut lba_high_port: Port<u8> = Port::new(LBA_HIGH_PORT);
            let mut device_port: Port<u8> = Port::new(DEVICE_PORT);
            let mut command_port: Port<u8> = Port::new(STATUS_COMMAND_PORT);

            sector_count_port.write(1);

            lba_low_port.write(lba_low);
            lba_mid_port.write(lba_mid);
            lba_high_port.write(lba_high);

            device_port.write(DEVICE_MASTER | 0x40 | lba_upper);

            command_port.write(CMD_READ_SECTORS);
        }
    }

    pub fn get_sector_buffer() -> *const [u8; SECTOR_SIZE] {
        &raw const SECTOR_BUFFER
    }

    pub fn get_sector_buffer_mut() -> *mut [u8; SECTOR_SIZE] {
        &raw mut SECTOR_BUFFER
    }
}

impl Default for AtaDriver {
    fn default() -> Self {
        Self::new()
    }
}


pub fn on_primary_ata_interrupt() {
    use crate::println;

    unsafe {
        let mut status_port: Port<u8> = Port::new(STATUS_COMMAND_PORT);
        let status = status_port.read();

        // Check for errors
        if status & STATUS_ERR != 0 {
            let mut error_port: Port<u8> = Port::new(ERROR_PORT);
            let error = error_port.read();
            println!("ATA read error: {:#x}", error);
            OPERATION_COMPLETE.store(false, Ordering::SeqCst);
            return;
        }

        // Check if data is ready
        if status & STATUS_DRQ != 0 {
            // Read sector data (256 u16 words = 512 bytes)
            let mut data_port: Port<u16> = Port::new(DATA_PORT);
            let buffer = &mut *AtaDriver::get_sector_buffer_mut();

            for i in 0..256 {
                let word = data_port.read();
                buffer[i * 2] = (word & 0xFF) as u8;
                buffer[i * 2 + 1] = (word >> 8) as u8;
            }

            let lba = PENDING_LBA.load(Ordering::SeqCst);
            println!("ATA sector read complete (LBA: {})", lba);

            // Mark operation as complete for the future to detect
            OPERATION_COMPLETE.store(true, Ordering::SeqCst);

            // Wake any waiting task
            if let Ok(waker) = DISK_WAKER.try_get() {
                waker.wake();
            }
        }
    }
}
