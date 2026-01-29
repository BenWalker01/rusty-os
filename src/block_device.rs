/// Abstraction layer for block devices
/// Allows different storage implementations to provide a common interface

use core::future::Future;

/// The size of a standard disk sector in bytes
pub const BLOCK_SIZE: usize = 512;

/// Error types for block device operations
#[derive(Debug, Clone, Copy)]
pub enum BlockDeviceError {
    /// Device not initialized
    NotInitialized,
    /// Request queue is full
    QueueFull,
    /// Device not found
    NotFound,
    /// I/O error during read/write
    IoError,
    /// Invalid block address
    InvalidAddress,
}

/// Trait for block-oriented storage devices
/// Provides asynchronous read/write operations on fixed-size blocks
pub trait BlockDevice: Send + Sync {
    /// Asynchronously read a single block (sector) from the device
    ///
    /// # Arguments
    /// * `block_address` - The logical block address (LBA) to read
    ///
    /// # Returns
    /// A future that resolves to a 512-byte block when the read completes
    fn read_block(&self, block_address: u32) -> impl Future<Output = Result<[u8; BLOCK_SIZE], BlockDeviceError>>;

    /// Asynchronously write a single block (sector) to the device
    ///
    /// # Arguments
    /// * `block_address` - The logical block address (LBA) to write
    /// * `data` - The 512-byte block to write
    ///
    /// # Returns
    /// A future that resolves when the write completes
    fn write_block(&self, block_address: u32, data: &[u8; BLOCK_SIZE]) -> impl Future<Output = Result<(), BlockDeviceError>>;

    /// Get the total number of blocks on the device
    fn block_count(&self) -> u64;

    /// Check if the device is ready for I/O
    fn is_ready(&self) -> bool;
}
