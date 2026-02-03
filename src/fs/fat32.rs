use crate::ata;
use alloc::vec::Vec;

pub struct BootSector {
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub num_fats: u8,
    pub total_sectors_32: u32,
    pub sectors_per_fat_32: u32,
    pub root_cluster: u32,
}

impl BootSector {
    pub async fn read() -> Result<Self, &'static str> {
        let sector = ata::AtaDriver::read_sector_async(0).await;
        
        if sector[510] != 0x55 || sector[511] != 0xAA {
            return Err("Invalid FAT32 boot signature");
        }
        
        // (little-endian)
        let bytes_per_sector = u16::from_le_bytes([sector[0x0B], sector[0x0C]]);
        let sectors_per_cluster = sector[0x0D];
        let reserved_sectors = u16::from_le_bytes([sector[0x0E], sector[0x0F]]);
        let num_fats = sector[0x10];
        let total_sectors_32 = u32::from_le_bytes([
            sector[0x20], sector[0x21], sector[0x22], sector[0x23]
        ]);
        let sectors_per_fat_32 = u32::from_le_bytes([
            sector[0x24], sector[0x25], sector[0x26], sector[0x27]
        ]);
        let root_cluster = u32::from_le_bytes([
            sector[0x2C], sector[0x2D], sector[0x2E], sector[0x2F]
        ]);
        
        crate::println!("[FAT32] Boot Sector Parsed:");
        crate::println!("  Bytes per sector: {}", bytes_per_sector);
        crate::println!("  Sectors per cluster: {}", sectors_per_cluster);
        crate::println!("  Reserved sectors: {}", reserved_sectors);
        crate::println!("  Number of FATs: {}", num_fats);
        crate::println!("  Total sectors: {}", total_sectors_32);
        crate::println!("  Sectors per FAT: {}", sectors_per_fat_32);
        crate::println!("  Root cluster: {}", root_cluster);
        
        Ok(BootSector {
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sectors,
            num_fats,
            total_sectors_32,
            sectors_per_fat_32,
            root_cluster,
        })
    }
    
    pub fn fat_start_sector(&self) -> u32 {
        self.reserved_sectors as u32
    }
    
    pub fn data_start_sector(&self) -> u32 {
        self.reserved_sectors as u32 + (self.num_fats as u32 * self.sectors_per_fat_32)
    }
    
    pub fn cluster_to_sector(&self, cluster: u32) -> u32 {
        self.data_start_sector() + ((cluster - 2) * self.sectors_per_cluster as u32)
    }
}

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: [u8; 11],  // 8-byte name + 3-byte extension (space-padded)
    pub attributes: u8,
    pub first_cluster: u32,
    pub file_size: u32,
}

impl DirEntry {
    pub fn is_directory(&self) -> bool {
        (self.attributes & 0x10) != 0
    }
    
    pub fn is_valid(&self) -> bool {
        self.name[0] != 0x00 && self.name[0] != 0xE5
    }
    
    pub fn get_name(&self) -> alloc::string::String {
        use alloc::string::String;
        let mut name = String::new();
        for &b in &self.name[0..8] {
            if b != 0x20 && b != 0 {
                name.push(b as char);
            }
        }
        name.push('.');
        for &b in &self.name[8..11] {
            if b != 0x20 && b != 0 {
                name.push(b as char);
            }
        }
        name
    }
}

pub struct Fat32 {
    pub boot_sector: BootSector,
}

impl Fat32 {
    pub async fn new() -> Result<Self, &'static str> {
        let boot_sector = BootSector::read().await?;
        Ok(Fat32 { boot_sector })
    }
    
    pub async fn follow_cluster_chain(&self, start_cluster: u32) -> Result<Vec<u32>, &'static str> {
        let mut clusters = Vec::new();
        let mut current = start_cluster;
        let mut iterations = 0;
        const MAX_ITERATIONS: usize = 10000; // Prevent infinite loops
        
        loop {
            if iterations >= MAX_ITERATIONS {
                return Err("Cluster chain too long or infinite loop");
            }
            iterations += 1;
            
            clusters.push(current);
            
            let fat_entry_offset = (current as usize) * 4;
            let fat_sector_index = fat_entry_offset / 512;
            let fat_offset = fat_entry_offset % 512;
            
            let fat_sector_number = self.boot_sector.fat_start_sector() + fat_sector_index as u32;
            let fat_data = ata::AtaDriver::read_sector_async(fat_sector_number).await;
            
            let next_cluster = u32::from_le_bytes([
                fat_data[fat_offset],
                fat_data[fat_offset + 1],
                fat_data[fat_offset + 2],
                fat_data[fat_offset + 3],
            ]);
            
            if next_cluster >= 0x0FFFFFF8 {
                break;
            }
            
            current = next_cluster;
        }
        
        Ok(clusters)
    }
    
    pub async fn read_file(&self, entry: &DirEntry) -> Result<Vec<u8>, &'static str> {
        let clusters = self.follow_cluster_chain(entry.first_cluster).await?;
        let mut data = Vec::new();
        
        for cluster in clusters {
            let cluster_sector = self.boot_sector.cluster_to_sector(cluster);
            
            for i in 0..self.boot_sector.sectors_per_cluster {
                let sector = ata::AtaDriver::read_sector_async(cluster_sector + i as u32).await;
                data.extend_from_slice(&sector);
            }
        }
        
        data.truncate(entry.file_size as usize);
        Ok(data)
    }
    
    pub async fn list_root_directory(&self) -> Result<Vec<DirEntry>, &'static str> {
        self.list_directory(self.boot_sector.root_cluster).await
    }
    
    pub async fn list_directory(&self, cluster: u32) -> Result<Vec<DirEntry>, &'static str> {
        let mut entries = Vec::new();
        let clusters = self.follow_cluster_chain(cluster).await?;
        
        for cluster_num in clusters {
            let cluster_sector = self.boot_sector.cluster_to_sector(cluster_num);
            
            for sector_offset in 0..self.boot_sector.sectors_per_cluster {
                let sector = ata::AtaDriver::read_sector_async(
                    cluster_sector + sector_offset as u32
                ).await;
                
                for entry_index in 0..16 {
                    let offset = entry_index * 32;
                    
                    let mut name = [0u8; 11];
                    name.copy_from_slice(&sector[offset..offset+11]);
                    
                    let attributes = sector[offset + 11];
                    
                    // FAT32: cluster = (high_word << 16) | low_word
                    let low_cluster = u16::from_le_bytes([
                        sector[offset + 26],
                        sector[offset + 27],
                    ]) as u32;
                    let high_cluster = u16::from_le_bytes([
                        sector[offset + 20],
                        sector[offset + 21],
                    ]) as u32;
                    let first_cluster = (high_cluster << 16) | low_cluster;
                    
                    let file_size = u32::from_le_bytes([
                        sector[offset + 28],
                        sector[offset + 29],
                        sector[offset + 30],
                        sector[offset + 31],
                    ]);
                    
                    let entry = DirEntry {
                        name,
                        attributes,
                        first_cluster,
                        file_size,
                    };
                    
                    if entry.is_valid() {
                        entries.push(entry);
                    }
                }
            }
        }
        
        Ok(entries)
    }
    
    // ==================== Write Operations ====================
    
    pub async fn find_free_cluster(&self) -> Result<u32, &'static str> {
        let sectors_per_fat = self.boot_sector.sectors_per_fat_32;
        
        for fat_sector_idx in 0..sectors_per_fat {
            let fat_sector_number = self.boot_sector.fat_start_sector() + fat_sector_idx;
            let fat_data = ata::AtaDriver::read_sector_async(fat_sector_number).await;
            
            for offset in (0..512).step_by(4) {
                let cluster_value = u32::from_le_bytes([
                    fat_data[offset],
                    fat_data[offset + 1],
                    fat_data[offset + 2],
                    fat_data[offset + 3],
                ]);
                
                if cluster_value == 0x00000000 {
                    // Found a free cluster
                    let cluster_index = (fat_sector_idx * 512 + offset as u32) / 4;
                    if cluster_index >= 2 {
                        return Ok(cluster_index);
                    }
                }
            }
        }
        
        Err("No free clusters available")
    }
    
    pub async fn allocate_cluster(&self, previous_cluster: Option<u32>, next_cluster: Option<u32>) -> Result<u32, &'static str> {
        let new_cluster = self.find_free_cluster().await?;
        
        // If previous_cluster is specified, update its FAT entry to point to new_cluster
        if let Some(prev) = previous_cluster {
            self.set_fat_entry(prev, new_cluster).await?;
        }
        
        // Mark the new cluster as end of chain (0x0FFFFFF8)
        if next_cluster.is_none() {
            self.set_fat_entry(new_cluster, 0x0FFFFFF8).await?;
        } else {
            self.set_fat_entry(new_cluster, next_cluster.unwrap()).await?;
        }
        
        Ok(new_cluster)
    }
    
    pub async fn set_fat_entry(&self, cluster: u32, value: u32) -> Result<(), &'static str> {
        let fat_entry_offset = (cluster as usize) * 4;
        let fat_sector_index = fat_entry_offset / 512;
        let fat_offset = fat_entry_offset % 512;
        
        let fat_sector_number = self.boot_sector.fat_start_sector() + fat_sector_index as u32;
        
        // Read the FAT sector
        let mut fat_data = ata::AtaDriver::read_sector_async(fat_sector_number).await;
        
        // Update the entry
        let value_bytes = value.to_le_bytes();
        fat_data[fat_offset] = value_bytes[0];
        fat_data[fat_offset + 1] = value_bytes[1];
        fat_data[fat_offset + 2] = value_bytes[2];
        fat_data[fat_offset + 3] = value_bytes[3];
        
        // Write the updated FAT sector back
        ata::AtaDriver::write_sector_async(fat_sector_number, &fat_data).await?;
        
        // Also write to the backup FAT (if num_fats > 1)
        if self.boot_sector.num_fats > 1 {
            let backup_fat_sector = fat_sector_number + self.boot_sector.sectors_per_fat_32;
            ata::AtaDriver::write_sector_async(backup_fat_sector, &fat_data).await?;
        }
        
        Ok(())
    }
    
    pub async fn create_file(&self, directory_cluster: u32, name: &str, data: &[u8]) -> Result<(), &'static str> {
        if name.len() > 12 {
            return Err("Filename too long");
        }
        
        // Allocate clusters for the file data
        let mut current_cluster = None;
        let mut first_cluster = None;
        
        for chunk in data.chunks(512 * self.boot_sector.sectors_per_cluster as usize) {
            let new_cluster = self.allocate_cluster(current_cluster, None).await?;
            if first_cluster.is_none() {
                first_cluster = Some(new_cluster);
            }
            current_cluster = Some(new_cluster);
            
            // Write the data to the cluster
            let cluster_sector = self.boot_sector.cluster_to_sector(new_cluster);
            let mut sector_data = [0u8; 512];
            for (i, &byte) in chunk.iter().enumerate() {
                if i < 512 {
                    sector_data[i] = byte;
                }
            }
            ata::AtaDriver::write_sector_async(cluster_sector, &sector_data).await?;
        }
        
        // Create directory entry
        let entry = self.create_dir_entry(name, first_cluster.unwrap_or(0), data.len() as u32);
        
        // Write directory entry to the directory cluster
        self.write_dir_entry(directory_cluster, &entry).await?;
        
        Ok(())
    }
    
    fn create_dir_entry(&self, name: &str, first_cluster: u32, file_size: u32) -> [u8; 32] {
        let mut entry = [0u8; 32];
        
        // Parse name into 8.3 format
        let parts: Vec<&str> = name.split('.').collect();
        let base_name = if parts.len() > 0 { parts[0] } else { "" };
        let ext = if parts.len() > 1 { parts[1] } else { "" };
        
        // Fill in base name (8 bytes, space-padded)
        for (i, &b) in base_name.as_bytes().iter().take(8).enumerate() {
            entry[i] = b;
        }
        for i in base_name.len()..8 {
            entry[i] = 0x20; // space
        }
        
        // Fill in extension (3 bytes, space-padded)
        for (i, &b) in ext.as_bytes().iter().take(3).enumerate() {
            entry[8 + i] = b;
        }
        for i in ext.len()..3 {
            entry[8 + i] = 0x20; // space
        }
        
        // Attributes: 0x20 = archive
        entry[11] = 0x20;
        
        // Reserved and creation time (skip for simplicity)
        
        // High cluster bits
        let high_cluster = ((first_cluster >> 16) & 0xFFFF) as u16;
        entry[20..22].copy_from_slice(&high_cluster.to_le_bytes());
        
        // Low cluster bits
        let low_cluster = (first_cluster & 0xFFFF) as u16;
        entry[26..28].copy_from_slice(&low_cluster.to_le_bytes());
        
        // File size
        entry[28..32].copy_from_slice(&file_size.to_le_bytes());
        
        entry
    }
    
    pub async fn write_dir_entry(&self, directory_cluster: u32, entry: &[u8; 32]) -> Result<(), &'static str> {
        let clusters = self.follow_cluster_chain(directory_cluster).await?;
        
        for cluster_num in clusters {
            let cluster_sector = self.boot_sector.cluster_to_sector(cluster_num);
            
            for sector_offset in 0..self.boot_sector.sectors_per_cluster {
                let sector = ata::AtaDriver::read_sector_async(
                    cluster_sector + sector_offset as u32
                ).await;
                
                // Look for an empty directory entry slot
                for entry_index in 0..16 {
                    let offset = entry_index * 32;
                    
                    // Check if entry is free (first byte is 0x00 or 0xE5)
                    if sector[offset] == 0x00 || sector[offset] == 0xE5 {
                        // Found a free slot, write the entry
                        let mut new_sector = sector;
                        new_sector[offset..offset+32].copy_from_slice(entry);
                        
                        ata::AtaDriver::write_sector_async(
                            cluster_sector + sector_offset as u32,
                            &new_sector
                        ).await?;
                        
                        return Ok(());
                    }
                }
            }
        }
        
        Err("No free directory entries available")
    }}